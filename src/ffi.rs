#![allow(unused_imports, dead_code)]

/// **************************************************************************
/// Copyright (c) 2015 Osspial All Rights Reserved.
///
/// This file is part of hidapi-rs, based on hidapi_rust by Roland Ruckerbauer.
/// *************************************************************************
// For documentation look at the corresponding C header file hidapi.h
use libc::{c_char, c_int, c_uchar, c_ushort, c_void, intptr_t, size_t, wchar_t};
type HidBusType = crate::BusType;
pub type HidDevice = c_void;
type LibusbContext = c_void;

#[repr(C)]
pub struct HidDeviceInfo {
    pub path: *mut c_char,
    pub vendor_id: c_ushort,
    pub product_id: c_ushort,
    pub serial_number: *mut wchar_t,
    pub release_number: c_ushort,
    pub manufacturer_string: *mut wchar_t,
    pub product_string: *mut wchar_t,
    pub usage_page: c_ushort,
    pub usage: c_ushort,
    pub interface_number: c_int,
    pub next: *mut HidDeviceInfo,
    pub bus_type: HidBusType,
}

#[allow(dead_code)]
extern "C" {
    #[cfg_attr(target_os = "openbsd", link_name = "hidapi_hid_init")]
    pub fn hid_init() -> c_int;
    pub fn hid_exit() -> c_int;
    pub fn hid_enumerate(vendor_id: c_ushort, product_id: c_ushort) -> *mut HidDeviceInfo;
    pub fn hid_free_enumeration(hid_device_info: *mut HidDeviceInfo);
    pub fn hid_open(
        vendor_id: c_ushort,
        product_id: c_ushort,
        serial_number: *const wchar_t,
    ) -> *mut HidDevice;
    pub fn hid_open_path(path: *const c_char) -> *mut HidDevice;
    #[cfg(libusb)]
    pub fn hid_libusb_wrap_sys_device(sys_dev: intptr_t, interface_num: c_int) -> *mut HidDevice;
    #[cfg(all(libusb, not(target_os = "freebsd")))]
    pub fn libusb_set_option(ctx: *mut LibusbContext, option: c_int);
    pub fn hid_write(device: *mut HidDevice, data: *const c_uchar, length: size_t) -> c_int;
    pub fn hid_read_timeout(
        device: *mut HidDevice,
        data: *mut c_uchar,
        length: size_t,
        milleseconds: c_int,
    ) -> c_int;
    pub fn hid_read(device: *mut HidDevice, data: *mut c_uchar, length: size_t) -> c_int;
    pub fn hid_set_nonblocking(device: *mut HidDevice, nonblock: c_int) -> c_int;
    pub fn hid_send_feature_report(
        device: *mut HidDevice,
        data: *const c_uchar,
        length: size_t,
    ) -> c_int;
    pub fn hid_get_feature_report(
        device: *mut HidDevice,
        data: *mut c_uchar,
        length: size_t,
    ) -> c_int;
    pub fn hid_send_output_report(
        device: *mut HidDevice,
        data: *const c_uchar,
        length: size_t,
    ) -> c_int;
    pub fn hid_get_input_report(
        device: *mut HidDevice,
        data: *mut c_uchar,
        length: size_t,
    ) -> c_int;
    pub fn hid_close(device: *mut HidDevice);
    pub fn hid_get_manufacturer_string(
        device: *mut HidDevice,
        string: *mut wchar_t,
        maxlen: size_t,
    ) -> c_int;
    pub fn hid_get_product_string(
        device: *mut HidDevice,
        string: *mut wchar_t,
        maxlen: size_t,
    ) -> c_int;
    pub fn hid_get_serial_number_string(
        device: *mut HidDevice,
        string: *mut wchar_t,
        maxlen: size_t,
    ) -> c_int;
    pub fn hid_get_device_info(device: *mut HidDevice) -> *mut HidDeviceInfo;
    pub fn hid_get_indexed_string(
        device: *mut HidDevice,
        string_index: c_int,
        string: *mut wchar_t,
        maxlen: size_t,
    ) -> c_int;
    pub fn hid_get_report_descriptor(
        hid_device: *mut HidDevice,
        buf: *mut c_uchar,
        buf_size: size_t,
    ) -> c_int;
    pub fn hid_error(device: *mut HidDevice) -> *const wchar_t;
}

// For documentation look at the corresponding C header file hidapi_darwin.h
#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;

    extern "C" {
        pub fn hid_darwin_get_location_id(device: *mut HidDevice, location_id: *mut u32) -> c_int;
        pub fn hid_darwin_set_open_exclusive(open_exclusive: c_int);
        pub fn hid_darwin_get_open_exclusive() -> c_int;
        pub fn hid_darwin_is_device_open_exclusive(device: *mut HidDevice) -> c_int;
    }
}

// For documentation look at the corresponding C header file hidapi_winapi.h
#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use windows_sys::core::GUID;

    extern "C" {
        pub fn hid_winapi_get_container_id(
            device: *mut HidDevice,
            container_id: *mut GUID,
        ) -> c_int;

    }
}
