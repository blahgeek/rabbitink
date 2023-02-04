mod bindings;

use std::fmt;
use log::info;
use opencv as cv;
use opencv::prelude::*;

#[derive(Debug, Clone)]
pub enum VNCError {
    APIError(&'static str),
}

impl std::error::Error for VNCError {}

impl fmt::Display for VNCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            &Self::APIError(name) => write!(f, "Error calling VNC API: {}", name)
        }
    }
}

type Result<T> = std::result::Result<T, VNCError>;


#[derive(Clone, Copy)]
pub struct PixelFormat {
    pub bits_per_sample: i32,
    pub samples_per_pixel: i32,
    pub bytes_per_pixel: i32,

    pub red_shift: u8,
    pub red_max: u16,
    pub green_shift: u8,
    pub green_max: u16,
    pub blue_shift: u8,
    pub blue_max: u16,
}

pub const PIXEL_FORMAT_BGR555: PixelFormat = PixelFormat {
    bits_per_sample: 5,
    samples_per_pixel: 3,
    bytes_per_pixel: 2,
    red_shift: 10,
    red_max: 31,
    green_shift: 5,
    green_max: 31,
    blue_shift: 0,
    blue_max: 31,
};

pub struct InitOptions {
    pub host: String,
    pub pixel_format: PixelFormat,
}

pub struct VNCClient {
    rfb_client: *mut bindings::rfbClient,

    pixel_format: PixelFormat,
    buffer: cv::core::Mat,
}

impl Drop for VNCClient {
    fn drop(&mut self) {
        unsafe { bindings::rfbClientCleanup(self.rfb_client) }
    }
}

const CLIENT_DATA_TAG: i32 = 0;

fn get_vncclient_from_client_data(client: *mut bindings::rfbClient) -> *mut VNCClient {
    let data = unsafe {
        bindings::rfbClientGetClientData(client, &CLIENT_DATA_TAG as *const i32 as *mut std::os::raw::c_void)
    };
    let res = data as *mut VNCClient;
    assert_eq!(unsafe { (*res).rfb_client }, client);
    return res;
}

extern "C" fn malloc_frame_buffer_callback(_client: *mut bindings::rfbClient) -> bindings::rfbBool {
    let client = unsafe { &mut *get_vncclient_from_client_data(_client)};
    assert!(client.buffer.empty()); // does not support resize yet

    let size = unsafe {cv::core::Size2i::new((*client.rfb_client).width, (*client.rfb_client).height)};
    info!("Malloc frame buffer callback: size={:?}", size);
    client.buffer =
        cv::core::Mat::new_size_with_default(
            size, cv::core::CV_MAKETYPE(cv::core::CV_8U, client.pixel_format.bytes_per_pixel), 0.into())
        .expect("Failed to create buffer Mat");
    assert!(client.buffer.is_continuous());
    unsafe {(*client.rfb_client).frameBuffer = client.buffer.ptr_mut(0).unwrap()};
    return -1;  // true
}

impl VNCClient {
    pub fn new(init_options: InitOptions) -> Result<Box<VNCClient>> {
        let pixel_format = init_options.pixel_format;
        let rfb_client = unsafe { bindings::rfbGetClient(
            pixel_format.bits_per_sample, pixel_format.samples_per_pixel, pixel_format.bytes_per_pixel
        ) };
        if rfb_client.is_null() {
            return Err(VNCError::APIError("rfbGetClient"));
        }

        let client = Box::new(VNCClient { rfb_client, pixel_format, buffer: cv::core::Mat::default() });
        unsafe {
            (*rfb_client).format.redShift = pixel_format.red_shift;
            (*rfb_client).format.redMax = pixel_format.red_max;
            (*rfb_client).format.greenShift = pixel_format.green_shift;
            (*rfb_client).format.greenMax = pixel_format.green_max;
            (*rfb_client).format.blueShift = pixel_format.blue_shift;
            (*rfb_client).format.blueMax = pixel_format.blue_max;
            (*rfb_client).MallocFrameBuffer = Some(malloc_frame_buffer_callback);
            bindings::rfbClientSetClientData(
                client.rfb_client,
                &CLIENT_DATA_TAG as *const i32 as *mut std::os::raw::c_void,
                client.as_ref() as *const VNCClient as *mut std::os::raw::c_void
            );
        }

        let mut argv: Vec<std::ffi::CString> = vec![];
        // TODO: add more options
        argv.push(std::ffi::CString::new(init_options.host).unwrap());
        let mut c_argv: Vec<*mut std::os::raw::c_char> =
            argv.iter().map(|s| s.as_ptr() as *mut std::os::raw::c_char).collect();
        let mut c_argc = argv.len() as std::os::raw::c_int;
        let init_res = unsafe { bindings::rfbInitClient(rfb_client, &mut c_argc as *mut i32, c_argv.as_mut_ptr()) };
        if init_res == 0 {
            return Err(VNCError::APIError("rfbInitClient"));
        }

        Ok(client)
    }

    // return >0 if message is received, 0 on timeout, error otherwise
    pub fn wait_for_message(&mut self, timeout: std::time::Duration) -> Result<i32> {
        let timeout_ms = std::os::raw::c_uint::try_from(timeout.as_micros())
            .expect("timeout too large");
        let res = unsafe { bindings::WaitForMessage(self.rfb_client, timeout_ms) };
        if res < 0 {
            return Err(VNCError::APIError("WaitForMessage"));
        }
        return Ok(res)
    }

    pub fn handle_message(&mut self) -> Result<()> {
        let res = unsafe { bindings::HandleRFBServerMessage(self.rfb_client) };
        if res == 0 {
            return Err(VNCError::APIError("HandleRFBServerMessage"));
        }
        return Ok(());
    }

    pub fn get_frame_buffer(&self) -> &cv::core::Mat {
        &self.buffer
    }
}
