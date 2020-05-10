use std::fs::File;
use std::io::{self, BufWriter, Write};

use glam::{vec3, Vec3};
use rand::random;
use wavefront_obj::obj::{self, ObjSet, Primitive, Vertex};


#[inline(always)]
fn clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        return min;
    }

    if x > max {
        return max;
    }

    return x;
}

// returns a minumal rectangle which contains triangle abc
#[inline(always)]
fn find_box(a: Vec3, b: Vec3, c: Vec3) -> ((usize, usize), (usize, usize)) {
    let max = |a, b| if a > b { a } else { b };
    let min = |a, b| if a < b { a } else { b };

    let min_x = min(a.x(), min(b.x(), c.x()));
    let min_y = min(a.y(), min(b.y(), c.y()));

    let max_x = max(a.x(), max(b.x(), c.x()));
    let max_y = max(a.y(), max(b.y(), c.y()));

    ((min_x as usize, min_y as usize), (max_x as usize, max_y as usize))
}

pub struct Image {
    data: Vec<Vec3>,
    width: usize,
    height: usize,
}

impl Image {
    fn new(width: usize, height: usize) -> Self {
        Image {
            data: vec![vec3(0.1, 0.1, 0.1); width * height],
            width,
            height,
        }
    }

    #[inline(always)]
    fn set(&mut self, col: usize, row: usize, pixel: Vec3) {
        let index = row * self.width + col;
        // if index < self.data.len() {
        self.data[index] = pixel;
        // }
    }

    fn flip_horizontally(&mut self) {
        for row in 0..((self.height + 1) / 2) {
            let idx = row * self.width;
            let bottom_idx = (self.height - row - 1) * self.width;
            for col in 0..self.width {
                self.data.swap(idx + col, bottom_idx + col);
            }
        }
    }

    // write as PPM
    fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
        writeln!(w, "P3")?; // means that colors are in ASCII
        writeln!(w, "{} {}", self.width, self.height)?;
        writeln!(w, "255")?; // max color value

        for line in self.data.chunks(self.width) {
            for pixel in line {
                let r = 256.0 * clamp(pixel.x(), 0.0, 0.999);
                let g = 256.0 * clamp(pixel.y(), 0.0, 0.999);
                let b = 256.0 * clamp(pixel.z(), 0.0, 0.999);
                write!(w, "{} {} {} ", r as u8, g as u8, b as u8)?;
            }
            writeln!(w, "")?;
        }

        Ok(())
    }
}

struct Renderer {
    target: Image,
    zbuffer: Vec<f32>,
}

impl Renderer {
    fn new(width: usize, height: usize) -> Self {
        Renderer {
            target: Image::new(width, height),
            zbuffer: vec![std::f32::NEG_INFINITY; width * height],
        }
    }

    fn flip_horizontally(&mut self) {
        self.target.flip_horizontally();
    }

    fn save(&self, path: &str) -> io::Result<()> {
        let file = File::create(path)?;
        let mut out = BufWriter::new(file);
        self.target.write(&mut out)?;
        Ok(())
    }

    #[inline(always)]
    fn set(&mut self, x: usize, y: usize, color: Vec3) {
        self.target.set(x, y, color);
    }

