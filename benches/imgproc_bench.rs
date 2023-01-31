use rabbitink::imgproc;

use opencv as cv;
use opencv::prelude::*;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn new_random_grey_image(size: cv::core::Size2i) -> cv::core::Mat1b {
    let mut img = cv::core::Mat::new_size_with_default(size, cv::core::CV_8UC1, 0.0.into()).unwrap();
    cv::core::randu(&mut img, &cv::core::Scalar::all(0.0), &cv::core::Scalar::all(255.0)).unwrap();
    img.try_into_typed().unwrap()
}

fn bitpack_bench(c: &mut Criterion) {
    c.bench_function("Pack 1bpp, 1k*1k", |b| {
        let img = new_random_grey_image((1000, 1000).into());
        b.iter(|| black_box(imgproc::bitpack::pack_image::<1>(&img, 128)));
    });
    c.bench_function("Pack 8bpp, 1k*1k", |b| {
        let img = new_random_grey_image((1000, 1000).into());
        b.iter(|| black_box(imgproc::bitpack::pack_image::<1>(&img, 1024)));
    });
}

fn dithering_bench(c: &mut Criterion) {
    c.bench_function("BW floyd steinberg dithering, 1k*1k", |b| {
        let img = new_random_grey_image((1000, 1000).into());
        b.iter(|| black_box(imgproc::dithering::floyd_steinberg(&img, imgproc::dithering::BW_TARGET_COLOR_SPACE)));
    });
}

criterion_group!(benches, bitpack_bench, dithering_bench);
criterion_main!(benches);
