use rabbitink::imgproc::dithering;

use opencv::prelude::*;
use opencv as cv;


fn main() {
    env_logger::init();

    let src = cv::imgcodecs::imread(&std::env::args().nth(1).unwrap(),
                                    cv::imgcodecs::IMREAD_GRAYSCALE).unwrap();
    let dst = dithering::floyd_steinberg(&src.try_into_typed().unwrap(),
                                         dithering::GREY16_TARGET_COLOR_SPACE);
    cv::imgcodecs::imwrite(&std::env::args().nth(2).unwrap(), &dst,
                           &cv::core::Vector::new()).unwrap();
}
