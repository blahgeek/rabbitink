use rabbitink::imgproc;

use opencv as cv;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bitpack_bench(c: &mut Criterion) {
    c.bench_function("Pack 1bpp, 1k*1k", |b| {
        let img : cv::core::Mat1b =
            cv::core::Mat::new_rows_cols_with_default(1000, 1000, cv::core::CV_8UC1, cv::core::Scalar::all(42.0))
            .unwrap().try_into_typed().unwrap();
        b.iter(|| black_box(imgproc::bitpack::pack_image::<1>(&img, 1024)));
    });
}

criterion_group!(benches, bitpack_bench);
criterion_main!(benches);
