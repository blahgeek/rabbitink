use opencv as cv;
use opencv::prelude::*;

pub fn pack_image<const BPP: i32>(image: &cv::core::Mat1b, pitch: i32) -> Vec<u8> {
    let ppbyte = 8 / BPP;  // pixel-per-byte
    let mut packed: Vec<u8> = vec![0; (pitch * image.rows()) as usize];

    for y in 0..image.rows() {
        for x in 0..image.cols() {
            packed[(y * pitch + x / ppbyte) as usize] |=
                (image.at_2d::<u8>(y, x).unwrap() >> (8-BPP)) << (x % ppbyte);
        }
    }

    return packed;
}
