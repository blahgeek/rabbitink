use opencv as cv;
use opencv::prelude::*;

use super::*;

pub fn cvmat_image_view<const BPP: i32>(m: &cv::core::Mat) -> ConstImageView<BPP> {
    assert!(m.elem_size().unwrap() * 8 == BPP as usize);
    assert!(m.is_continuous());
    let data = m.data_bytes().unwrap();
    ConstImageView::new(data, m.cols(), m.rows(), None)
}
