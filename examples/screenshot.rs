use opencv as cv;
use cv::prelude::*;

use rabbitink::image::*;
use rabbitink::source::{Source, XcbGrabSource};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let mut source = XcbGrabSource::new(":0.0", None)?;
    let frame = source.get_frame()?;

    // set alpha
    let mut cv_img = cv_adapter::cvmat_from_image(&frame);
    for y in 0..cv_img.rows() {
        for x in 0..cv_img.cols() {
            cv_img.at_2d_mut::<cv::core::Vec4b>(y, x)?.0[3] = 255;
        }
    }

    cv::imgcodecs::imwrite(
        &std::env::args().nth(1).unwrap(),
        &cv_img,
        &cv::core::Vector::default(),
    )?;

    Ok(())
}
