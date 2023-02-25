use crate::image::*;

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

fn bgra_to_gray(bgra: *const u8) -> u8 {
    unsafe {
        let r = *bgra.add(2);
        let g = *bgra.add(1);
        let b = *bgra;
        (0.3 * r as f32 + 0.59 * g as f32 + 0.11 * b as f32).clamp(0.0, 255.0) as u8 // Luminosity Method
    }
}

pub fn floyd_steinberg(bgra_src: &impl ConstImage<32>, target_color_space: TargetColorSpace) -> ImageBuffer<8> {
    let mut dst: ImageBuffer<8> = ImageBuffer::new(bgra_src.width(), bgra_src.height(), None);

    let mut current_row_additions: Vec<i32> = vec![0; (bgra_src.width() as usize) + 1];
    let mut next_row_additions: Vec<i32> = vec![0; (bgra_src.width() as usize) + 1];

    for row in 0..bgra_src.height() {
        let src_row_ptr = bgra_src.ptr(row);
        let dst_row_ptr = dst.mut_ptr(row);
        for col in 0..(bgra_src.width() as usize) {
            let src_gray_val = bgra_to_gray(unsafe { src_row_ptr.add(col * 4) });
            let (val, residual) = target_color_space.find_nearest_and_residual(
                (src_gray_val as i32 + current_row_additions[col] / 256)
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
    return dst;
}
