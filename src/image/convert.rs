use super::*;

pub fn repack_mono<const SRC_BPP: i32, const DST_BPP: i32>(src: &impl ConstImage<SRC_BPP>, dst_pitch: i32) -> ImageBuffer<DST_BPP> {
    assert!(DST_BPP > 0 && DST_BPP <= 8);
    assert!(SRC_BPP > 0 && SRC_BPP <= 8);

    let src_ppbyte = 8 / SRC_BPP;  // pixel-per-byte
    let dst_ppbyte = 8 / DST_BPP;  // pixel-per-byte
    let mut dst = ImageBuffer::<DST_BPP>::new(src.width(), src.height(), Some(dst_pitch));
    assert!(dst_pitch * dst_ppbyte >= dst.width());

    for y in 0..src.height() {
        let src_row_ptr = src.ptr(y);
        let dst_row_ptr = dst.mut_ptr(y);
        for x in 0..src.width() {
            unsafe {
                let value =
                    (*src_row_ptr.add((x / src_ppbyte) as usize) >> (x % src_ppbyte)) &
                    (((1u32 << SRC_BPP) - 1) as u8);
                if (value >> (SRC_BPP - 1)) > 0 {
                    *dst_row_ptr.add((x / dst_ppbyte) as usize) |=
                        (((1u32 << DST_BPP) - 1) as u8) << (x % dst_ppbyte);
                }
            }
        }
    }

    return dst;
}
