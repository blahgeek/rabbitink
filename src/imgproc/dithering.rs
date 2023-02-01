use opencv as cv;
use opencv::core::{Mat, Mat1b};
use opencv::prelude::*;

pub struct TargetColorSpace {
    base: u8,
    step: u8,
    n_levels: u8,
}

impl TargetColorSpace {
    fn find_nearest_and_residual(&self, src: f32) -> (u8, f32) {
        let src = src.min(255.0).max(0.0);
        let level = ((src - (self.base as f32)) / (self.step as f32) - 0.5)
            .round().min((self.n_levels - 1) as f32).max(0.0) as u8;
        let dst = self.base + level * self.step;
        (dst, src - (dst as f32))
    }
}

pub const BW_TARGET_COLOR_SPACE: TargetColorSpace = TargetColorSpace {
    base: 0,
    step: 0xf0,
    n_levels: 2,
};

pub const GREY16_TARGET_COLOR_SPACE: TargetColorSpace = TargetColorSpace {
    base: 0,
    step: 0x10,
    n_levels: 16,
};


pub fn floyd_steinberg(grey_src: &Mat1b, target_color_space: TargetColorSpace) -> Mat1b {
    // new_size without default is unsafe. but we will fill it soon
    let mut dst: Mat = unsafe { Mat::new_size(grey_src.size().unwrap(), cv::core::CV_8UC1).unwrap() };

    let mut current_row_additions: Vec<f32> = vec![0.0; (grey_src.cols() as usize) + 1];
    let mut next_row_additions: Vec<f32> = vec![0.0; (grey_src.cols() as usize) + 1];

    for row in 0..grey_src.rows() {
        let src_row_ptr = grey_src.ptr(row).unwrap();
        let dst_row_ptr = dst.ptr_mut(row).unwrap();
        for col in 0..(grey_src.cols() as usize) {
            let (val, residual) = target_color_space.find_nearest_and_residual(
                (unsafe{*src_row_ptr.add(col)} as f32) +
                    current_row_additions[col]);
            unsafe{ *dst_row_ptr.add(col) = val; }

            current_row_additions[col+1] += residual * 7.0 / 16.0;
            if col >= 1 {
                next_row_additions[col-1] += residual * 3.0 / 16.0;
            }
            next_row_additions[col] += residual * 5.0 / 16.0;
            next_row_additions[col+1] += residual * 1.0 / 16.0;
        }
        std::mem::swap(&mut current_row_additions, &mut next_row_additions);
        next_row_additions.fill(0.0);
    }
    dst.try_into_typed().unwrap()
}
