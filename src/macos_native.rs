mod ffi;

use core_foundation::array::CFArray;
use core_foundation::base::kCFAllocatorDefault;
use core_foundation::base::CFGetTypeID;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::set::CFSet;
use core_foundation::set::CFSetGetValues;
use core_foundation::string::CFString;
use libc::c_void;
use mach2::kern_return::KERN_SUCCESS;
use mach2::port::MACH_PORT_NULL;
use std::ffi::CStr;
use std::ffi::CString;

use crate::macos_native::ffi::{
    kIOHIDProductIDKey, kIOHIDVendorIDKey, IOHIDDevice, IOHIDDeviceRef,
};
use crate::DeviceInfo;
use crate::HidDevice;
use crate::HidDeviceBackendBase;
use crate::HidDeviceBackendMacos;
use crate::HidResult;
use crate::WcharString;

pub struct HidApiBackend;

impl HidApiBackend {
    pub fn get_hid_device_info_vector() -> HidResult<Vec<DeviceInfo>> {
        let manager = unsafe { ffi::IOHIDManagerCreate(kCFAllocatorDefault, 0) };

        // TODO: Error handling
        assert!(!manager.is_null());

        let manager = unsafe { ffi::IOHIDManager::wrap_under_create_rule(manager) };

        unsafe {
            ffi::IOHIDManagerSetDeviceMatching(manager.as_concrete_TypeRef(), std::ptr::null());
        }

        let set: CFSet<IOHIDDevice> = unsafe {
            CFSet::<IOHIDDevice>::wrap_under_create_rule(ffi::IOHIDManagerCopyDevices(
                manager.as_concrete_TypeRef(),
            ))
        };

        let num_devices = set.len();

        let mut device_refs: Vec<IOHIDDeviceRef> = Vec::with_capacity(num_devices);

        // TODO: Continue
        unsafe {
            CFSetGetValues(
                set.as_concrete_TypeRef(),
                device_refs.as_mut_ptr() as *mut *const c_void,
            );

            device_refs.set_len(num_devices);
        }

        let device_list: Vec<_> = device_refs
            .into_iter()
            .filter_map(|r| {
                if r.is_null() {
                    None
                } else {
                    unsafe { Some(IOHIDDevice::wrap_under_create_rule(r)) }
                }
            })
            .collect();

        let mut result_list = Vec::with_capacity(num_devices);

        for device in device_list {
            let kIOHIDPrimaryUsagePageKey = "PrimaryUsagePage";
            let kIOHIDPrimaryUsageKey = "PrimaryUsage";

            let primary_usage_page = device
                .get_i32_property(kIOHIDPrimaryUsagePageKey)
                .unwrap_or_default();
            let primary_usage = device
                .get_i32_property(kIOHIDPrimaryUsageKey)
                .unwrap_or_default();

            let dev_info = create_device_info_with_usage(
                &device,
                primary_usage_page as u16,
                primary_usage as u16,
            );

            result_list.push(dev_info);

            let usage_pairs = get_usage_pairs(&device);

            for usage_pair in &usage_pairs {
                let dict = unsafe { CFDictionary::wrap_under_get_rule(*usage_pair as _) };

                let Some(usage_page_ref) = dict.find(CFString::from_static_string("DeviceUsagePage")) else {
                    continue;
                };
                let Some(usage_ref) = dict.find(CFString::from_static_string("DeviceUsage")) else { continue;};

                if unsafe { CFGetTypeID(*usage_page_ref) } != CFNumber::type_id() {
                    continue;
                }

                if unsafe { CFGetTypeID(*usage_ref) } != CFNumber::type_id() {
                    continue;
                }

                let usage_page = unsafe { CFNumber::wrap_under_get_rule(*usage_page_ref as _) };
                let usage = unsafe { CFNumber::wrap_under_get_rule(*usage_ref as _) };

                let usage_page = usage_page.to_i32().unwrap();
                let usage = usage.to_i32().unwrap();

                if (usage_page == primary_usage_page) && (usage == primary_usage) {
                    continue;
                }

                let dev_info =
                    create_device_info_with_usage(&device, usage_page as u16, usage as u16);

                result_list.push(dev_info);
            }
        }

        Ok(result_list)
    }

