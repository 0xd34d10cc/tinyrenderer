use std::fs::File;
use std::io::{self, BufWriter, Write};

use glam::{vec3, Vec3};
use rand::random;
use wavefront_obj::obj::{self, ObjSet, Primitive, Vertex};

type Point<T> = (T, T);

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
}

impl Renderer {
    fn new(width: usize, height: usize) -> Self {
        Renderer {
            target: Image::new(width, height),
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

    fn set(&mut self, x: usize, y: usize, color: Vec3) {
        self.target.set(x, y, color);
    }

    fn line(
        &mut self,
        (mut x0, mut y0): Point<usize>,
        (mut x1, mut y1): Point<usize>,
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

    fn triangle(
        &mut self,
        mut p0: Point<usize>,
        mut p1: Point<usize>,
        mut p2: Point<usize>,
        color: Vec3,
    ) {
        // sort points by Y
        if p1.1 < p0.1 {
            std::mem::swap(&mut p1, &mut p0);
        }

        if p2.1 < p0.1 {
            std::mem::swap(&mut p2, &mut p0);
        }

        if p2.1 < p1.1 {
            std::mem::swap(&mut p2, &mut p1);
        }

        debug_assert!(p0.1 <= p1.1);
        debug_assert!(p1.1 <= p2.1);

        let total_height = p2.1 - p0.1;
        let segment_height = p1.1 - p0.1;

        if segment_height != 0 {
            for y in p0.1..(p1.1 + 1) {
                let alpha = (y - p0.1) as f32 / total_height as f32;
                let beta = (y - p0.1) as f32 / segment_height as f32;

                let a = p0.0 as f32 + (p2.0 as i32 - p0.0 as i32) as f32 * alpha;
                let b = p0.0 as f32 + (p1.0 as i32 - p0.0 as i32) as f32 * beta;

                let mut a = a as usize;
                let mut b = b as usize;

                if b < a {
                    std::mem::swap(&mut a, &mut b);
                }

                // TODO: replace by memset?
                for x in a..(b + 1) {
                    self.set(x, y, color);
                }
            }
        }

        let segment_height = p2.1 - p1.1;

        if segment_height != 0 {
            for y in p1.1..(p2.1 + 1) {
                let alpha = (y - p0.1) as f32 / total_height as f32;
                let beta = (y - p1.1) as f32 / segment_height as f32;

                let a = p0.0 as f32 + (p2.0 as i32 - p0.0 as i32) as f32 * alpha;
                let b = p1.0 as f32 + (p2.0 as i32 - p1.0 as i32) as f32 * beta;

                let mut a = a as usize;
                let mut b = b as usize;

                if b < a {
                    std::mem::swap(&mut a, &mut b);
                }

                for x in a..(b + 1) {
                    self.set(x, y, color);
                }
            }
        }
    }

    fn obj(&mut self, model: &ObjSet) {
        for object in &model.objects {
            for geometry in &object.geometry {
                for shape in &geometry.shapes {
                    let translate = |vertex: Vertex| -> Point<usize> {
                        // coordinates in obj file are in [-1.0; 1.0] range
                        let x = (vertex.x as f32 + 1.0) / 2.0 * (WIDTH - 1) as f32;
                        let y = (vertex.y as f32 + 1.0) / 2.0 * (HEIGHT - 1) as f32;
                        (x as usize, y as usize)
                    };

                    match shape.primitive {
                        Primitive::Triangle(x, y, z) => {
                            let (x, y, z) = (x.0, y.0, z.0);

                            let p1 = object.vertices[x];
                            let p2 = object.vertices[y];
                            let p3 = object.vertices[z];

                            self.triangle(
                                translate(p1),
                                translate(p2),
                                translate(p3),
                                random_color(),
                            );
                        }
                        Primitive::Line(x, y) => {
                            let (x, y) = (x.0, y.0);

                            let p1 = object.vertices[x];
                            let p2 = object.vertices[y];

                            self.line(translate(p1), translate(p2), random_color());
                        }
                        Primitive::Point(x) => {
                            let x = x.0;
                            let p = object.vertices[x];
                            let (x, y) = translate(p);
                            self.set(x, y, random_color());
                        }
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

const WIDTH: usize = 800;
const HEIGHT: usize = 800;

fn read_model(path: &str) -> Result<ObjSet, Box<dyn std::error::Error>> {
    let model = std::fs::read_to_string(path)?;
    let model = obj::parse(&model)
        .map_err(|e| format!("Failed to parse line #{}: {}", e.line_number, e.message))?;
    Ok(model)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = Renderer::new(WIDTH, HEIGHT);
    // renderer.triangle((10, 70), (50, 160), (70, 80), red());
    // renderer.triangle((180, 50), (150, 1), (70, 180), white());
    // renderer.triangle((180, 150), (120, 160), (130, 180), green());

    let model = read_model("obj/african_head.obj")?;
    renderer.obj(&model);

    renderer.flip_horizontally();
    renderer.save("out.ppm")?;
    Ok(())
}
