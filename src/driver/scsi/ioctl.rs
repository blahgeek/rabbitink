use nix::{ioctl_read_bad, ioctl_readwrite_bad};

use super::bindings;

#[cfg(target_os = "linux")]
ioctl_read_bad!(scsi_get_bus_number, bindings::SCSI_IOCTL_GET_BUS_NUMBER, std::os::raw::c_int);

#[cfg(target_os = "linux")]
ioctl_readwrite_bad!(scsi_sg_io, bindings::SG_IO, bindings::sg_io_hdr_t);
