use rabbitink::vncclient::*;

use opencv as cv;
use opencv::prelude::*;
use log::trace;
use clap::Parser;


#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    host: String,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();

    let args = Args::parse();

    let mut client = VNCClient::new(InitOptions { host: args.host, pixel_format: PIXEL_FORMAT_BGR555 })?;
    trace!("client created, start loop");
    loop {
        let wait_res = client.wait_for_message(std::time::Duration::from_secs(1))?;
        trace!("wait for message: {}", wait_res);
        if wait_res > 0 {
            client.handle_message()?;
            trace!("handle for message done");
        }
        let framebuf = client.get_frame_buffer();
        trace!("framebuf: {:?}", framebuf.size());
        if !framebuf.empty() {
            let mut rgb_image = cv::core::Mat::default();
            cv::imgproc::cvt_color(framebuf, &mut rgb_image, cv::imgproc::COLOR_BGR5552BGR, 0)?;
            cv::imgcodecs::imwrite("/tmp/vncclient.png", &rgb_image, &cv::core::Vector::default())?;
        }
    }

    // Ok(())
}
