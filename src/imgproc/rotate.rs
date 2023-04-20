use crate::image::*;

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

pub fn rotate<const BPP: i32, T: ConstImage<BPP> + ?Sized>(input_img: &T, rotation: Rotation) -> ImageBuffer<BPP> {
    assert!(BPP % 8 == 0, "Does not support non-byte-aligned image");

    let output_size = rotation.rotated_size(input_img.size());
    let mut output_img = ImageBuffer::<BPP>::new(output_size.width, output_size.height, None);

    let transform = |x: i32, y: i32| -> (i32, i32) {
        match rotation {
            Rotation::NoRotation => (x, y),
            Rotation::Rotate90 => (y, input_img.height() - 1 - x),
            Rotation::Rotate180 => (input_img.width() - 1 - x, input_img.height() - 1 - y),
            Rotation::Rotate270 => (input_img.width() - 1 - y, x),
        }
    };

    for y in 0..output_size.height {
        let row_ptr = output_img.mut_ptr(y);
        for x in 0..output_size.width {
            let (input_x, input_y) = transform(x, y);
            let input_row_ptr = input_img.ptr(input_y);
            unsafe {
                std::ptr::copy(input_row_ptr.add((input_x * BPP / 8) as usize),
                               row_ptr.add((x * BPP / 8) as usize),
                               (BPP / 8) as usize);
            }
        }
    }

    return output_img;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate90() {
        let input_img_data: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let input_img = ConstImageView::<16>::new(input_img_data.as_slice(), 2, 3, None);

        let output_img = rotate(&input_img, Rotation::Rotate90);
        assert_eq!(output_img.size(), (3, 2).into());
        assert_eq!(output_img.pitch(), 6);

        let output_data = unsafe {
            std::slice::from_raw_parts(output_img.ptr(0) as *const u8, 12)
        };
        assert_eq!(output_data, &[8, 9, 4, 5, 0, 1, 10, 11, 6, 7, 2, 3]);
    }
}
