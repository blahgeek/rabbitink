use super::*;

pub fn pack<const BPP: i32>(image: &impl ConstImage<8>, pitch: i32) -> ImageBuffer<BPP> {
    assert!(BPP > 0 && BPP <= 8);

    let ppbyte = 8 / BPP;  // pixel-per-byte
    let mut packed = ImageBuffer::<BPP>::new(image.width(), image.height(), Some(pitch));
    assert!(pitch * ppbyte >= image.width());

    for y in 0..image.height() {
        let row_ptr = image.ptr(y);
        let packed_row_ptr = packed.mut_ptr(y);
        for x in 0..image.width() {
            unsafe {
                *packed_row_ptr.add((x / ppbyte) as usize) |=
                    (*(row_ptr.add(x as usize)) >> (8-BPP)) << (x % ppbyte);
            }
        }
    }

    return packed;
}
