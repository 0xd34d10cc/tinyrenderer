use std::fs::File;
use std::io::{self, BufWriter, Write};

use glam::{vec3, Vec3};

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
        self.data[index] = pixel;
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
}

fn white() -> Vec3 {
    vec3(1.0, 1.0, 1.0)
}

fn red() -> Vec3 {
    vec3(1.0, 0.0, 0.0)
}

fn main() -> io::Result<()> {
    let mut renderer = Renderer::new(100, 100);

    renderer.line((13, 20), (80, 40), white());
    renderer.line((20, 13), (40, 80), red());
    renderer.line((80, 40), (13, 20), red());

    renderer.flip_horizontally();
    renderer.save("out.ppm")?;
    Ok(())
}
