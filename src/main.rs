use std::error::Error;
use std::time::Instant;

use glam::{vec2, vec3, Vec2, Vec3};
use wavefront_obj::obj::{self, ObjSet, Primitive, TVertex, Vertex};

type Image = image::RgbImage;
type Color = image::Rgb<u8>;
type Texture = Image;

#[inline(always)]
fn min(a: f32, b: f32) -> f32 {
    if a > b {
        b
    } else {
        a
    }
}

#[inline(always)]
fn max(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

#[inline(always)]
fn barycentric(a: Vec2, b: Vec2, c: Vec2, p: Vec2) -> Vec3 {
    let xs = vec3(c.x() - a.x(), b.x() - a.x(), a.x() - p.x());
    let ys = vec3(c.y() - a.y(), b.y() - a.y(), a.y() - p.y());
    let u = xs.cross(ys);
    // (u.y/u.z, u.x/u.z) are coordinates in (a, ab, ac) basis
    // TODO: why are we dividing by z here?
    vec3(1.0 - (u.x() + u.y()) / u.z(), u.y() / u.z(), u.x() / u.z())
}

fn in_triangle<F>(a: Vec2, b: Vec2, c: Vec2, mut f: F) where F: FnMut(usize, usize, Vec3) {
    let min_x = min(a.x(), min(b.x(), c.x())) as usize;
    let min_y = min(a.y(), min(b.y(), c.y())) as usize;

    let max_x = max(a.x(), max(b.x(), c.x())) as usize;
    let max_y = max(a.y(), max(b.y(), c.y())) as usize;

    for y in min_y..(max_y + 1) {
        for x in min_x..(max_x + 1) {
            let p = vec2(x as f32, y as f32);
            let bc = barycentric(a, b, c, p);
            if bc.x() < 0.0 || bc.y() < 0.0 || bc.z() < 0.0 {
                continue;
            }

            f(x, y, bc);
        }
    }
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
        // image::imageops::flip_vertical_in_place(&mut self.zbuffer)
    }

    fn save(&self, path: &str) -> Result<(), Box<dyn Error>> {
        self.target.save(path)?;
        // self.zbuffer.save("zbuffer.png")?;
        Ok(())
    }

    #[inline(always)]
    fn set(&mut self, x: usize, y: usize, color: Color) {
        self.target.put_pixel(x as u32, y as u32, color);
    }

    fn triangle_texture(
        &mut self,
        a: Vec3,
        b: Vec3,
        c: Vec3,
        uv0: Vec2,
        uv1: Vec2,
        uv2: Vec2,
        texture: &Texture,
        intensity: f32,
    ) {
        let intensity = intensity.sqrt(); // gamma correction
        let shade = |color: u8| -> u8 { (color as f32 * intensity) as u8 };

        in_triangle(a.truncate(), b.truncate(), c.truncate(), |x, y, bc| {
            // TODO: WTF?
            let z = a.z() * bc.x() + b.z() * bc.y() + c.z() * bc.z() + 0.5;

            // if previous pixel put at |x, y| as further away from camera, replace it
            let prev_z = &mut self.zbuffer[x + y * self.target.width() as usize];
            if *prev_z <= z {
                *prev_z = z;

                // TODO: WTF?
                let uv = uv0 * bc.x() + uv1 * bc.y() + uv2 * bc.z();
                let color = *texture.get_pixel(uv.x() as u32, uv.y() as u32);
                let color = Color::from([
                    shade(color[0]),
                    shade(color[1]),
                    shade(color[2]),
                ]);
                self.set(x, y, color);
            }
        });
    }

    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        in_triangle(a.truncate(), b.truncate(), c.truncate(), |x, y, bc| {
            let z = a.z() * bc.x() + b.z() * bc.y() + c.z() * bc.z() + 0.5;
            let prev_z = &mut self.zbuffer[x  + y * self.target.width() as usize];
            if *prev_z <= z {
                *prev_z = z;
                self.set(x, y, color);
            }
        });
    }

    fn primitive(
        &mut self,
        primitive: &Primitive,
        vertices: &[Vertex],
        texture_vertices: &[TVertex],
        texture: &Texture,
    ) {
        let width = self.target.width();
        let height = self.target.height();

        let screen_coords = |vertex: Vec3| -> Vec3 {
            // coordinates in obj file are in [-1.0; 1.0] range
            // NOTE: not really, but it's true for african_head.obj
            vec3(
                (vertex.x() + 1.0) / 2.0 * (width - 1) as f32,
                (vertex.y() + 1.0) / 2.0 * (height - 1) as f32,
                vertex.z(),
            )
        };

        let texture_coords = |vertex: Vec2| -> Vec2 {
            vec2(
                vertex.x() * (texture.width() - 1) as f32,
                vertex.y() * (texture.height() - 1) as f32,
            )
        };

        let to_vec3 = |p: Vertex| -> Vec3 { vec3(p.x as f32, p.y as f32, p.z as f32) };
        let to_vec2 = |p: TVertex| -> Vec2 { vec2(p.u as f32, p.v as f32) };

        let light_direction = vec3(0.0, 0.0, -1.0);

        match primitive {
            Primitive::Triangle((x, Some(tx), _), (y, Some(ty), _), (z, Some(tz), _)) => {
                let p1 = to_vec3(vertices[*x]);
                let p2 = to_vec3(vertices[*y]);
                let p3 = to_vec3(vertices[*z]);

                let normal = (p3 - p1).cross(p2 - p1).normalize();
                let intensity = normal.dot(light_direction);
                if intensity.is_sign_positive() {
                    let uv0 = to_vec2(texture_vertices[*tx]);
                    let uv1 = to_vec2(texture_vertices[*ty]);
                    let uv2 = to_vec2(texture_vertices[*tz]);
                    self.triangle_texture(
                        screen_coords(p1),
                        screen_coords(p2),
                        screen_coords(p3),
                        texture_coords(uv0),
                        texture_coords(uv1),
                        texture_coords(uv2),
                        texture,
                        intensity,
                    );
                }
            }
            Primitive::Triangle((x, _, _), (y, _, _), (z, _, _)) => {
                let p1 = to_vec3(vertices[*x]);
                let p2 = to_vec3(vertices[*y]);
                let p3 = to_vec3(vertices[*z]);

                let normal = (p3 - p1).cross(p2 - p1).normalize();
                let intensity = normal.dot(light_direction);

                let shade = (0xff as f32 * intensity) as u8;
                let color = [shade, shade, shade].into();

                if intensity.is_sign_positive() {
                    self.triangle(
                        screen_coords(p1),
                        screen_coords(p2),
                        screen_coords(p3),
                        color
                    );
                }
            },
            _ => todo!(),
        }
    }

    fn obj(&mut self, model: &ObjSet, texture: &Texture) {
        for object in &model.objects {
            for geometry in &object.geometry {
                for shape in &geometry.shapes {
                    self.primitive(
                        &shape.primitive,
                        &object.vertices,
                        &object.tex_vertices,
                        &texture,
                    );
                }
            }
        }
    }
}

fn read_model(path: &str) -> Result<ObjSet, Box<dyn Error>> {
    let model = std::fs::read_to_string(path)?;
    let model = obj::parse(&model)
        .map_err(|e| format!("Failed to parse line #{}: {}", e.line_number, e.message))?;
    Ok(model)
}

fn read_texture(path: &str) -> Result<Texture, Box<dyn Error>> {
    let mut texture = image::open(path)?.to_rgb();
    image::imageops::flip_vertical_in_place(&mut texture);
    Ok(texture)
}

fn main() -> Result<(), Box<dyn Error>> {
    let (width, height) = (1024, 1024);
    let mut renderer = Renderer::new(width, height);

    let model = read_model("obj/african_head.obj")?;
    let texture = read_texture("obj/african_head_diffuse.png")?;

    let start = Instant::now();
    renderer.obj(&model, &texture);
    let elapsed = Instant::now() - start;
    println!("Render took {:.3} ms", elapsed.as_micros() as f64 / 1_000.0);

    renderer.flipv();
    renderer.save("target.png")?;
    Ok(())
}
