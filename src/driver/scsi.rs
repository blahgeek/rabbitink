use std::{os::fd::RawFd, path::Path};

use log::{trace, info};
use nix::{unistd, fcntl, sys::stat::Mode};

mod bindings;
mod ioctl;

pub struct Device {
    fd: RawFd,
}

impl Drop for Device {
    fn drop(&mut self) {
        trace!("Closing scsi device {}", self.fd);
        unistd::close(self.fd).expect("cannot close scsi device");
    }
}

#[cfg(target_os = "linux")]
impl Device {
    pub fn open(path: &Path) -> nix::Result<Device> {
        let fd = fcntl::open(path, fcntl::OFlag::O_RDWR | fcntl::OFlag::O_NONBLOCK, Mode::empty())?;
        info!("Opened scsi device {:?}: {}", path, fd);
        // construct it now, so that it would drop after following error
        let device = Device{fd};

        let mut bus_number : std::os::raw::c_int = 0;
        unsafe {
            ioctl::scsi_get_bus_number(device.fd, &mut bus_number)?;
        }
        trace!("Device bus number: {}", bus_number);

        Ok(device)
    }

    pub fn io_write<CMD, DATA>(&mut self, cmd: &CMD, data: &DATA) -> nix::Result<()> {
        let mut iohdr = bindings::sg_io_hdr::default();
        iohdr.interface_id = 'S' as i32;
        iohdr.dxfer_direction = bindings::SG_DXFER_TO_DEV;
        iohdr.timeout = u32::MAX;

        iohdr.cmd_len = u8::try_from(std::mem::size_of::<CMD>()).expect("cmd too large");
        iohdr.dxfer_len = u32::try_from(std::mem::size_of::<DATA>()).expect("data too large");
        iohdr.cmdp = (cmd as *const CMD) as *mut u8;
        if iohdr.dxfer_len > 0 {
            iohdr.dxferp = (data as *const DATA) as *mut std::os::raw::c_void;
        }
        unsafe {
            ioctl::scsi_sg_io(self.fd, &mut iohdr)?;
        }

        Ok(())
    }

    pub fn io_write_gather<CMD>(&mut self, cmd: &CMD, data_list: &[(*const u8, usize)]) -> nix::Result<()> {
        let mut iohdr = bindings::sg_io_hdr::default();
        iohdr.interface_id = 'S' as i32;
        iohdr.dxfer_direction = bindings::SG_DXFER_TO_DEV;
        iohdr.timeout = u32::MAX;
        iohdr.cmd_len = u8::try_from(std::mem::size_of::<CMD>()).expect("cmd too large");
        iohdr.cmdp = (cmd as *const CMD) as *mut u8;

        let mut iovecs: Vec<bindings::sg_iovec> = Vec::new();
        for (data_p, data_len) in data_list {
            iovecs.push(bindings::sg_iovec {
                iov_base: *data_p as *mut std::os::raw::c_void,
                iov_len: *data_len,
            });
            iohdr.dxfer_len += *data_len as u32;
        }

        iohdr.iovec_count = data_list.len() as u16;
        iohdr.dxferp = iovecs.as_ptr() as *mut std::os::raw::c_void;

        unsafe {
            ioctl::scsi_sg_io(self.fd, &mut iohdr)?;
        }
        Ok(())
    }

    pub fn io_read<CMD, DATA>(&mut self, cmd: &CMD, data: &mut DATA) -> nix::Result<()> {
        let mut iohdr = bindings::sg_io_hdr::default();
        iohdr.interface_id = 'S' as i32;
        iohdr.dxfer_direction = bindings::SG_DXFER_FROM_DEV;
        iohdr.timeout = u32::MAX;

        iohdr.cmd_len = u8::try_from(std::mem::size_of::<CMD>()).expect("cmd too large");
        iohdr.dxfer_len = u32::try_from(std::mem::size_of::<DATA>()).expect("data too large");
        iohdr.cmdp = (cmd as *const CMD) as *mut u8;
        if iohdr.dxfer_len > 0 {
            iohdr.dxferp = (data as *mut DATA) as *mut std::os::raw::c_void;
        }
        unsafe {
            ioctl::scsi_sg_io(self.fd, &mut iohdr)?;
        }

        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
impl Device {
    pub fn open(path: &Path) -> nix::Result<Device> {
        unimplemented!();
    }

    pub fn io_write<CMD, DATA>(&mut self, cmd: &CMD, data: &DATA) -> nix::Result<()> {
        unimplemented!();
    }

    pub fn io_write_gather<CMD>(&mut self, cmd: &CMD, data_list: &[(*const u8, usize)]) -> nix::Result<()> {
        unimplemented!();
    }

    pub fn io_read<CMD, DATA>(&mut self, cmd: &CMD, data: &mut DATA) -> nix::Result<()> {
        unimplemented!();
    }
}
