// **************************************************************************
// Copyright (c) 2015 Osspial All Rights Reserved.
//
// This file is part of hidapi-rs, based on hidapi_rust by Roland Ruckerbauer.
// *************************************************************************

//! This crate provides a rust abstraction over the features of the C library
//! hidapi by [signal11](https://github.com/signal11/hidapi).
//!
//! # Usage
//!
//! This crate is [on crates.io](https://crates.io/crates/hidapi) and can be
//! used by adding `hidapi` to the dependencies in your project's `Cargo.toml`.
//!
//! ```toml
//! [dependencies]
//! hidapi = "0.4"
//! ```
//! # Example
//!
//! ```rust,no_run
//! extern crate hidapi;
//!
//! let api = hidapi::HidApi::new().unwrap();
//! // Print out information about all connected devices
//! for device in &api.devices() {
//!     println!("{:#?}", device);
//! }
//!
//! // Connect to device using its VID and PID
//! let (VID, PID) = (0x0123, 0x3456);
//! let device = api.open(VID, PID).unwrap();
//!
//! // Read data from device
//! let mut buf = [0u8; 8];
//! let res = device.read(&mut buf[..]).unwrap();
//! println!("Read: {:?}", &buf[..res]);
//!
//! // Write data to device
//! let buf = [0u8, 1, 2, 3, 4];
//! let res = device.write(&buf).unwrap();
//! println!("Wrote: {:?} byte(s)", res);
//! ```
extern crate libc;

mod ffi;

use std::ffi::CStr;
use std::marker::PhantomData;
use libc::{wchar_t, size_t, c_int};

pub type HidError = &'static str;
pub type HidResult<T> = Result<T, HidError>;
const STRING_BUF_LEN: usize = 128;

/// Object for handling hidapi context and implementing RAII for it.
/// Only one instance can exist at a time.
pub struct HidApi {
    devices: Vec<HidDeviceInfo>,
}

static mut HID_API_LOCK: bool = false;

impl HidApi {
    /// Initializes the HID
    pub fn new() -> HidResult<Self> {
        if unsafe { !HID_API_LOCK } {

            // Initialize the HID and prevent other HIDs from being created
            unsafe {
                if ffi::hid_init() == -1 {
                    return Err("Failed to init hid");
                }
                HID_API_LOCK = true;
            }


            Ok(HidApi { devices: unsafe { HidApi::get_hid_device_info_vector() } })

        } else {
            Err("HidApi already in use")
        }
    }

    /// Refresh devices list and information about them (to access them use
    /// `devices()` method)
    pub fn refresh_devices(&mut self) {
        self.devices = unsafe { HidApi::get_hid_device_info_vector() };
    }

    unsafe fn get_hid_device_info_vector() -> Vec<HidDeviceInfo> {
        let mut device_vector = Vec::with_capacity(8);

        let enumeration = ffi::hid_enumerate(0, 0);
        {
            let mut current_device = enumeration;

            while !current_device.is_null() {
                device_vector.push(conv_hid_device_info(current_device));
                current_device = (*current_device).next;
            }

        }

        if !enumeration.is_null() {
            ffi::hid_free_enumeration(enumeration);
        }

        device_vector
    }

    /// Returns list of objects containing information about connected devices
    pub fn devices(&self) -> Vec<HidDeviceInfo> {
        self.devices.clone()
    }

    /// Open a HID device using a Vendor ID (VID) and Product ID (PID)
    pub fn open(&self, vid: u16, pid: u16) -> HidResult<HidDevice> {
        let device = unsafe { ffi::hid_open(vid, pid, std::ptr::null()) };

        if device.is_null() {
            Err("Unable to open hid device")
        } else {
            Ok(HidDevice {
                _hid_device: device,
                phantom: PhantomData,
            })
        }
    }

    /// Open a HID device using a Vendor ID (VID), Product ID (PID) and
    /// a serial number.
    pub fn open_serial(&self, vid: u16, pid: u16, sn: &str) -> HidResult<HidDevice> {
        let device = unsafe { ffi::hid_open(vid, pid, std::mem::transmute(sn.as_ptr())) };
        if device.is_null() {
            Err("Unable to open hid device")
        } else {
            Ok(HidDevice {
                _hid_device: device,
                phantom: PhantomData,
            })
        }
    }