    fn line(
        &mut self,
        (mut x0, mut y0): (usize, usize),
        (mut x1, mut y1): (usize, usize),
        color: Vec3,
    ) {
        let dx = (x1 as i32 - x0 as i32).abs();
        let dy = (y1 as i32 - y0 as i32).abs();
        let steep = if dx < dy {
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
            true
        } else {
            false
        };

        if x0 > x1 {
            // draw from left to right
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }

        let dx = x1 as i32 - x0 as i32;
        let dy = y1 as i32 - y0 as i32;
        let derror2 = dy.abs() * 2;
        let mut error2 = 0;

        let mut y = y0 as i32;
        let y_step = if y1 > y0 { 1 } else { -1 };

        if steep {
            for x in x0..(x1 + 1) {
                self.set(y as usize, x, color);

                error2 += derror2;
                if error2 > dx {
                    y += y_step;
                    error2 -= dx * 2;
                }
            }
        } else {
            for x in x0..(x1 + 1) {
                self.set(x, y as usize, color);

                error2 += derror2;
                if error2 > dx {
                    y += y_step;
                    error2 -= dx * 2;
                }
            }
        }
    }

    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Vec3) {
        let (min, max) = find_box(a, b, c);
        let barycentric = |p: Vec3| {
            let xs = vec3(
                c.x() - a.x(),
                b.x() - a.x(),
                a.x() - p.x(), // projection(pa, x)
            );
            let ys = vec3(
                c.y() - a.y(),
                b.y() - a.y(),
                a.y() - p.y(),
            );

            let u = xs.cross(ys);
            // (u.y/u.z, u.x/u.z) are coordinates in (a, ab, ac) basis
            // TODO: why are we dividing by z here?

            vec3(1.0 - (u.x() + u.y()) / u.z(), u.y() / u.z(), u.x() / u.z())
        };

        let mut z = 0.0;
        for y in min.1..(max.1 + 1) {
            for x in min.0..(max.0 + 1) {
                let p = vec3(x as f32, y as f32, z);
                let bc = barycentric(p);
                if bc.x() < 0.0 || bc.y() < 0.0 || bc.z() < 0.0
                {
                    // (x, y) is out of triangle
                    continue;
                }

                z = bc.x() * a.z() + bc.y() * b.z() + bc.z() * c.z();

                // if previous pixel put at |x, y| as further away from camera, replace it
                let prev_z = &mut self.zbuffer[x + y * self.target.width];
                if *prev_z < z {
                    *prev_z =  z;
                    self.set(x, y, color);
                }
            }
        }
    }

    fn obj(&mut self, model: &ObjSet) {
        let width = self.target.width;
        let height = self.target.height;
        let translate = |vertex: Vertex| -> Vec3 {
            // coordinates in obj file are in [-1.0; 1.0] range
            let x = (vertex.x as f32 + 1.0) / 2.0 * (width - 1) as f32;
            let y = (vertex.y as f32 + 1.0) / 2.0 * (height - 1) as f32;
            let z = vertex.z as f32;
            vec3(x, y, z)
        };

        for object in &model.objects {
            for geometry in &object.geometry {
                for shape in &geometry.shapes {
                    match shape.primitive {
                        Primitive::Triangle(x, y, z) => {
                            let (x, y, z) = (x.0, y.0, z.0);

                            let p1 = object.vertices[x];
                            let p2 = object.vertices[y];
                            let p3 = object.vertices[z];

                            let to_vec3 =
                                |p: Vertex| -> Vec3 { vec3(p.x as f32, p.y as f32, p.z as f32) };

                            let normal = (to_vec3(p3) - to_vec3(p1))
                                .cross(to_vec3(p2) - to_vec3(p1))
                                .normalize();
                            let light_direction = vec3(0.0, 0.0, -1.0);
                            let intensity = normal.dot(light_direction);

                            if intensity.is_sign_positive() {
                                self.triangle(
                                    translate(p1),
                                    translate(p2),
                                    translate(p3),
                                    white() * intensity,
                                );
                            }
                        }
                        _ => todo!(),
                    }
                }
            }
        }
    }
}

fn random_color() -> Vec3 {
    vec3(random::<f32>(), random::<f32>(), random::<f32>())
}

fn white() -> Vec3 {
    vec3(1.0, 1.0, 1.0)
}

fn red() -> Vec3 {
    vec3(1.0, 0.0, 0.0)
}

fn green() -> Vec3 {
    vec3(0.0, 1.0, 0.0)
}

fn blue() -> Vec3 {
    vec3(0.0, 0.0, 1.0)
}

fn read_model(path: &str) -> Result<ObjSet, Box<dyn std::error::Error>> {
    let model = std::fs::read_to_string(path)?;
    let model = obj::parse(&model)
        .map_err(|e| format!("Failed to parse line #{}: {}", e.line_number, e.message))?;
    Ok(model)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (800, 800);
    let mut renderer = Renderer::new(width, height);

    let model = read_model("obj/african_head.obj")?;
    renderer.obj(&model);

    renderer.flip_horizontally();
    renderer.save("out.ppm")?;
    Ok(())
}
