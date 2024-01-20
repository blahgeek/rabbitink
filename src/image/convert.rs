use super::*;

pub fn repack_mono(src: &impl ConstImage, dst_format: ImageFormat, dst_pitch: i32) -> ImageBuffer {
    assert!(src.format() == ImageFormat::Mono1Bpp || src.format() == ImageFormat::Mono8Bpp);
    let src_bpp = src.bpp();
    let dst_bpp = dst_format.bpp();
    assert!(dst_bpp > 0 && dst_bpp <= 8);
    assert!(src_bpp > 0 && src_bpp <= 8);

    let src_ppbyte = 8 / src_bpp;  // pixel-per-byte
    let dst_ppbyte = 8 / dst_bpp;  // pixel-per-byte
    let mut dst = ImageBuffer::new(dst_format, src.width(), src.height(), Some(dst_pitch));
    assert!(dst_pitch * dst_ppbyte >= dst.width());

    for y in 0..src.height() {
        let src_row_ptr = src.ptr(y);
        let dst_row_ptr = dst.mut_ptr(y);
        for x in 0..src.width() {
            unsafe {
                let value =
                    (*src_row_ptr.add((x / src_ppbyte) as usize) >> (x % src_ppbyte)) &
                    (((1u32 << src_bpp) - 1) as u8);
                if (value >> (src_bpp - 1)) > 0 {
                    *dst_row_ptr.add((x / dst_ppbyte) as usize) |=
                        (((1u32 << dst_bpp) - 1) as u8) << (x % dst_ppbyte);
                }
            }
        }
    }

    return dst;
}
