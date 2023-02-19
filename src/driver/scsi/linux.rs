use super::DeviceIO;
use super::bindings;
use super::ioctl;

use std::{os::fd::RawFd, path::Path};
use log::{trace, info};
use nix::{unistd, fcntl, sys::stat::Mode};

pub struct LinuxDeviceIO {
    fd: RawFd,
}

impl Drop for LinuxDeviceIO {
    fn drop(&mut self) {
        trace!("Closing scsi device {}", self.fd);
        unistd::close(self.fd).expect("cannot close scsi device");
    }
}

impl DeviceIO for LinuxDeviceIO {
    fn io_read(&mut self, cmd: &[u8], data: &mut [u8]) -> anyhow::Result<()> {
        let mut iohdr = bindings::sg_io_hdr::default();
        iohdr.interface_id = 'S' as i32;
        iohdr.dxfer_direction = bindings::SG_DXFER_FROM_DEV;
        iohdr.timeout = u32::MAX;

        iohdr.cmd_len = u8::try_from(cmd.len()).expect("cmd too large");
        iohdr.dxfer_len = u32::try_from(data.len()).expect("data too large");
        iohdr.cmdp = (cmd.as_ptr()) as *mut u8;
        if iohdr.dxfer_len > 0 {
            iohdr.dxferp = (data.as_mut_ptr()) as *mut std::os::raw::c_void;
        }
        unsafe {
            ioctl::scsi_sg_io(self.fd, &mut iohdr)?;
        }

        Ok(())
    }

    fn io_write(&mut self, cmd: &[u8], data: &[u8]) -> anyhow::Result<()> {
        let mut iohdr = bindings::sg_io_hdr::default();
        iohdr.interface_id = 'S' as i32;
        iohdr.dxfer_direction = bindings::SG_DXFER_TO_DEV;
        iohdr.timeout = u32::MAX;

        iohdr.cmd_len = u8::try_from(cmd.len()).expect("cmd too large");
        iohdr.dxfer_len = u32::try_from(data.len()).expect("data too large");
        iohdr.cmdp = (cmd.as_ptr()) as *mut u8;
        if iohdr.dxfer_len > 0 {
            iohdr.dxferp = (data.as_ptr()) as *mut std::os::raw::c_void;
        }
        unsafe {
            ioctl::scsi_sg_io(self.fd, &mut iohdr)?;
        }

        Ok(())
    }
}

impl LinuxDeviceIO {
    pub fn open(path: &Path) -> nix::Result<LinuxDeviceIO> {
        let fd = fcntl::open(path, fcntl::OFlag::O_RDWR | fcntl::OFlag::O_NONBLOCK, Mode::empty())?;
        info!("Opened scsi device {:?}: {}", path, fd);
        // construct it now, so that it would drop after following error
        let device = LinuxDeviceIO{fd};

        let mut bus_number : std::os::raw::c_int = 0;
        unsafe {
            ioctl::scsi_get_bus_number(device.fd, &mut bus_number)?;
        }
        trace!("LinuxDeviceIO bus number: {}", bus_number);

        Ok(device)
    }
}