    /// The path name be determined by calling hid_enumerate(), or a
    /// platform-specific path name can be used (eg: /dev/hidraw0 on Linux).
    pub fn open_path(&self, device_path: &str) -> HidResult<HidDevice> {
        let device = unsafe { ffi::hid_open_path(std::mem::transmute(device_path.as_ptr())) };

        if device.is_null() {
            Err("Unable to open hid device")
        } else {
            Ok(HidDevice {
                _hid_device: device,
                phantom: PhantomData,
            })
        }
    }
}

impl Drop for HidApi {
    fn drop(&mut self) {
        unsafe {
            ffi::hid_exit();
            HID_API_LOCK = false;
        }
    }
}

/// Converts a pointer to a `wchar_t` to a string
unsafe fn wchar_to_string(wstr: *const wchar_t) -> HidResult<String> {
    if wstr.is_null() {
        return Err("Null pointer!");
    }

    let mut char_vector: Vec<char> = Vec::with_capacity(8);
    let mut index: isize = 0;

    while *wstr.offset(index) != 0 {
        use std::char;
        char_vector.push(match char::from_u32(*wstr.offset(index) as u32) {
            Some(ch) => ch,
            None => return Err("Unable to add next char"),
        });

        index += 1;
    }

    Ok(char_vector.into_iter().collect())
}

/// Convert the CFFI `HidDeviceInfo` struct to a native `HidDeviceInfo` struct
unsafe fn conv_hid_device_info(src: *mut ffi::HidDeviceInfo) -> HidDeviceInfo {
    HidDeviceInfo {
        path: std::str::from_utf8(CStr::from_ptr((*src).path).to_bytes()).unwrap().to_owned(),
        vendor_id: (*src).vendor_id,
        product_id: (*src).product_id,
        serial_number: wchar_to_string((*src).serial_number).ok(),
        release_number: (*src).release_number,
        manufacturer_string: wchar_to_string((*src).manufacturer_string).ok(),
        product_string: wchar_to_string((*src).product_string).ok(),
        usage_page: (*src).usage_page,
        usage: (*src).usage,
        interface_number: (*src).interface_number,
    }
}

#[derive(Debug, Clone)]
/// Storage for device related information
pub struct HidDeviceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub release_number: u16,
    pub manufacturer_string: Option<String>,
    pub product_string: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
}

/// Object for accessing HID device
pub struct HidDevice<'a> {
    _hid_device: *mut ffi::HidDevice,
    /// Prevents this from outliving the api instance that created it
    phantom: PhantomData<&'a ()>,
}

impl<'a> Drop for HidDevice<'a> {
    fn drop(&mut self) {
        unsafe { ffi::hid_close(self._hid_device) };
    }
}

impl<'a> HidDevice<'a> {
    /// Check size returned by other methods, if it's equal to -1 check for
    /// error and return Error, otherwise return size as unsigned number
    fn check_size(&self, res: i32) -> HidResult<usize> {
        if res == -1 {
            match self.check_error() {
                Ok(err) => {
                    if err.is_empty() {
                        Err("Undetected error")
                    } else {
                        println!("{:?}", err);
                        Err("Detected error")
                    }
                }
                Err(_) => {
                    // Err(err)
                    Err("Failed to retrieve error message")
                }
            }
        } else {
            Ok(res as usize)
        }
    }

    /// Get a string describing the last error which occurred.
    pub fn check_error(&self) -> HidResult<String> {
        unsafe { wchar_to_string(ffi::hid_error(self._hid_device)) }
    }


    /// The first byte of `data` must contain the Report ID. For
    /// devices which only support a single report, this must be set
    /// to 0x0. The remaining bytes contain the report data. Since
    /// the Report ID is mandatory, calls to `write()` will always
    /// contain one more byte than the report contains. For example,
    /// if a hid report is 16 bytes long, 17 bytes must be passed to
    /// `write()`, the Report ID (or 0x0, for devices with a
    /// single report), followed by the report data (16 bytes). In
    /// this example, the length passed in would be 17.
    /// `write()` will send the data on the first OUT endpoint, if
    /// one exists. If it does not, it will send the data through
    /// the Control Endpoint (Endpoint 0).
    pub fn write(&self, data: &[u8]) -> HidResult<usize> {
        if data.len() == 0 {
            return Err("Data must contain at least one byte");
        }
        let res = unsafe { ffi::hid_write(self._hid_device, data.as_ptr(), data.len() as size_t) };
        self.check_size(res)
    }

    /// Input reports are returned to the host through the 'INTERRUPT IN'
    /// endpoint. The first byte will contain the Report number if the device
    /// uses numbered reports.
    pub fn read(&self, buf: &mut [u8]) -> HidResult<usize> {
        let res = unsafe { ffi::hid_read(self._hid_device, buf.as_mut_ptr(), buf.len() as size_t) };
        self.check_size(res)
    }

