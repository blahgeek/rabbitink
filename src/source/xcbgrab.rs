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

    fn mut_slice(&self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.data as *mut u8, self.size) }
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

    screensave_img: ImageBuffer<32>,
}

fn make_screensave_img(size: Size) -> ImageBuffer<32> {
    let mut img = ImageBuffer::<32>::new(size.width, size.height, None);
    img.fill(0xff);
    return img
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
            screensave_img: make_screensave_img(size),
        })
    }

    fn draw_cursor(
        &self,
        img: &mut impl Image<32>,
        cursor_image: xcb::xfixes::GetCursorImageReply,
    ) {
        let cx = cursor_image.x() as i32 - cursor_image.xhot() as i32;
        let cy = cursor_image.y() as i32 - cursor_image.yhot() as i32;

        let x = i32::max(cx, self.top_left.x);
        let y = i32::max(cy, self.top_left.y);
        let w = i32::min(
            cx + cursor_image.width() as i32,
            self.top_left.x + self.size.width,
        ) - x;
        let h = i32::min(
            cy + cursor_image.height() as i32,
            self.top_left.y + self.size.height,
        ) - y;

        for j in 0..h {
            let c_ptr = unsafe {
                cursor_image.cursor_image().as_ptr().add(
                    cursor_image.width() as usize * (j + (y - cy)) as usize + (x - cx) as usize,
                )
            };
            let i_ptr = unsafe {
                let p = img.mut_ptr(j + y - self.top_left.y).cast::<u32>();
                p.add((x - self.top_left.x) as usize)
            };
            for i in 0..w {
                unsafe {
                    let cursor_rgba = *c_ptr.add(i as usize);
                    *i_ptr.add(i as usize) = blend_rgba(cursor_rgba, *i_ptr.add(i as usize));
                }
            }
        }
    }
}

fn blend_rgba(cursor: u32, img: u32) -> u32 {
    let alpha = (cursor >> 24) & 0xff;
    return blend(cursor & 0xff, img & 0xff, alpha)
        | (blend((cursor >> 8) & 0xff, (img >> 8) & 0xff, alpha) << 8)
        | (blend((cursor >> 16) & 0xff, (img >> 16) & 0xff, alpha) << 16);
}

fn blend(cursor: u32, img: u32, alpha: u32) -> u32 {
    (cursor + (img * (255 - alpha) + 255 / 2) / 255).clamp(0, 255)
}

impl Source for XcbGrabSource {
    fn get_frame(&mut self) -> anyhow::Result<ConstImageView<32>> {
        let screensaver_query_cookie = self.conn.send_request(&xcb::screensaver::QueryInfo {
            drawable: xcb::x::Drawable::Window(self.window),
        });
        let screensaver_queryinfo = self.conn.wait_for_reply(screensaver_query_cookie)?;
        trace!("got screensaver info: {:?}", screensaver_queryinfo);
        if screensaver_queryinfo.state() == xcb::screensaver::State::On as u8 {
            return Ok(self.screensave_img.view());
        }

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

        let cursor_cookie = self.conn.send_request(&xcb::x::QueryPointer {
            window: self.window,
        });
        let cursor = self.conn.wait_for_reply(cursor_cookie)?;
        trace!("got cursor: {:?}", cursor);
        if cursor.same_screen() {
            let cursor_image_cookie = self.conn.send_request(&xcb::xfixes::GetCursorImage {});
            let cursor_image = self.conn.wait_for_reply(cursor_image_cookie)?;
            let mut image = ImageView::<32>::new(
                self.shmem.mut_slice(),
                self.size.width,
                self.size.height,
                Some(self.size.width * 4),
            );
            self.draw_cursor(&mut image, cursor_image);
        }

        Ok(ConstImageView::<32>::new(
            self.shmem.slice(),
            self.size.width,
            self.size.height,
            Some(self.size.width * 4),
        ))
    }
}
