pub mod gpu;

use crate::image::Size;

#[derive(Clone, Copy, Debug)]
pub struct ImgprocOptions {
    pub image_size: Size,
    pub bgra_pitch: i32,
    pub bw_pitch: i32,
}

pub use gpu::GpuImgproc as Imgproc;
