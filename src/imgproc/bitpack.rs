use opencv as cv;
use opencv::prelude::*;

pub fn pack_image<const BPP: i32>(image: &cv::core::Mat1b, pitch: i32) -> Vec<u8> {
    let ppbyte = 8 / BPP;  // pixel-per-byte
    let mut packed: Vec<u8> = vec![0; (pitch * image.rows()) as usize];
    assert!(pitch * ppbyte >= image.cols());

    for y in 0..image.rows() {
        let row_ptr = image.ptr(y).unwrap();
        let packed_row_ptr = unsafe { packed.as_mut_ptr().add((y * pitch) as usize) };
        for x in 0..image.cols() {
            unsafe {
                *packed_row_ptr.add((x / ppbyte) as usize) |=
                    (*(row_ptr.add(x as usize)) >> (8-BPP)) << (x % ppbyte);
            }
        }
    }

    return packed;
}
