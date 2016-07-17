/****************************************************************************
    Copyright (c) 2015 Osspial All Rights Reserved.

    This file is part of hidapi-rs, based on hidapi_rust by Roland Ruckerbauer.

    hidapi-rs is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    hidapi-rs is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with hidapi-rs.  If not, see <http://www.gnu.org/licenses/>.
****************************************************************************/

extern crate libc;

mod ffi;

use std::ffi::{CStr};
use std::marker::PhantomData;
use libc::{wchar_t, size_t};
pub use libc::{c_ushort, c_int};

pub struct HidApi {
    devices: Vec<HidDeviceInfo>,
}

static mut hid_api_lock: bool = false;

impl HidApi {

    pub fn new() -> Result<Self, &'static str> {
        if unsafe {!hid_api_lock} {

            //Initialize the HID and prevent other HIDs from being created
            unsafe {
                ffi::hid_init();
                hid_api_lock = true;
            }


            Ok(HidApi{devices: unsafe {HidApi::get_hid_device_info_vector()}})

        } else {
            Err("HidApi already in use")
        }
    }

    ///Refresh devices list
    pub fn refresh_devices(&mut self) {
        self.devices = unsafe {HidApi::get_hid_device_info_vector()};
    }

    unsafe fn get_hid_device_info_vector() -> Vec<HidDeviceInfo> {
        let mut device_vector = Vec::with_capacity(8);

        let enumeration = ffi::hid_enumerate(0, 0);
        {
            let mut current_device = enumeration;

            'do_while: loop {

                device_vector.push(conv_hid_device_info(current_device));

                if (*current_device).next.is_null() {
                    break 'do_while;
                } else {
                    current_device = (*current_device).next;
                }
            }
        }

        ffi::hid_free_enumeration(enumeration);

        device_vector
    }

    pub fn devices(&self) -> Vec<HidDeviceInfo> {
        self.devices.clone()
    }

    pub fn open(&self, vendor_id: u16, product_id: u16) -> Result<HidDevice, &'static str> {
        let device = unsafe {ffi::hid_open(vendor_id, product_id, std::ptr::null())};

        if device.is_null() {
            Err("Cannot open hid device")
        } else {
            Ok(HidDevice {_hid_device: device, _read_buffer: [0; 512], phantom: PhantomData})
        }
    }

    pub fn open_path(&self, device_path: &str) -> Result<HidDevice, &'static str> {
        let device = unsafe {ffi::hid_open_path(std::mem::transmute(device_path.as_ptr()))};

        if device.is_null() {
            Err("Cannot open hid device")
        } else {
            Ok(HidDevice {_hid_device: device, _read_buffer: [0; 512], phantom: PhantomData})
        }
    }
}

impl Drop for HidApi {
    fn drop(&mut self) {
        unsafe {
            ffi::hid_exit();
            hid_api_lock = false;
        }
    }
}

///Converts a pointer to a wchar_t to a string
unsafe fn wchar_to_string(wstr: *mut wchar_t) -> Result<String, &'static str> {

    if wstr.is_null() {
        return Err("Null pointer!");
    }

    let mut char_vector: Vec<char> = Vec::with_capacity(8);
    let mut index: isize = 0;

    while *wstr.offset(index) != 0 {
        use std::char;
        char_vector.push(char::from_u32(*wstr.offset(index) as u32).unwrap());

        index += 1;
    }

    Ok(char_vector.into_iter().collect())
}

///Convert the C hidapi HidDeviceInfo struct to a native HidDeviceInfo struct
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
pub struct HidDeviceInfo {
    path: String,
    vendor_id: u16,
    product_id: u16,
    serial_number: Option<String>,
    release_number: u16,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    usage_page: u16,
    usage: u16,
    interface_number: i32,

}

impl HidDeviceInfo {
    /// Platform-specific device path
    pub fn get_path(&self) -> String {
        self.path.clone()
    }

    pub fn get_vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub fn get_product_id(&self) -> u16 {
        self.product_id
    }

    pub fn get_serial_number(&self) -> Option<String> {
        self.serial_number.clone()
    }

    pub fn get_release_number(&self) -> u16 {
        self.release_number
    }

    pub fn get_manufacturer_string(&self) -> Option<String> {
        self.manufacturer_string.clone()
    }

    pub fn get_product_string(&self) -> Option<String> {
        self.product_string.clone()
    }

    pub fn get_usage_page(&self) -> u16 {
        self.usage_page
    }

    pub fn get_usage(&self) -> u16 {
        self.usage
    }

    pub fn get_interface_number(&self) -> i32 {
        self.interface_number
    }
}

pub struct HidDevice<'a> {
    _hid_device: *mut ffi::HidDevice,
    _read_buffer: [u8; 512],
    /// Prevents this from outliving the api instance that created it
    phantom: PhantomData<&'a ()>
}

impl<'a> Drop for HidDevice<'a> {
    fn drop(&mut self) {
        unsafe {ffi::hid_close(self._hid_device)};
    }
}

impl <'a> HidDevice<'a> {
    pub fn write(&self, data: &[u8]) -> i32 {
        unsafe {ffi::hid_write(self._hid_device, data.as_ptr(), data.len() as size_t)}
    }

    pub fn read (&mut self) -> Option<&[u8]> {
        let actual_size = unsafe {ffi::hid_read(self._hid_device, self._read_buffer.as_mut_ptr(), 256)};

        if actual_size == 0 {
            None
        } else {
            let actual_size = actual_size as usize;

            Some(&self._read_buffer[0..actual_size])
        }
    }

    pub fn send_feature_report(&self, data: &[u8]) -> usize {
        unsafe {
            ffi::hid_send_feature_report(self._hid_device, data.as_ptr(), data.len() as size_t) as usize
        }
    }

    pub fn get_feature_report(&self, report: u8, data: &mut [u8]) -> usize {
        data[0] = report;
        unsafe {
            ffi::hid_get_feature_report(self._hid_device, data.as_mut_ptr(), data.len() as size_t) as usize
        }
    }
}
