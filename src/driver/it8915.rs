use std::{path::Path, fmt::Debug};

use log::{trace, info};
use opencv::prelude::*;
use opencv as cv;
use anyhow::Context;

use super::serde::{BigEndianU16, BigEndianU32};
use super::scsi;
use crate::imgproc::bitpack;

const LOAD_IMAGE_MAX_TRANSFER_SIZE: i32 = 60800;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum DisplayMode {
    INIT = 0,
    DU,
    GC16,
    GL16,
    GLR16,
    GLD16,
    A2,
    DU4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMode {
    Default8bpp,
    Pack1bpp,
}

#[repr(packed)]
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
struct Sysinfo {
    standard_cmd_no: BigEndianU32,
    extend_cmd_no: BigEndianU32,
    signature: BigEndianU32,
    version: BigEndianU32,
    width: BigEndianU32,
    height: BigEndianU32,
    update_buf_base: BigEndianU32,
    image_buf_base: BigEndianU32,
    temperature_no: BigEndianU32,
    mode_no: BigEndianU32,
    frame_count: [BigEndianU32; 8],
    num_img_buf: BigEndianU32,
    reserved: [u32; 9],
}

#[repr(packed)]
#[allow(dead_code)]
#[derive(Default)]
struct MemIOCmd {
    hdr: u8,
    padding0: u8,
    addr: BigEndianU32,
    cmd: u8,
    len: BigEndianU16,
    padding1: [u8; 7],
}

#[repr(packed)]
#[derive(Default)]
#[allow(dead_code)]
struct PMICControlCmd {
    hdr: u8,
    padding1: [u8; 5],
    cmd: u8,
    vcom: BigEndianU16,
    vcom_set: u8,
    power_set: u8,
    power: u8,
    padding2: u32,
}

#[repr(packed)]
#[allow(dead_code)]
struct LoadImageAreaArgs {
    addr: BigEndianU32,
    x: BigEndianU32,
    y: BigEndianU32,
    w: BigEndianU32,
    h: BigEndianU32,
}

#[repr(packed)]
#[allow(dead_code)]
struct DisplayAreaArgs {
    addr: BigEndianU32,
    mode: BigEndianU32,
    x: BigEndianU32,
    y: BigEndianU32,
    w: BigEndianU32,
    h: BigEndianU32,
    wait_ready: BigEndianU32,
}


pub struct IT8915 {
    device: scsi::Device,
    sysinfo: Sysinfo,

    mem_mode: MemoryMode,
    mem_pitch: u32,
}

const EXPECT_INQUERY_VENDOR_PRODUCT : &'static str = "Generic Storage RamDisc 1.00";


impl IT8915 {
    pub fn screen_size(&self) -> (i32, i32) {
        (self.sysinfo.width.val() as i32, self.sysinfo.height.val() as i32)
    }

    pub fn open(path: &Path) -> anyhow::Result<IT8915> {
        let mut device = scsi::Device::open(path)?;

        // inquery, check vendor
        let mut inquery_cmd = [0_u8; 16];
        inquery_cmd[0] = 0x12;
        let mut inquery_result_buf = [0_u8; 40];
        device.io_read(&inquery_cmd, &mut inquery_result_buf)
            .context("failed to inquery")?;
        let inquery_result_vendor_product = String::from_utf8_lossy(&inquery_result_buf[8..36]);
        trace!("Inquery result: {}", inquery_result_vendor_product);
        if inquery_result_vendor_product != EXPECT_INQUERY_VENDOR_PRODUCT {
            anyhow::bail!("unexpected vendor product string: {}", inquery_result_vendor_product);
        }

        let sysinfo_cmd: [u8; 16] =
            [0xfe, 0x00,
             0x38, 0x39, 0x35, 0x31,  // "8951"
             0x80, 0x00,
             0x01, 0x00, 0x02, 0x00,  // version: 0x00010002
             0x00, 0x00, 0x00, 0x00];
        let mut sysinfo = Sysinfo::default();
        device.io_read(&sysinfo_cmd, &mut sysinfo).context("failed to read sysinfo")?;
        trace!("Sysinfo: {:?}", sysinfo);

        let mut res = IT8915 {
            device,
            sysinfo,
            mem_mode: MemoryMode::Default8bpp,
            mem_pitch: sysinfo.width.val(),
        };
        res.set_memory_mode(MemoryMode::Default8bpp)?;
        Ok(res)
    }

    pub fn set_memory_mode(&mut self, mode: MemoryMode) -> anyhow::Result<()> {
        self.mem_mode = mode;

        // Enable/Disable 1bit drawing and image pitch mode
        // 0000 0000 0000 0110 0000 0000 0000 0000
        // |         |     ^^  |         |
        // 113B      113A      1139      1138
        let mut up1sr = self.read_mem::<4>(0x1800_1138)?;
        match mode {
            MemoryMode::Pack1bpp => up1sr[2] |= 0x06,
            MemoryMode::Default8bpp => up1sr[2] &= !0x06,
        }
        self.write_mem(0x1800_1138, &up1sr)?;

        // Set bitmap mode color definition (0 - set black(0x00), 1 - set white(0xf0))
        self.write_mem(0x1800_1250, match mode {
            MemoryMode::Pack1bpp => &[0xf0, 0x00],
            _ => &[0x00, 0x00],
        })?;

        self.mem_pitch = match mode {
            // 4 byte align
            MemoryMode::Pack1bpp => (self.screen_size().0 as u32 + 31) / 8,
            _ => self.screen_size().0 as u32,
        };
        // (not sure about why the "/4"... apparently the reg is in double-word)
        self.write_mem(0x1800_124c, &[((self.mem_pitch / 4) & 0xff) as u8,
                                      (((self.mem_pitch / 4) >> 8) & 0xff) as u8])?;

        Ok(())
    }

    pub fn reset_display(&mut self) -> anyhow::Result<()> {
        let (w, h) = self.screen_size();
        let white_img: cv::core::Mat1b =
            cv::core::Mat::new_rows_cols_with_default(
                h, w,
                opencv::core::CV_8U,
                opencv::core::Scalar::all(0xf0 as f64))?
            .try_into_typed()?;
        // although INIT would flush the display regardless of the memory content,
        // if we don't initialize the memory content, the following display cannot work correctly,
        // apparently they would depend on the last state.
        self.load_image_fullwidth(0, &white_img)?;
        self.display_area(cv::core::Rect2i::new(0, 0, w, h), DisplayMode::INIT, true)
    }

    pub fn pmic_control(&mut self, vcom: Option<u16>, power: Option<bool>) -> anyhow::Result<()> {
        let mut cmd = PMICControlCmd {
            hdr: 0xfe,
            cmd: 0xa3,
            ..PMICControlCmd::default()
        };
        if let Some(vcom) = vcom {
            cmd.vcom = BigEndianU16::from(vcom);
            cmd.vcom_set = 1;
            info!("Setting VCOM value: {}", vcom);
        }
        if let Some(power) = power {
            cmd.power_set = 1;
            cmd.power = if power { 1 } else { 0 };
            info!("Setting power: {}", power);
        }
        self.device.io_write(&cmd, &())?;
        Ok(())
    }

    pub fn read_mem<const LEN: usize>(&mut self, addr: u32) -> anyhow::Result<[u8; LEN]> {
        let mut res : [u8; LEN] = [0_u8; LEN];
        let cmd = MemIOCmd {
            hdr: 0xfe,
            addr: BigEndianU32::from(addr),
            cmd: 0x81,
            len: BigEndianU16::from(u16::try_from(LEN).expect("read_mem buf too long")),
            ..MemIOCmd::default()
        };
        self.device.io_read(&cmd, &mut res)?;
        Ok(res)
    }

    pub fn read_busy_state(&mut self) -> anyhow::Result<bool> {
        let res = self.read_mem::<2>(0x18001224)?;   // LUTAFSR + 0x18000000
        trace!("Read busy state: {:?}", res);
        let busy = res.iter().any(|x| *x != 0);
        Ok(busy)
    }

    fn write_mem(&mut self, addr: u32, data: &[u8]) -> anyhow::Result<()> {
        let cmd = MemIOCmd {
            hdr: 0xfe,
            addr: BigEndianU32::from(addr),
            cmd: 0x82,
            len: BigEndianU16::from(u16::try_from(data.len()).expect("write_mem buf too long")),
            ..MemIOCmd::default()
        };
        self.device.io_write_gather(&cmd, &[(data.as_ptr(), data.len())])?;
        Ok(())
    }

    fn write_mem_fast(&mut self, addr: u32, data: &[u8]) -> anyhow::Result<()> {
        let cmd = MemIOCmd {
            hdr: 0xfe,
            addr: BigEndianU32::from(addr),
            cmd: 0xa5,
            len: BigEndianU16::from(u16::try_from(data.len()).expect("write_mem_fast data too long")),
            ..MemIOCmd::default()
        };
        self.device.io_write_gather(&cmd, &[(data.as_ptr(), data.len())])?;
        Ok(())
    }

    // make sure that image size is within max transfer size
    fn load_image_area_onestep(&mut self, pos: (u32, u32), image: cv::core::Mat1b) -> anyhow::Result<()> {
        trace!("Loading image slice to pos {:?}, image size=({}, {})",
               pos, image.cols(), image.rows());
        let cmd: [u8; 16] = [
            0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa2, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let args = LoadImageAreaArgs {
            addr: self.sysinfo.image_buf_base,
            x: BigEndianU32::from(pos.0),
            y: BigEndianU32::from(pos.1),
            w: BigEndianU32::from(image.cols() as u32),
            h: BigEndianU32::from(image.rows() as u32),
        };

        let mut data_iovec: Vec<(*const u8, usize)> = vec![
            ((&args as *const LoadImageAreaArgs) as *const u8, std::mem::size_of::<LoadImageAreaArgs>()),
        ];
        for row in 0..image.rows() {
            data_iovec.push((image.ptr(row).unwrap(), image.cols() as usize));
        }
        self.device.io_write_gather(&cmd, &data_iovec)?;
        Ok(())
    }

    pub fn load_image_area(&mut self, pos: (u32, u32), image: &cv::core::Mat1b) -> anyhow::Result<()> {
        // fast path, if the image is full width
        if pos.0 == 0 && image.cols() == self.screen_size().0 {
            return self.load_image_fullwidth(pos.1, image);
        }

        // slow path, only support 8bpp mode
        assert_eq!(self.mem_mode, MemoryMode::Default8bpp);

        let (canvas_w, canvas_h) = (self.sysinfo.width.val(), self.sysinfo.height.val());
        trace!("Loading image to pos {:?}, image size=({}, {})",
               pos, image.cols(), image.rows());
        if pos.0 + (image.cols() as u32) > canvas_w || pos.1 + (image.rows() as u32) > canvas_h {
            anyhow::bail!("load image too large: pos={:?}, image size=({}, {})",
                          pos, image.cols(), image.rows());
        }
        let rows_per_step = LOAD_IMAGE_MAX_TRANSFER_SIZE / image.cols();
        assert!(rows_per_step > 0);

        let mut row = 0_i32;
        while row < image.rows() {
            let subimg = image.row_bounds(row, i32::min(row + rows_per_step, image.rows())).unwrap();
            self.load_image_area_onestep((pos.0, pos.1 + row as u32), subimg.try_into_typed().unwrap())?;
            row += rows_per_step;
        }

        Ok(())
    }

    // faster than load_image_area, but the image must cover full width
    fn load_image_fullwidth(&mut self, row_offset: u32, image: &cv::core::Mat1b) -> anyhow::Result<()> {
        assert_eq!(image.cols(), self.screen_size().0);

        let rows_per_step = ((u16::MAX as u32) / self.mem_pitch) as i32;
        let mut row = 0;
        while row < image.rows() {
            let subimg : cv::core::Mat1b =
                image.row_bounds(row, i32::min(row + rows_per_step, image.rows()))?.try_into_typed()?;
            let packed = match self.mem_mode {
                MemoryMode::Pack1bpp => bitpack::pack_image::<1>(&subimg, self.mem_pitch as i32),
                MemoryMode::Default8bpp => bitpack::pack_image::<8>(&subimg, self.mem_pitch as i32),
            };
            self.write_mem_fast(self.sysinfo.image_buf_base.val() + self.mem_pitch * (row as u32 + row_offset),
                                &packed)?;

            row += rows_per_step;
        }

        Ok(())
    }

    pub fn display_area(&mut self, region: cv::core::Rect2i, mode: DisplayMode, wait_ready: bool) -> anyhow::Result<()> {
        trace!("Displaying region {:?}, mode = {:?}", region, mode);
        let cmd: [u8; 16] = [
            0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0x94, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let args = DisplayAreaArgs {
            addr: self.sysinfo.image_buf_base,
            mode: BigEndianU32::from(mode as u32),
            x: BigEndianU32::from(region.x as u32),
            y: BigEndianU32::from(region.y as u32),
            w: BigEndianU32::from(region.width as u32),
            h: BigEndianU32::from(region.height as u32),
            wait_ready: BigEndianU32::from(if wait_ready { 1 } else { 0 }),
        };

        self.device.io_write(&cmd, &args)?;
        Ok(())
    }
}
