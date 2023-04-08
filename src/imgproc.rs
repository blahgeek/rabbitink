pub mod dithering;

use crate::image::{Size, ConstImage, Image};

#[derive(Clone, Copy, Debug, PartialEq)]
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
}


pub trait MonoImgprocTrait {
    fn new(options: MonoImgprocOptions) -> Self;
    fn process(&mut self,
               input_bgra_img: &impl ConstImage<32>,
               output_bw_img: &mut impl Image<1>,
               dithering_method: DitheringMethod);
}

// TODO: implement a real CPU mono imgproc
pub struct DummyMonoImgproc {}

impl MonoImgprocTrait for DummyMonoImgproc {
    fn new(_: MonoImgprocOptions) -> Self {
        DummyMonoImgproc {}
    }
    fn process(&mut self,
               _: &impl ConstImage<32>,
               _: &mut impl ConstImage<1>,
               _: DitheringMethod) {
        unimplemented!()
    }
}

#[cfg(feature = "wgpu")]
pub mod gpu;
#[cfg(feature = "wgpu")]
pub use gpu::GpuMonoImgproc as MonoImgproc;

#[cfg(not(feature = "wgpu"))]
pub use DummyMonoImgproc as MonoImgproc;
