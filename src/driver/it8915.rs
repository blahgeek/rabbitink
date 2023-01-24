use std::{path::Path, fmt::Debug};

use log::{trace, info};
use opencv::prelude::*;
use anyhow::Context;

use super::scsi;

const LOAD_IMAGE_MAX_TRANSFER_SIZE: i32 = 60800;

#[repr(packed)]
#[derive(Clone, Copy, Default)]
struct BigEndianU32 ([u8; 4]);

impl BigEndianU32 {
    fn val(&self) -> u32 {
        ((self.0[0] as u32) << 24) | ((self.0[1] as u32) << 16) | ((self.0[2] as u32) << 8) | (self.0[3] as u32)
    }
}

impl From<u32> for BigEndianU32 {
    fn from(value: u32) -> Self {
        Self([(value >> 24) as u8, ((value >> 16) & 0xff) as u8, ((value >> 8) & 0xff) as u8, (value & 0xff) as u8])
    }
}

impl Debug for BigEndianU32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [0x{:08x}, BE]", self.val(), self.val())
    }
}

#[repr(packed)]
#[derive(Clone, Copy, Default)]
struct BigEndianU16 ([u8; 2]);

impl BigEndianU16 {
    fn val(&self) -> u16 {
        ((self.0[0] as u16) << 8) | (self.0[1] as u16)
    }
}

impl From<u16> for BigEndianU16 {
    fn from(value: u16) -> Self {
        Self([((value >> 8) & 0xff) as u8, (value & 0xff) as u8])
    }
}

impl Debug for BigEndianU16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [0x{:04x}, BE]", self.val(), self.val())
    }
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
struct MemIOCmd {
    hdr: BigEndianU16,
    addr: BigEndianU32,
    cmd: u8,
    len: BigEndianU16,
    padding: [u8; 7],
}

#[repr(packed)]
#[derive(Default)]
#[allow(dead_code)]
struct PMICControlCmd {
    hdr: BigEndianU16,
    padding0: u32,
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
}

const EXPECT_INQUERY_VENDOR_PRODUCT : &'static str = "Generic Storage RamDisc 1.00";


impl IT8915 {
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

        Ok(IT8915 { device, sysinfo })
    }

    pub fn pmic_control(&mut self, vcom: Option<u16>, power: Option<bool>) -> anyhow::Result<()> {
        let mut cmd = PMICControlCmd {
            hdr: BigEndianU16::from(0x00fe),
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

    // TODO: should not be public
    pub fn read_mem<DATA>(&mut self, addr: u32, buf: &mut DATA) -> anyhow::Result<()> {
        let cmd = MemIOCmd {
            hdr: BigEndianU16::from(0x00fe),
            addr: BigEndianU32::from(addr),
            cmd: 0x81,
            len: BigEndianU16::from(u16::try_from(std::mem::size_of::<DATA>()).expect("read_mem buf too long")),
            padding: [0; 7],
        };
        self.device.io_read(&cmd, buf)?;
        Ok(())
    }

    // TODO: should not be public
    pub fn write_mem<DATA>(&mut self, addr: u32, buf: &DATA) -> anyhow::Result<()> {
        let cmd = MemIOCmd {
            hdr: BigEndianU16::from(0x00fe),
            addr: BigEndianU32::from(addr),
            cmd: 0x82,
            len: BigEndianU16::from(u16::try_from(std::mem::size_of::<DATA>()).expect("write_mem buf too long")),
            padding: [0; 7],
        };
        self.device.io_write(&cmd, buf)?;
        Ok(())
    }

    // make sure that image size is within max transfer size
    fn load_image_area_onestep(&mut self, pos: (u32, u32), image: opencv::core::Mat1b) -> anyhow::Result<()> {
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

    pub fn load_image_area(&mut self, pos: (u32, u32), image: opencv::core::Mat1b) -> anyhow::Result<()> {
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

    // TODO: mode
    pub fn display_area(&mut self, region: opencv::core::Rect2i, mode: u32) -> anyhow::Result<()> {
        trace!("Displaying region {:?}, mode = {:?}", region, mode);
        let cmd: [u8; 16] = [
            0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0x94, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let args = DisplayAreaArgs {
            addr: self.sysinfo.image_buf_base,
            mode: BigEndianU32::from(mode),
            x: BigEndianU32::from(region.x as u32),
            y: BigEndianU32::from(region.y as u32),
            w: BigEndianU32::from(region.width as u32),
            h: BigEndianU32::from(region.height as u32),
            wait_ready: BigEndianU32::from(0xffffffff),
        };

        self.device.io_write(&cmd, &args)?;
        Ok(())
    }
}
