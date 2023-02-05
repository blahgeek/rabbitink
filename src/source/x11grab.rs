use opencv as cv;
use std::io::Read;
use cv::prelude::{MatTraitConst, MatTrait};
use super::PublishingSource;

pub struct X11GrabSource {
    pub width: i32,
    pub height: i32,
    pub framerate: i32,
    pub input: String,
}

impl PublishingSource for X11GrabSource {
    fn run(
        &mut self,
        frame_callback: Box<dyn Fn(&opencv::core::Mat)>,
        should_quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> anyhow::Result<()> {
        let mut child = std::process::Command::new("ffmpeg")
            .arg("-framerate").arg(&self.framerate.to_string())
            .arg("-video_size").arg(&format!("{}x{}", self.width, self.height))
            .arg("-f").arg("x11grab")
            .arg("-i").arg(&self.input)
            .arg("-vf").arg("format=gray8")
            .arg("-vsync").arg("0")
            .arg("-f").arg("rawvideo").arg("-")
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        {
            let stdout: &mut std::process::ChildStdout = child.stdout.as_mut().unwrap();
            let mut buffer = cv::core::Mat::new_size_with_default(
                (self.width, self.height).into(), cv::core::CV_8UC1, 0.into())?;
            let buffer_slice = unsafe {
                std::slice::from_raw_parts_mut(
                    buffer.ptr_mut(0).unwrap(),
                    (buffer.cols() * buffer.rows()) as usize)
            };

            assert!(buffer.is_continuous());
            while !should_quit.load(std::sync::atomic::Ordering::SeqCst) {
                stdout.read_exact(buffer_slice)?;
                frame_callback(&buffer);
            }
        }

        child.kill()?;
        child.wait()?;

        Ok(())
    }
}
