//! The implementation which uses the C library to perform operations

use std::{
    ffi::CStr,
    fmt::{self, Debug},
};

use libc::{c_int, size_t, wchar_t};

use crate::{ffi, DeviceInfo, HidDeviceBackendBase, HidError, HidResult, WcharString};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

const STRING_BUF_LEN: usize = 128;

pub struct HidApiBackend;

impl HidApiBackend {
    pub fn get_hid_device_info_vector() -> HidResult<Vec<DeviceInfo>> {
        let mut device_vector = Vec::with_capacity(8);

        let enumeration = unsafe { ffi::hid_enumerate(0, 0) };
        {
            let mut current_device = enumeration;

            while !current_device.is_null() {
                device_vector.push(unsafe { conv_hid_device_info(current_device)? });
                current_device = unsafe { (*current_device).next };
            }
        }

        if !enumeration.is_null() {
            unsafe { ffi::hid_free_enumeration(enumeration) };
        }

        Ok(device_vector)
    }

    pub fn open(vid: u16, pid: u16) -> HidResult<HidDevice> {
        let device = unsafe { ffi::hid_open(vid, pid, std::ptr::null()) };

        if device.is_null() {
            match Self::check_error() {
                Ok(err) => Err(err),
                Err(e) => Err(e),
            }
        } else {
            Ok(HidDevice::from_raw(device))
        }
    }

    pub fn open_serial(vid: u16, pid: u16, sn: &str) -> HidResult<HidDevice> {
        let mut chars = sn.chars().map(|c| c as wchar_t).collect::<Vec<_>>();
        chars.push(0 as wchar_t);
        let device = unsafe { ffi::hid_open(vid, pid, chars.as_ptr()) };
        if device.is_null() {
            match Self::check_error() {
                Ok(err) => Err(err),
                Err(e) => Err(e),
            }
        } else {
            Ok(HidDevice::from_raw(device))
        }
    }

    pub fn open_path(device_path: &CStr) -> HidResult<HidDevice> {
        let device = unsafe { ffi::hid_open_path(device_path.as_ptr()) };

        if device.is_null() {
            match Self::check_error() {
                Ok(err) => Err(err),
                Err(e) => Err(e),
            }
        } else {
            Ok(HidDevice::from_raw(device))
        }
    }

    pub fn check_error() -> HidResult<HidError> {
        Ok(HidError::HidApiError {
            message: unsafe {
                match wchar_to_string(ffi::hid_error(std::ptr::null_mut())) {
                    WcharString::String(s) => s,
                    _ => return Err(HidError::HidApiErrorEmpty),
                }
            },
        })
    }
}

/// Converts a pointer to a `*const wchar_t` to a WcharString.
unsafe fn wchar_to_string(wstr: *const wchar_t) -> WcharString {
    if wstr.is_null() {
        return WcharString::None;
    }

    let mut char_vector: Vec<char> = Vec::with_capacity(8);
    let mut raw_vector: Vec<wchar_t> = Vec::with_capacity(8);
    let mut index: isize = 0;
    let mut invalid_char = false;

    let o = |i| *wstr.offset(i);

    while o(index) != 0 {
        use std::char;

        raw_vector.push(*wstr.offset(index));

        if !invalid_char {
            if let Some(c) = char::from_u32(o(index) as u32) {
                char_vector.push(c);
            } else {
                invalid_char = true;
            }
        }

        index += 1;
    }

    if !invalid_char {
        WcharString::String(char_vector.into_iter().collect())
    } else {
        WcharString::Raw(raw_vector)
    }
}

/// Convert the CFFI `HidDeviceInfo` struct to a native `HidDeviceInfo` struct
pub unsafe fn conv_hid_device_info(src: *mut ffi::HidDeviceInfo) -> HidResult<DeviceInfo> {
    Ok(DeviceInfo {
        path: CStr::from_ptr((*src).path).to_owned(),
        vendor_id: (*src).vendor_id,
        product_id: (*src).product_id,
        serial_number: wchar_to_string((*src).serial_number),
        release_number: (*src).release_number,
        manufacturer_string: wchar_to_string((*src).manufacturer_string),
        product_string: wchar_to_string((*src).product_string),
        usage_page: (*src).usage_page,
        usage: (*src).usage,
        interface_number: (*src).interface_number,
        bus_type: (*src).bus_type,
    })
}

/// Object for accessing HID device
pub struct HidDevice {
    _hid_device: *mut ffi::HidDevice,
}

impl HidDevice {
    pub fn from_raw(device: *mut ffi::HidDevice) -> Self {
        Self {
            _hid_device: device,
        }
    }
}

unsafe impl Send for HidDevice {}

impl Debug for HidDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HidDevice").finish()
    }
}

impl Drop for HidDevice {
    fn drop(&mut self) {
        unsafe { ffi::hid_close(self._hid_device) }
    }
}

