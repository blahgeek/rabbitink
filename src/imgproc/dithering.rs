use opencv as cv;
use opencv::core::{Mat, Mat1b};
use opencv::prelude::*;

pub struct TargetColorSpace {
    step: u8,
    n_levels: u8,
}

impl TargetColorSpace {
    fn find_nearest_and_residual(&self, src: u8) -> (u8, u8) {
        let level = (src / (self.step / 2) / 2).min(self.n_levels - 1);
        let dst = level * self.step;
        (dst, src.saturating_sub(dst))
    }
}

pub const BW_TARGET_COLOR_SPACE: TargetColorSpace = TargetColorSpace {
    step: 0xf0,
    n_levels: 2,
};

pub const GREY16_TARGET_COLOR_SPACE: TargetColorSpace = TargetColorSpace {
    step: 0x10,
    n_levels: 16,
};


pub fn floyd_steinberg(grey_src: &Mat1b, target_color_space: TargetColorSpace) -> Mat1b {
    // new_size without default is unsafe. but we will fill it soon
    let mut dst: Mat = unsafe { Mat::new_size(grey_src.size().unwrap(), cv::core::CV_8UC1).unwrap() };

    let mut current_row_additions: Vec<i32> = vec![0; (grey_src.cols() as usize) + 1];
    let mut next_row_additions: Vec<i32> = vec![0; (grey_src.cols() as usize) + 1];

    for row in 0..grey_src.rows() {
        let src_row_ptr = grey_src.ptr(row).unwrap();
        let dst_row_ptr = dst.ptr_mut(row).unwrap();
        for col in 0..(grey_src.cols() as usize) {
            let (val, residual) = target_color_space.find_nearest_and_residual(
                (unsafe{*src_row_ptr.add(col)} as i32 + current_row_additions[col] / 256)
                    .clamp(0, 255) as u8);
            unsafe{ *dst_row_ptr.add(col) = val; }

            // row_additions are scaled by 256 to preserve precision
            let residual_16th = residual as i32 * 16;
            current_row_additions[col+1] += residual_16th * 7;
            if col >= 1 {
                next_row_additions[col-1] += residual_16th * 3;
            }
            next_row_additions[col] += residual_16th * 5;
            next_row_additions[col+1] += residual_16th;
        }
        std::mem::swap(&mut current_row_additions, &mut next_row_additions);
        next_row_additions.fill(0);
    }
    dst.try_into_typed().unwrap()
}
