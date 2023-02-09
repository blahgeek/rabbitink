use log::{info, trace};
use anyhow::bail;
use opencv as cv;

use super::Source;

struct Shmem {
    id: i32,
    data: *mut libc::c_void,
}

impl Shmem {
    fn new(size: usize) -> anyhow::Result<Shmem> {
        let id = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0777) };
        if id < 0 {
            bail!("failed to create shmem with size {}", size);
        }
        let data = unsafe { libc::shmat(id, std::ptr::null(), 0) };
        assert!(!data.is_null());

        Ok(Shmem {id, data})
    }

    fn id(&self) -> i32 {
        self.id
    }

    fn ptr(&self) -> *mut libc::c_void {
        self.data
    }
}

impl Drop for Shmem {
    fn drop(&mut self) {
        unsafe {
            libc::shmctl(self.id, libc::IPC_RMID, std::ptr::null_mut());
            libc::shmdt(self.data);
        }
    }
}


pub struct XcbGrabSource {
    conn: xcb::Connection,
    window: xcb::x::Window,
    format: xcb::x::Format,

    segment: xcb::shm::Seg,
    shmem: Shmem,

    rect: cv::core::Rect2i,
    cv_color_to_gray: i32,

    grey_frame: cv::core::Mat,
}

impl XcbGrabSource {
    pub fn new(display_name: &str, rect: Option<cv::core::Rect2i>) -> anyhow::Result<XcbGrabSource> {
        let (conn, screen_num) = xcb::Connection::connect(Some(display_name))?;
        let setup = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize).unwrap();
        let window = screen.root();

        let geo_cookie = conn.send_request(&xcb::x::GetGeometry {
            drawable: xcb::x::Drawable::Window(window),
        });
        let geo = conn.wait_for_reply(geo_cookie)?;
        let format = setup.pixmap_formats().iter().find(|f| f.depth() == geo.depth()).unwrap().clone();
        let cv_color_to_gray = match (geo.depth(), format.bits_per_pixel(), setup.image_byte_order()) {
            (32, 32, xcb::x::ImageOrder::LsbFirst) => cv::imgproc::COLOR_BGRA2GRAY,
            (24, 32, xcb::x::ImageOrder::LsbFirst) => cv::imgproc::COLOR_BGRA2GRAY,
            (16, 16, xcb::x::ImageOrder::LsbFirst) => cv::imgproc::COLOR_BGR5652GRAY,
            (15, 16, xcb::x::ImageOrder::LsbFirst) => cv::imgproc::COLOR_BGR5552GRAY,
            v => bail!("unsupported pix fmt: {:?}", v),
        };

        let rect = rect.unwrap_or(cv::core::Rect2i::new(0, 0, geo.width() as i32, geo.height() as i32));
        let frame_size = rect.area() * format.bits_per_pixel() as i32 / 8;

        let segment: xcb::shm::Seg = conn.generate_id();
        let shmem = Shmem::new(frame_size as usize)?;

        conn.send_and_check_request(&xcb::shm::Attach {
            shmseg: segment,
            shmid: shmem.id() as u32,
            read_only: false,
        })?;
        info!("Created XcbGrabSource: {:?}, {:?}, {:?}", window, format, rect);
        Ok(XcbGrabSource { conn, window, format, segment, shmem, rect, cv_color_to_gray, grey_frame: cv::core::Mat::default() })
    }
}

impl Source for XcbGrabSource {
    fn get_frame(&mut self, output_callback: &mut dyn FnMut(&cv::core::Mat) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let image_cookie = self.conn.send_request(&xcb::shm::GetImage {
            drawable: xcb::x::Drawable::Window(self.window),
            x: self.rect.x as i16,
            y: self.rect.y as i16,
            width: self.rect.width as u16,
            height: self.rect.height as u16,
            plane_mask: 0xffffffff,
            format: xcb::x::ImageFormat::ZPixmap as u8,
            shmseg: self.segment,
            offset: 0,
        });
        let image = self.conn.wait_for_reply(image_cookie)?;
        trace!("got image: {:?}", image);
        assert_eq!(image.size() as i32, self.rect.area() * self.format.bits_per_pixel() as i32 / 8);

        let mat_ref = unsafe {cv::core::Mat::new_size_with_data(
            self.rect.size(),
            cv::core::CV_MAKETYPE(cv::core::CV_8U, self.format.bits_per_pixel() as i32 / 8),
            self.shmem.ptr(),
            cv::core::Mat_AUTO_STEP)
        }?;
        cv::imgproc::cvt_color(&mat_ref, &mut self.grey_frame, self.cv_color_to_gray, 0)?;
        output_callback(&self.grey_frame)
    }
}