    /// Input reports are returned to the host through the 'INTERRUPT IN'
    /// endpoint. The first byte will contain the Report number if the device
    /// uses numbered reports. Timeout measured in milliseconds, set -1 for
    /// blocking wait.
    pub fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> HidResult<usize> {
        let res = unsafe {
            ffi::hid_read_timeout(self._hid_device,
                                  buf.as_mut_ptr(),
                                  buf.len() as size_t,
                                  timeout)
        };
        self.check_size(res)
    }

    /// Send a Feature report to the device.
    /// Feature reports are sent over the Control endpoint as a
    /// Set_Report transfer.  The first byte of `data` must contain the
    /// 'Report ID'. For devices which only support a single report, this must
    /// be set to 0x0. The remaining bytes contain the report data. Since the
    /// 'Report ID' is mandatory, calls to `send_feature_report()` will always
    /// contain one more byte than the report contains. For example, if a hid
    /// report is 16 bytes long, 17 bytes must be passed to
    /// `send_feature_report()`: 'the Report ID' (or 0x0, for devices which
    /// do not use numbered reports), followed by the report data (16 bytes).
    /// In this example, the length passed in would be 17.
    pub fn send_feature_report(&self, data: &[u8]) -> HidResult<()> {
        if data.len() == 0 {
            return Err("Data must contain at least one byte");
        }
        let res = unsafe {
            ffi::hid_send_feature_report(self._hid_device, data.as_ptr(), data.len() as size_t)
        };
        let res = try!(self.check_size(res));
        if res != data.len() {
            Err("Failed to send feature report completely")
        } else {
            Ok(())
        }
    }

    /// Set the first byte of `buf` to the 'Report ID' of the report to be read.
    /// Upon return, the first byte will still contain the Report ID, and the
    /// report data will start in buf[1].
    pub fn get_feature_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        let res = unsafe {
            ffi::hid_get_feature_report(self._hid_device, buf.as_mut_ptr(), buf.len() as size_t)
        };
        self.check_size(res)
    }

    /// Set the device handle to be in blocking or in non-blocking mode. In
    /// non-blocking mode calls to `read()` will return immediately with an empty
    /// slice if there is no data to be read. In blocking mode, `read()` will
    /// wait (block) until there is data to read before returning.
    /// Modes can be changed at any time.
    pub fn set_blocking_mode(&self, blocking: bool) -> HidResult<()> {
        let res = unsafe {
            ffi::hid_set_nonblocking(self._hid_device, if blocking { 0i32 } else { 1i32 })
        };
        if res == -1 {
            Err("Failed to set blocking mode")
        } else {
            Ok(())
        }
    }

    /// Get The Manufacturer String from a HID device.
    pub fn get_manufacturer_string(&self) -> HidResult<String> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_manufacturer_string(self._hid_device,
                                             buf.as_mut_ptr(),
                                             STRING_BUF_LEN as size_t)
        };
        let res = try!(self.check_size(res));
        unsafe { wchar_to_string(buf[..res].as_ptr()) }
    }

    /// Get The Manufacturer String from a HID device.
    pub fn get_product_string(&self) -> HidResult<String> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_product_string(self._hid_device,
                                        buf.as_mut_ptr(),
                                        STRING_BUF_LEN as size_t)
        };
        let res = try!(self.check_size(res));
        unsafe { wchar_to_string(buf[..res].as_ptr()) }
    }

    /// Get The Serial Number String from a HID device.
    pub fn get_serial_number_string(&self) -> HidResult<String> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_serial_number_string(self._hid_device,
                                              buf.as_mut_ptr(),
                                              STRING_BUF_LEN as size_t)
        };
        let res = try!(self.check_size(res));
        unsafe { wchar_to_string(buf[..res].as_ptr()) }
    }

    /// Get a string from a HID device, based on its string index.
    pub fn get_indexed_string(&self, index: i32) -> HidResult<String> {
        let mut buf = [0 as wchar_t; STRING_BUF_LEN];
        let res = unsafe {
            ffi::hid_get_indexed_string(self._hid_device,
                                        index as c_int,
                                        buf.as_mut_ptr(),
                                        STRING_BUF_LEN)
        };
        let res = try!(self.check_size(res));
        unsafe { wchar_to_string(buf[..res].as_ptr()) }
    }
}
