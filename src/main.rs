use std::error::Error;
use std::time::Instant;

use glam::{vec2, vec3, Mat3, Vec2, Vec3};
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

// Invoke function f for every point in triangle abc
fn in_triangle<F>(a: Vec2, b: Vec2, c: Vec2, mut f: F)
where
    F: FnMut(usize, usize, Vec3),
{
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

struct Camera {
    translation: Mat3,
}

impl Camera {
    fn new(lookfrom: Vec3, lookat: Vec3, up: Vec3) -> Self {
        // z axis points from the camera
        let z_axis = (lookat - lookfrom).normalize();
        // y axis points up
        let y_axis = up.normalize();
        // x axis points to the left
        let x_axis = y_axis.cross(z_axis).normalize();
        // translation to camera-centric coordinate system (rotation part)
        let translation = Mat3::from_cols(x_axis, y_axis, z_axis);

        Camera {
            translation,
        }
    }

    // translate point p to camera-centric coordinate system
    fn translate(&self, point: Vec3) -> Vec3 {
        self.translation * point //+ self.lookfrom
    }

    fn direction(&self) -> Vec3 {
        self.translation.z_axis()
    }
}

struct Renderer {
    camera: Camera,
    target: Image,
    zbuffer: Vec<f32>,
}

impl Renderer {
    fn new(camera: Camera, (width, height): (usize, usize)) -> Self {
        Renderer {
            camera,
            target: Image::new(width as u32, height as u32),
            zbuffer: vec![std::f32::NEG_INFINITY; width * height],
        }
    }

    fn flipv(&mut self) {
        image::imageops::flip_vertical_in_place(&mut self.target);
    }

    fn save(&self, path: &str) -> Result<(), Box<dyn Error>> {
        self.target.save(path)?;
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
            let position = x + y * self.target.width() as usize;
            if position >= self.zbuffer.len() {
                // this pixel is out of bounds
                return;
            }

            // TODO: WTF?
            let z = a.z() * bc.x() + b .z() * bc.y() + c.z() * bc.z() + 0.5;

            // if previous pixel put at |x, y| as further away from camera, replace it
            let prev_z = &mut self.zbuffer[position];
            if *prev_z <= z {
                *prev_z = z;

                // TODO: WTF?
                let uv = uv0 * bc.x() + uv1 * bc.y() + uv2 * bc.z();
                let color = *texture.get_pixel(uv.x() as u32, uv.y() as u32);
                let color = Color::from([shade(color[0]), shade(color[1]), shade(color[2])]);
                self.set(x, y, color);
            }
        });
    }

    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        in_triangle(a.truncate(), b.truncate(), c.truncate(), |x, y, bc| {
            let z = a.z() * bc.x() + b.z() * bc.y() + c.z() * bc.z() + 0.5;
            let prev_z = &mut self.zbuffer[x + y * self.target.width() as usize];
            if *prev_z <= z {
                *prev_z = z;
                self.set(x, y, color);
            }
        });
    }

    fn screen_coords(&self, v: Vec3) -> Vec3 {
        let r = self.camera.translate(v);

        // coordinates in obj file are in [-1.0; 1.0] range
        // NOTE: not really, but it's true for african_head.obj
        let r = (r + Vec3::splat(1.0)) / 2.0; // [-1; 1] => [0; 1]
        vec3(
            r.x() * (self.target.width() - 1) as f32,
            r.y() * (self.target.height() - 1) as f32,
            r.z() * (self.target.width() + self.target.height() - 2) as f32 / 2.0
        )
    }

    // returns UV coordinates for v
    fn texture_coords(&self, v: TVertex, texture: &Texture) -> Vec2 {
        vec2(
            v.u as f32 * (texture.width() - 1) as f32,
            v.v as f32 * (texture.height() - 1) as f32,
        )
    }

    fn primitive(
        &mut self,
        primitive: &Primitive,
        vertices: &[Vertex],
        texture_vertices: &[TVertex],
        texture: &Texture,
    ) {
        let to_vec3 = |v: Vertex| vec3(v.x as f32, v.y as f32, v.z as f32);

        let light_direction = self.screen_coords(self.camera.direction()).normalize();
        match primitive {
            Primitive::Triangle((x, Some(tx), _), (y, Some(ty), _), (z, Some(tz), _)) => {
                let a = self.screen_coords(to_vec3(vertices[*x]));
                let b = self.screen_coords(to_vec3(vertices[*y]));
                let c = self.screen_coords(to_vec3(vertices[*z]));

                // dbg!(vertices[*x], a);

                let normal = (b - a).cross(c - a).normalize();
                let intensity = max(normal.dot(light_direction), 0.2);
                if intensity.is_sign_positive() {
                    let uv0 = self.texture_coords(texture_vertices[*tx], texture);
                    let uv1 = self.texture_coords(texture_vertices[*ty], texture);
                    let uv2 = self.texture_coords(texture_vertices[*tz], texture);
                    self.triangle_texture(a, b, c, uv0, uv1, uv2, texture, intensity);
                }
            }
            Primitive::Triangle((x, _, _), (y, _, _), (z, _, _)) => {
                let a = self.screen_coords(to_vec3(vertices[*x]));
                let b = self.screen_coords(to_vec3(vertices[*y]));
                let c = self.screen_coords(to_vec3(vertices[*z]));

                let normal = (b - a).cross(c - a).normalize();
                let intensity = normal.dot(light_direction);

                let shade = (0xff as f32 * intensity) as u8;
                let color = [shade, shade, shade].into();

                if intensity.is_sign_positive() {
                    self.triangle(a, b, c, color);
                }
            }
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
    let lookfrom = vec3(0.0, 0.0, -1.0);
    let lookat = vec3(0.0, 0.0, 0.0);
    let up = vec3(0.0, 1.0, 0.0);
    let camera = Camera::new(lookfrom, lookat, up);
    let mut renderer = Renderer::new(camera, (1024, 1024));
    let model = read_model("obj/african_head.obj")?;
    let texture = read_texture("obj/african_head_diffuse.png")?;

    let start = Instant::now();
    renderer.obj(&model, &texture);
    println!(
        "Render took {:.3} ms",
        start.elapsed().as_micros() as f64 / 1_000.0
    );

    renderer.flipv();
    renderer.save("target.png")?;
    Ok(())
}
