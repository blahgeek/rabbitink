pub mod gpu;

use crate::image::Size;

#[derive(Clone, Copy, Debug)]
pub enum DitheringMethod {
    NoDithering,
    Bayers2,
    Bayers4,
}

#[derive(Clone, Copy, Debug)]
pub struct MonoImgprocOptions {
    pub image_size: Size,
    pub bgra_pitch: i32,
    pub bw_pitch: i32,
    pub dithering_method: DitheringMethod,
}

pub use gpu::GpuMonoImgproc as MonoImgproc;
