use std::io::{self, Write, BufWriter};
use std::fs::File;

use glam::{Vec3, vec3};


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
    pub fn new(width: usize, height: usize) -> Self {
        Image {
            data: vec![Vec3::default(); width * height],
            width,
            height,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn get(&self, col: usize, row: usize) -> Vec3 {
        let index = row * self.width + col;
        self.data[index]
    }

    pub fn set(&mut self, col: usize, row: usize, pixel: Vec3) {
        let index = row * self.width + col;
        self.data[index] = pixel;
    }

    pub fn data_mut(&mut self) -> &mut [Vec3] {
        &mut self.data[..]
    }

    // write as PPM
    pub fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
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

fn main() {
    let mut image = Image::new(100, 100);
    image.set(52, 41, vec3(1.0, 0.0, 0.0));
    let file = File::create("out.ppm").unwrap();
    let mut out = BufWriter::new(file);
    image.write(&mut out).unwrap();
}