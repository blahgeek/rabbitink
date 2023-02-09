use opencv as cv;
use std::{
    ops::Deref,
    sync::{atomic, Arc, Mutex},
};

mod x11grab;
mod xcbgrab;

// Source that produces fixed-size grey (CV_8U) images
// The API is "pull" instead of "push", to allow smallest possible latency
pub trait Source {
    fn get_frame(&mut self, output_callback: &mut dyn FnMut(&cv::core::Mat) -> anyhow::Result<()>) -> anyhow::Result<()>;
}

pub trait PublishingSource {
    fn run(
        &mut self,
        frame_callback: Box<dyn Fn(&cv::core::Mat)>,
        should_quit: Arc<atomic::AtomicBool>,
    ) -> anyhow::Result<()>;
}

pub struct PublishingSourceAdapter<T> {
    publishing_source: Arc<Mutex<T>>,
    thread: Option<std::thread::JoinHandle<()>>,

    latest_result: Arc<Mutex<anyhow::Result<cv::core::Mat>>>,
    should_quit: Arc<atomic::AtomicBool>,
}

impl<T> Source for PublishingSourceAdapter<T> {
    fn get_frame(&mut self, output_callback: &mut dyn FnMut(&cv::core::Mat) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let res = self.latest_result.lock().unwrap();
        let res: &anyhow::Result<cv::core::Mat> = res.deref();
        match &res {
            &Ok(m) => output_callback(m),
            &Err(err) => anyhow::bail!("{}", err),
        }
    }
}

impl<T: PublishingSource + Send + 'static> PublishingSourceAdapter<T> {
    pub fn new(publishing_source: T) -> anyhow::Result<PublishingSourceAdapter<T>> {
        Ok(PublishingSourceAdapter {
            publishing_source: Arc::new(Mutex::new(publishing_source)),
            thread: None,
            latest_result: Arc::new(Mutex::new(Ok(cv::core::Mat::default()))),
            should_quit: Arc::new(atomic::AtomicBool::new(false)),
        })
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        assert!(self.thread.is_none(), "Cannot start twice");
        self.should_quit.store(false, atomic::Ordering::SeqCst);

        let frame_callback = {
            let latest_result = self.latest_result.clone();
            move |frame: &cv::core::Mat| {
                *latest_result.lock().unwrap() = Ok(frame.clone());
            }
        };
        let error_callback = {
            let latest_result = self.latest_result.clone();
            move |err: anyhow::Error| {
                *latest_result.lock().unwrap() = Err(err);
            }
        };

        let publishing_source = self.publishing_source.clone();
        let should_quit = self.should_quit.clone();
        self.thread = Some(std::thread::spawn(move || {
            if let Err(err) = publishing_source.lock().unwrap().run(Box::new(frame_callback), should_quit) {
                error_callback(err);
            }
        }));

        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        assert!(self.thread.is_some(), "Cannot stop before start");
        self.should_quit.store(true, atomic::Ordering::SeqCst);

        let thread = std::mem::replace(&mut self.thread, None);
        thread.unwrap().join().unwrap();
        Ok(())
    }
}

pub use x11grab::X11GrabSource;
pub use xcbgrab::XcbGrabSource;
