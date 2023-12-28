use clap::Parser;
use std::path::PathBuf;

use rabbitink::driver::it8915::{DisplayMode, IT8915, MemMode};
use rabbitink::image::*;
use rabbitink::imgproc::dithering;

use image as imagex; // external, for IO

#[derive(Parser, Debug)]
struct Args {
    image_file: PathBuf,

    #[arg(long, default_value = "")]
    device: String,

    #[arg(short, long, default_value = "gc16")]
    mode: DisplayMode,

    #[arg(long, default_value_t = false)]
    use_1bpp: bool,

    #[arg(long)]
    vcom: f32,

    #[arg(long, default_value_t = false)]
    reset: bool,

    #[arg(long, default_value_t = 0)]
    rotate: u32,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    let mut dev = IT8915::open(&args.device)?;
    dev.pmic_control(Some(args.vcom), None)?;

    if args.reset {
        dev.reset_display()?;
    }

    let mut img_x = imagex::io::Reader::open(&args.image_file).unwrap().decode()?;
    match args.rotate {
        0 => {},
        90 => {
            img_x = img_x.rotate90();
        },
        180 => {
            img_x = img_x.rotate180();
        },
        270 => {
            img_x = img_x.rotate270();
        },
        _ => panic!("invalid rotation angle"),
    }
    img_x = img_x.resize_exact(dev.get_screen_size().width as u32,
                               dev.get_screen_size().height as u32,
                               imagex::imageops::FilterType::Triangle);
    let img_x = img_x.to_rgba8();

    // convert to my own image
    let mut img =
        ImageBuffer::<32>::new(img_x.width() as i32, img_x.height() as i32, None);
    for y in 0..img.height() {
        let ptr = img.mut_ptr(y);
        for x in 0..img.width() {
            let pixel = img_x.get_pixel(x as u32, y as u32);
            unsafe {
                std::slice::from_raw_parts_mut(ptr.add(x as usize * 4), 4)
                    .copy_from_slice(&pixel.0);
            }
        }
    }

    let gray_img = dithering::floyd_steinberg(&img, dithering::GREY16_TARGET_COLOR_SPACE);

    if args.use_1bpp {
        let img = convert::repack_mono::<8, 1>(&gray_img, dev.get_mem_pitch(MemMode::Mem1bpp));
        dev.load_image_fullwidth_1bpp(0, &img)?;
    } else {
        dev.load_image_fullwidth_8bpp(0, &gray_img)?;
    }

    dev.display_area((0, 0).into(), dev.get_screen_size(), args.mode, true)?;
    Ok(())
}
