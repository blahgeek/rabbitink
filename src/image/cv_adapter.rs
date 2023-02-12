use opencv as cv;
use opencv::prelude::*;

use super::*;

pub fn cvmat_image_view<const BPP: i32>(m: &cv::core::Mat) -> ConstImageView<BPP> {
    assert!(m.elem_size().unwrap() * 8 == BPP as usize);
    assert!(m.is_continuous());
    let data = m.data_bytes().unwrap();
    ConstImageView::new(data, m.cols(), m.rows(), None)
}

pub fn cvmat_from_image<const BPP: i32>(img: &impl ConstImage<BPP>) -> cv::core::Mat {
    let cv_type = match BPP {
        8 => cv::core::CV_8UC1,
        16 => cv::core::CV_8UC2,
        24 => cv::core::CV_8UC3,
        32 => cv::core::CV_8UC4,
        _ => panic!("unsupported image"),
    };
    unsafe {
        cv::core::Mat::new_rows_cols_with_data(
            img.height(),
            img.width(),
            cv_type,
            img.ptr(0) as *mut libc::c_void,
            img.pitch() as usize,
        )
        .unwrap()
    }
    .clone()
}
