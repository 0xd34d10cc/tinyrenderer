use glam::{vec3, Vec3};
use wavefront_obj::obj::{self, ObjSet, Primitive, Vertex};

type Image = image::RgbImage;
type Color = image::Rgb<u8>;


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

struct Renderer {
    target: Image,
    zbuffer: Vec<f32>,
}

impl Renderer {
    fn new(width: usize, height: usize) -> Self {
        Renderer {
            target: Image::new(width as u32, height as u32),
            zbuffer: vec![std::f32::NEG_INFINITY; width * height],
        }
    }

    fn flipv(&mut self) {
        image::imageops::flip_vertical_in_place(&mut self.target);
    }

    fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.target.save(path)?;
        Ok(())
    }

    #[inline(always)]
    fn set(&mut self, x: usize, y: usize, color: Color) {
        self.target.put_pixel(x as u32, y as u32, color);
    }

    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
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

        for y in min.1..(max.1 + 1) {
            for x in min.0..(max.0 + 1) {
                let p = vec3(x as f32, y as f32, 0.0);
                let bc = barycentric(p);
                if bc.x() < 0.0 || bc.y() < 0.0 || bc.z() < 0.0
                {
                    // (x, y) is out of triangle
                    continue;
                }

                // TODO: WTF?
                let z = bc.x() * a.z() + bc.y() * b.z() + bc.z() * c.z();

                // if previous pixel put at |x, y| as further away from camera, replace it
                let prev_z = &mut self.zbuffer[x + y * self.target.width() as usize];
                if *prev_z < z {
                    *prev_z =  z;
                    self.set(x, y, color);
                }
            }
        }
    }

    fn obj(&mut self, model: &ObjSet) {
        let width = self.target.width();
        let height = self.target.height();
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

                            let shade = (0xff as f32 * intensity) as u8;
                            let color = [shade, shade, shade].into();

                            if intensity.is_sign_positive() {
                                self.triangle(
                                    translate(p1),
                                    translate(p2),
                                    translate(p3),
                                    color,
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

fn read_model(path: &str) -> Result<ObjSet, Box<dyn std::error::Error>> {
    let model = std::fs::read_to_string(path)?;
    let model = obj::parse(&model)
        .map_err(|e| format!("Failed to parse line #{}: {}", e.line_number, e.message))?;
    Ok(model)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (800, 800);
    let mut renderer = Renderer::new(width, height);

    let texture = image::open("obj/african_head_diffuse.png")?;
    let model = read_model("obj/african_head.obj")?;
    renderer.obj(&model);

    renderer.flipv();
    renderer.save("out.png")?;
    Ok(())
}
