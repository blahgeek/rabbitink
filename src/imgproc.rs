pub mod dithering;

use crate::image::Size;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DitheringMethod {
    NoDithering,
    Bayers2,
    Bayers4,
}

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum Rotation {
    NoRotation,
    Rotate90,
    Rotate180,
    Rotate270,
}

impl Rotation {
    pub fn rotated_size(&self, size: Size) -> Size {
        match self {
            Rotation::NoRotation | Rotation::Rotate180 => size,
            Rotation::Rotate90 | Rotation::Rotate270 => (size.height, size.width).into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MonoImgprocOptions {
    pub input_size: Size,
    pub input_pitch: i32,
    pub output_size: Size,
    pub output_pitch: i32,
    pub rotation: Rotation,
}

pub mod gpu;
pub use gpu::MonoImgproc as MonoImgproc;
