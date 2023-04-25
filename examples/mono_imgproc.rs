use rabbitink::image::*;
use rabbitink::imgproc::{MonoImgproc, MonoImgprocOptions, DitheringMethod, Rotation};

use image as imagex; // external, for IO

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let src_img_x = imagex::io::Reader::open(std::env::args().nth(1).unwrap())?
        .decode()?
        .into_rgba8();
    let mut src_img =
        ImageBuffer::<32>::new(src_img_x.width() as i32, src_img_x.height() as i32, None);

    for y in 0..src_img.height() {
        let ptr = src_img.mut_ptr(y);
        for x in 0..src_img.width() {
            let pixel = src_img_x.get_pixel(x as u32, y as u32);
            unsafe {
                std::slice::from_raw_parts_mut(ptr.add(x as usize * 4), 4)
                    .copy_from_slice(&pixel.0);
            }
        }
    }

    let mut dst_img = ImageBuffer::<1>::new(src_img.width(), src_img.height(), None);

    let mut imgproc = MonoImgproc::new(MonoImgprocOptions {
        input_size: src_img.size(),
        output_size: dst_img.size(),
        rotation: Rotation::NoRotation,
    });

    for _ in 0..10 {
        imgproc.process(&src_img, &mut dst_img, DitheringMethod::Bayers4);
    }

    let mut dst_img_x = imagex::GrayImage::new(dst_img.width() as u32, dst_img.height() as u32);
    for y in 0..dst_img.height() {
        let ptr = dst_img.mut_ptr(y);
        for x in 0..dst_img.width() {
            let pixel = if unsafe { ((*ptr.add(x as usize / 8) >> (x % 8)) & 0x1) == 1 } {
                0xff
            } else {
                0
            };
            dst_img_x.put_pixel(x as u32, y as u32, imagex::Luma([pixel]));
        }
    }
    dst_img_x.save(std::env::args().nth(2).unwrap())?;

    Ok(())
}
