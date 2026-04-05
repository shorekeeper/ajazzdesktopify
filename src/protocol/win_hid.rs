/// Direct Windows HID API for report transfers that `hidapi` cannot do.
///
/// Interface 3 (`0xFF67`) has a 4097-byte output report (firmware update)
/// and a 65-byte feature report. `hidapi`'s `write()` hangs on this
/// interface, so we use `HidD_SetOutputReport` / `HidD_GetInputReport`
/// through the control pipe instead.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HidD_GetInputReport, HidD_SetOutputReport,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};

use crate::device::DeviceError;

unsafe extern "system" {
    fn CreateFileW(
        lpFileName: *const u16, dwDesiredAccess: u32, dwShareMode: u32,
        lpSecurityAttributes: *const std::ffi::c_void,
        dwCreationDisposition: u32, dwFlagsAndAttributes: u32,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    fn GetLastError() -> u32;
    fn HidD_GetPreparsedData(HidDeviceObject: HANDLE, PreparsedData: *mut isize) -> i32;
    fn HidD_FreePreparsedData(PreparsedData: isize) -> i32;
    fn HidP_GetCaps(PreparsedData: isize, Capabilities: *mut HidPCaps) -> i32;
}

#[repr(C)]
#[derive(Default)]
pub struct HidPCaps {
    pub usage: u16,
    pub usage_page: u16,
    pub input_report_byte_length: u16,
    pub output_report_byte_length: u16,
    pub feature_report_byte_length: u16,
    pub reserved: [u16; 17],
    pub number_link_collection_nodes: u16,
    pub number_input_button_caps: u16,
    pub number_input_value_caps: u16,
    pub number_input_data_indices: u16,
    pub number_output_button_caps: u16,
    pub number_output_value_caps: u16,
    pub number_output_data_indices: u16,
    pub number_feature_button_caps: u16,
    pub number_feature_value_caps: u16,
    pub number_feature_data_indices: u16,
}

const FILE_SHARE_RW: u32 = 0x01 | 0x02;
const OPEN_EXISTING: u32 = 3;
const REPORT_BUF_SIZE: usize = 65;

/// A raw Windows HID handle for direct OUTPUT/INPUT report transfers.
pub struct WinHidDevice {
    handle: HANDLE,
}

unsafe impl Send for WinHidDevice {}
unsafe impl Sync for WinHidDevice {}

impl WinHidDevice {
    pub fn open(path: &str) -> Result<Self, DeviceError> {
        let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(), GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_RW, ptr::null(), OPEN_EXISTING, 0, ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(DeviceError::Protocol(format!("CreateFileW failed: {path}")));
        }
        log::info!("WinHID: opened {path}");
        Ok(Self { handle })
    }

    pub fn get_caps(&self) -> Result<HidPCaps, DeviceError> {
        let mut preparsed: isize = 0;
        if unsafe { HidD_GetPreparsedData(self.handle, &mut preparsed) } == 0 {
            return Err(DeviceError::Protocol("HidD_GetPreparsedData failed".into()));
        }
        let mut caps = HidPCaps::default();
        let status = unsafe { HidP_GetCaps(preparsed, &mut caps) };
        unsafe { HidD_FreePreparsedData(preparsed) };
        if status != 0x0011_0000 { // HIDP_STATUS_SUCCESS
            return Err(DeviceError::Protocol(format!("HidP_GetCaps: {status:#010X}")));
        }
        Ok(caps)
    }

    pub fn handle_raw(&self) -> HANDLE { self.handle }

    pub fn set_output_report(&self, data: &[u8; 64]) -> Result<(), DeviceError> {
        let mut buf = [0u8; REPORT_BUF_SIZE];
        buf[0] = 0x00;
        buf[1..].copy_from_slice(data);
        let ok = unsafe {
            HidD_SetOutputReport(self.handle, buf.as_ptr() as _, REPORT_BUF_SIZE as u32)
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            Err(DeviceError::Protocol(format!("SetOutputReport failed ({err:#010X})")))
        } else {
            Ok(())
        }
    }

    pub fn get_input_report(&self) -> Result<[u8; 64], DeviceError> {
        let mut buf = [0u8; REPORT_BUF_SIZE];
        buf[0] = 0x00;
        let ok = unsafe {
            HidD_GetInputReport(self.handle, buf.as_mut_ptr() as _, REPORT_BUF_SIZE as u32)
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            Err(DeviceError::Protocol(format!("GetInputReport failed ({err:#010X})")))
        } else {
            let mut out = [0u8; 64];
            out.copy_from_slice(&buf[1..]);
            Ok(out)
        }
    }

    pub fn set_output_report_dynamic(&self, data: &[u8], report_size: usize) -> Result<(), DeviceError> {
        let mut buf = vec![0u8; report_size];
        buf[0] = 0x00;
        let n = data.len().min(report_size - 1);
        buf[1..1 + n].copy_from_slice(&data[..n]);
        let ok = unsafe {
            HidD_SetOutputReport(self.handle, buf.as_ptr() as _, report_size as u32)
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            Err(DeviceError::Protocol(format!("SetOutputReport({report_size}B) failed ({err:#010X})")))
        } else {
            Ok(())
        }
    }

    pub fn get_input_report_dynamic(&self, report_size: usize) -> Result<Vec<u8>, DeviceError> {
        let mut buf = vec![0u8; report_size];
        buf[0] = 0x00;
        let ok = unsafe {
            HidD_GetInputReport(self.handle, buf.as_mut_ptr() as _, report_size as u32)
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            Err(DeviceError::Protocol(format!("GetInputReport({report_size}B) failed ({err:#010X})")))
        } else {
            Ok(buf[1..].to_vec())
        }
    }
}

impl Drop for WinHidDevice {
    fn drop(&mut self) { unsafe { CloseHandle(self.handle); } }
}

/// Find the Windows device path for a given VID/PID/usage_page.
pub fn find_device_path(vid: u16, pid: u16, usage_page: u16) -> Result<String, DeviceError> {
    let api = hidapi::HidApi::new()?;
    let path = api.device_list()
        .find(|d| d.vendor_id() == vid && d.product_id() == pid && d.usage_page() == usage_page)
        .map(|d| d.path().to_string_lossy().into_owned());
    path.ok_or_else(|| DeviceError::NotFound(format!(
        "No HID device VID={vid:#06X} PID={pid:#06X} page={usage_page:#06X}"
    )))
}