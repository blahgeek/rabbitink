pub mod dithering;
pub mod rotate;

pub use rotate::Rotation;

use crate::image::Size;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DitheringMethod {
    NoDithering,
    Bayers2,
    Bayers4,
}

#[derive(Clone, Copy, Debug)]
pub struct MonoImgprocOptions {
    pub input_size: Size,
    pub output_size: Size,
    pub rotation: Rotation,
}

pub mod gpu;
pub use gpu::MonoImgproc as MonoImgproc;