    pub fn open(vid: u16, pid: u16) -> HidResult<HidDevice> {
        todo!()
    }

    pub fn open_serial(vid: u16, pid: u16, sn: &str) -> HidResult<HidDevice> {
        todo!()
    }

    pub fn open_path(device_path: &CStr) -> HidResult<HidDevice> {
        todo!()
    }
}

fn create_device_info_with_usage(device: &IOHIDDevice, usage_page: u16, usage: u16) -> DeviceInfo {
    let vendor_id = device.get_i32_property(kIOHIDVendorIDKey);
    let product_id = device.get_i32_property(kIOHIDProductIDKey);

    let iokit_dev = unsafe { ffi::IOHIDDeviceGetService(device.as_concrete_TypeRef()) };

    let path = if (iokit_dev != MACH_PORT_NULL) {
        let mut entry_id = 0u64;
        let res = unsafe { ffi::IORegistryEntryGetRegistryEntryID(iokit_dev, &mut entry_id) };

        format!("DevSrvsID:{entry_id}")
    } else {
        String::new()
    };

    let serial_number = device
        .get_string_property("SerialNumber")
        .unwrap_or_default();

    let manufacturer = device
        .get_string_property("Manufacturer")
        .unwrap_or_default();

    let product = device.get_string_property("Product").unwrap_or_default();

    let release_number = device.get_i32_property("VersionNumber").unwrap_or_default();

    let is_usb_hid =device.get_i32_property("bInterfaceClass") == Some(3) /* kUSBHIDClass */;

    let interface_number = if is_usb_hid {
        device.get_i32_property("bInterfaceNumber").unwrap_or(-1)
    } else {
        -1
    };

    let transport_str = device.get_string_property("Transport");

    let bus_type = match transport_str.as_deref() {
        Some("USB") => crate::BusType::Usb,
        Some("Bluetooth") => crate::BusType::Bluetooth,
        Some("I2C") => crate::BusType::I2c,
        Some("SPI") => crate::BusType::Spi,
        _ => crate::BusType::Unknown,
    };

    // TODO: Handle additional usage pages

    DeviceInfo {
        vendor_id: vendor_id.unwrap_or_default() as u16,
        product_id: product_id.unwrap_or_default() as u16,
        bus_type,
        path: CString::new(path).unwrap(),
        serial_number: crate::WcharString::String(serial_number),
        release_number: release_number as u16,
        manufacturer_string: WcharString::String(manufacturer),
        product_string: WcharString::String(product),
        usage_page: usage_page as u16,
        usage: usage as u16,
        interface_number,
    }
}

fn get_usage_pairs(device: &IOHIDDevice) -> CFArray {
    device.get_array_property("DeviceUsagePairs").unwrap()
}

impl HidDeviceBackendBase for HidDevice {
    fn write(&self, data: &[u8]) -> HidResult<usize> {
        todo!()
    }

    fn read(&self, buf: &mut [u8]) -> HidResult<usize> {
        todo!()
    }

    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> HidResult<usize> {
        todo!()
    }

    fn send_feature_report(&self, data: &[u8]) -> HidResult<()> {
        todo!()
    }

    fn get_feature_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        todo!()
    }

    fn set_blocking_mode(&self, blocking: bool) -> HidResult<()> {
        todo!()
    }

    fn get_device_info(&self) -> HidResult<DeviceInfo> {
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

    fn get_report_descriptor(&self, buf: &mut [u8]) -> HidResult<usize> {
        todo!()
    }
}

impl HidDeviceBackendMacos for HidDevice {
    fn get_location_id(&self) -> HidResult<u32> {
        todo!()
    }

    fn is_open_exclusive(&self) -> HidResult<bool> {
        todo!()
    }
}
