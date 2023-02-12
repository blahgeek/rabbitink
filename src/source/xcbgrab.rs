use anyhow::bail;
use log::{info, trace};

use super::Source;
use crate::image::*;

struct Shmem {
    id: i32,
    size: usize,
    data: *mut libc::c_void,
}

impl Shmem {
    fn new(size: usize) -> anyhow::Result<Shmem> {
        let id = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0b111_111_111) };
        if id < 0 {
            bail!("failed to create shmem with size {}", size);
        }
        let data = unsafe { libc::shmat(id, std::ptr::null(), 0) };
        assert!(!data.is_null());

        Ok(Shmem { id, size, data })
    }

    fn id(&self) -> i32 {
        self.id
    }

    fn size(&self) -> usize {
        self.size
    }

    fn slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data as *const u8, self.size) }
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

    segment: xcb::shm::Seg,
    shmem: Shmem,

    top_left: Point,
    size: Size,
}

impl XcbGrabSource {
    pub fn new(display_name: &str, rect: Option<(Point, Size)>) -> anyhow::Result<XcbGrabSource> {
        let (conn, screen_num) = xcb::Connection::connect(Some(display_name))?;
        let setup = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize).unwrap();
        let window = screen.root();

        let geo_cookie = conn.send_request(&xcb::x::GetGeometry {
            drawable: xcb::x::Drawable::Window(window),
        });
        let geo = conn.wait_for_reply(geo_cookie)?;
        let format = setup
            .pixmap_formats()
            .iter()
            .find(|f| f.depth() == geo.depth())
            .unwrap()
            .clone();
        if format.bits_per_pixel() != 32 {
            bail!("Unsupported format: {:?}", format);
        }

        let (top_left, size) = rect.unwrap_or((
            (0, 0).into(),
            (geo.width() as i32, geo.height() as i32).into(),
        ));
        let frame_size = size.width * size.height * 4;

        let segment: xcb::shm::Seg = conn.generate_id();
        let shmem = Shmem::new(frame_size as usize)?;

        conn.send_and_check_request(&xcb::shm::Attach {
            shmseg: segment,
            shmid: shmem.id() as u32,
            read_only: false,
        })?;
        info!(
            "Created XcbGrabSource: {:?}, {:?}, {:?}",
            window, format, rect
        );
        Ok(XcbGrabSource {
            conn,
            window,
            segment,
            shmem,
            top_left,
            size,
        })
    }
}

impl Source for XcbGrabSource {
    fn get_frame(&mut self) -> anyhow::Result<ConstImageView<32>> {
        let image_cookie = self.conn.send_request(&xcb::shm::GetImage {
            drawable: xcb::x::Drawable::Window(self.window),
            x: self.top_left.x as i16,
            y: self.top_left.y as i16,
            width: self.size.width as u16,
            height: self.size.height as u16,
            plane_mask: 0xffffffff,
            format: xcb::x::ImageFormat::ZPixmap as u8,
            shmseg: self.segment,
            offset: 0,
        });
        let image = self.conn.wait_for_reply(image_cookie)?;
        trace!("got image: {:?}", image);
        assert_eq!(image.size() as usize, self.shmem.size());

        Ok(ConstImageView::<32>::new(
            self.shmem.slice(),
            self.size.width,
            self.size.height,
            Some(self.size.width * 4),
        ))
    }
}