impl HidDevice {
    /// Check size returned by other methods, if it's equal to -1 check for
    /// error and return Error, otherwise return size as unsigned number
    fn check_size(&self, res: i32) -> HidResult<usize> {
        if res == -1 {
            match self.check_error() {
                Ok(err) => Err(err),
                Err(e) => Err(e),
            }
        } else {
            Ok(res as usize)
        }
    }
}

impl HidDeviceBackendBase for HidDevice {
    fn check_error(&self) -> HidResult<HidError> {
        Ok(HidError::HidApiError {
            message: unsafe {
                match wchar_to_string(ffi::hid_error(self._hid_device)) {
                    WcharString::String(s) => s,
                    _ => return Err(HidError::HidApiErrorEmpty),
                }
            },
        })
    }

    fn write(&self, data: &[u8]) -> HidResult<usize> {
        if data.is_empty() {
            return Err(HidError::InvalidZeroSizeData);
        }
        let res = unsafe { ffi::hid_write(self._hid_device, data.as_ptr(), data.len() as size_t) };
        self.check_size(res)
    }

    fn read(&self, buf: &mut [u8]) -> HidResult<usize> {
        let res = unsafe { ffi::hid_read(self._hid_device, buf.as_mut_ptr(), buf.len() as size_t) };
        self.check_size(res)
    }

    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> HidResult<usize> {
        let res = unsafe {
            ffi::hid_read_timeout(
                self._hid_device,
                buf.as_mut_ptr(),
                buf.len() as size_t,
                timeout,
            )
        };
        self.check_size(res)
    }

    fn send_feature_report(&self, data: &[u8]) -> HidResult<()> {
        if data.is_empty() {
            return Err(HidError::InvalidZeroSizeData);
        }
        let res = unsafe {
            ffi::hid_send_feature_report(self._hid_device, data.as_ptr(), data.len() as size_t)
        };
        let res = self.check_size(res)?;
        if res != data.len() {
            Err(HidError::IncompleteSendError {
                sent: res,
                all: data.len(),
            })
        } else {
            Ok(())
        }
    }

    /// Set the first byte of `buf` to the 'Report ID' of the report to be read.
    /// Upon return, the first byte will still contain the Report ID, and the
    /// report data will start in `buf[1]`.
    fn get_feature_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        let res = unsafe {
            ffi::hid_get_feature_report(self._hid_device, buf.as_mut_ptr(), buf.len() as size_t)
        };
        self.check_size(res)
    }

    fn set_blocking_mode(&self, blocking: bool) -> HidResult<()> {
        let res = unsafe {
            ffi::hid_set_nonblocking(self._hid_device, if blocking { 0i32 } else { 1i32 })
        };
        if res == -1 {
            Err(HidError::SetBlockingModeError {
                mode: match blocking {
                    true => "blocking",
                    false => "not blocking",
                },
            })
        } else {
            Ok(())
        }
    }

    fn get_manufacturer_string(&self) -> HidResult<Option<String>> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_manufacturer_string(
                self._hid_device,
                buf.as_mut_ptr(),
                STRING_BUF_LEN as size_t,
            )
        };
        let res = self.check_size(res)?;
        unsafe { Ok(wchar_to_string(buf[..res].as_ptr()).into()) }
    }

    fn get_product_string(&self) -> HidResult<Option<String>> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_product_string(
                self._hid_device,
                buf.as_mut_ptr(),
                STRING_BUF_LEN as size_t,
            )
        };
        let res = self.check_size(res)?;
        unsafe { Ok(wchar_to_string(buf[..res].as_ptr()).into()) }
    }

    fn get_serial_number_string(&self) -> HidResult<Option<String>> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_serial_number_string(
                self._hid_device,
                buf.as_mut_ptr(),
                STRING_BUF_LEN as size_t,
            )
        };
        let res = self.check_size(res)?;
        unsafe { Ok(wchar_to_string(buf[..res].as_ptr()).into()) }
    }

    fn get_indexed_string(&self, index: i32) -> HidResult<Option<String>> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_indexed_string(
                self._hid_device,
                index as c_int,
                buf.as_mut_ptr(),
                STRING_BUF_LEN,
            )
        };
        let res = self.check_size(res)?;
        unsafe { Ok(wchar_to_string(buf[..res].as_ptr()).into()) }
    }

    fn get_device_info(&self) -> HidResult<DeviceInfo> {
        let raw_device = unsafe { ffi::hid_get_device_info(self._hid_device) };
        if raw_device.is_null() {
            match self.check_error() {
                Ok(err) | Err(err) => return Err(err),
            }
        }

        unsafe { conv_hid_device_info(raw_device) }
    }

    fn get_report_descriptor(&self, buf: &mut [u8]) -> HidResult<usize> {
        let res = unsafe {
            ffi::hid_get_report_descriptor(self._hid_device, buf.as_mut_ptr(), buf.len())
        };
        self.check_size(res)
    }
}
