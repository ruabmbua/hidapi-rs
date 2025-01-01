use core::cell::Ref;
use core::ffi::CStr;
use std::ffi::CString;

use wasm_bindgen_futures::{js_sys::Array, wasm_bindgen::JsCast, JsFuture};

use crate::{DeviceInfo, HidDeviceBackendBase, HidResult};

pub struct HidApiBackend;

impl HidApiBackend {
    pub async fn get_hid_device_info_vector(vid: u16, pid: u16) -> HidResult<Vec<DeviceInfo>> {
        let window = web_sys::window().unwrap();
        let navigator = window.navigator();
        let hid = navigator.hid();
        let devices = JsFuture::from(hid.get_devices()).await.unwrap();

        let devices: Array = JsCast::unchecked_from_js(devices);

        let mut result = vec![];
        for device in devices {
            let device: web_sys::HidDevice = JsCast::unchecked_from_js(device);

            // vid = 0 and pid = 0 means no filter
            if (device.vendor_id() != vid && vid != 0) || (device.product_id() != pid && pid != 0) {
                continue;
            }

            result.push(DeviceInfo {
                path: CString::new("").unwrap(),
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                serial_number: crate::WcharString::None,
                release_number: 0,
                manufacturer_string: crate::WcharString::None,
                product_string: crate::WcharString::String(device.product_name()),
                usage_page: 0,
                usage: 0,
                interface_number: 0,
                bus_type: crate::BusType::Usb,
            });
        }

        Ok(result)
    }

    pub fn open(vid: u16, pid: u16) -> HidResult<HidDevice> {
        HidDevice::open(vid, pid, None)
    }

    pub fn open_serial(vid: u16, pid: u16, sn: &str) -> HidResult<HidDevice> {
        HidDevice::open(vid, pid, Some(sn))
    }

    pub fn open_path(device_path: &CStr) -> HidResult<HidDevice> {
        HidDevice::open_path(device_path)
    }
}

pub struct HidDevice {}

unsafe impl Send for HidDevice {}

// API for the library to call us, or for internal uses
impl HidDevice {
    pub(crate) fn open(_vid: u16, _pid: u16, _sn: Option<&str>) -> HidResult<Self> {
        todo!()
    }

    pub(crate) fn open_path(_device_path: &CStr) -> HidResult<HidDevice> {
        todo!()
    }

    fn _info(&self) -> HidResult<Ref<DeviceInfo>> {
        todo!()
    }
}

impl HidDeviceBackendBase for HidDevice {
    fn write(&self, _data: &[u8]) -> HidResult<usize> {
        todo!()
    }

    fn read(&self, _buf: &mut [u8]) -> HidResult<usize> {
        todo!()
    }

    fn read_timeout(&self, _buf: &mut [u8], _timeout: i32) -> HidResult<usize> {
        todo!()
    }

    fn send_feature_report(&self, _data: &[u8]) -> HidResult<()> {
        todo!()
    }

    fn get_feature_report(&self, _buf: &mut [u8]) -> HidResult<usize> {
        todo!()
    }

    fn set_blocking_mode(&self, _blocking: bool) -> HidResult<()> {
        todo!()
    }

    fn get_manufacturer_string(&self) -> HidResult<Option<String>> {
        todo!()
    }

    fn get_product_string(&self) -> HidResult<Option<String>> {
        todo!()
    }

    fn get_serial_number_string(&self) -> HidResult<Option<String>> {
        todo!()
    }

    fn get_device_info(&self) -> HidResult<DeviceInfo> {
        todo!()
    }

    fn get_report_descriptor(&self, _buf: &mut [u8]) -> HidResult<usize> {
        todo!()
    }
}
