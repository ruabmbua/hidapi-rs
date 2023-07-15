use std::ffi::c_char;

use core_foundation::{
    array::CFArray,
    base::TCFType,
    base::{mach_port_t, CFAllocatorRef, CFType, CFTypeID, CFTypeRef},
    declare_TCFType,
    dictionary::CFDictionaryRef,
    impl_CFTypeDescription, impl_TCFType,
    number::CFNumber,
    set::CFSetRef,
    string::{CFString, CFStringRef},
};
use libc::c_void;
use mach2::kern_return::kern_return_t;

#[allow(non_upper_case_globals)]
pub const kIOHIDVendorIDKey: &'static str = "VendorID";
#[allow(non_upper_case_globals)]
pub const kIOHIDProductIDKey: &'static str = "ProductID";

#[repr(C)]
pub struct __IOHIDManager(c_void);
pub type IOHIDManagerRef = *const __IOHIDManager;

declare_TCFType!(IOHIDManager, IOHIDManagerRef);
impl_TCFType!(IOHIDManager, IOHIDManagerRef, IOHIDManagerGetTypeID);
impl_CFTypeDescription!(IOHIDManager);

#[repr(C)]
pub struct __IOHIDDevice(c_void);
pub type IOHIDDeviceRef = *const __IOHIDDevice;

declare_TCFType!(IOHIDDevice, IOHIDDeviceRef);
impl_TCFType!(IOHIDDevice, IOHIDDeviceRef, IOHIDDeviceGetTypeID);
impl_CFTypeDescription!(IOHIDDevice);

impl IOHIDDevice {
    pub fn get_property(&self, key: &CFString) -> Option<CFType> {
        let property_ref = unsafe {
            IOHIDDeviceGetProperty(self.as_concrete_TypeRef(), key.as_concrete_TypeRef())
        };

        if property_ref.is_null() {
            None
        } else {
            let property = unsafe { CFType::wrap_under_get_rule(property_ref) };
            Some(property)
        }
    }

    pub fn get_i32_property(&self, key: &'static str) -> Option<i32> {
        self.get_property(&CFString::from_static_string(key))?
            .downcast_into::<CFNumber>()
            .and_then(|v| v.to_i32())
    }

    pub fn get_string_property(&self, key: &'static str) -> Option<String> {
        self.get_property(&CFString::from_static_string(key))?
            .downcast_into::<CFString>()
            .map(|v| v.to_string())
    }

    pub fn get_array_property(&self, arg: &'static str) -> Option<CFArray> {
        self.get_property(&CFString::from_static_string(arg))?
            .downcast_into::<CFArray>()
    }
}

pub type io_object_t = mach_port_t;
pub type io_service_t = io_object_t;
pub type io_registry_entry_t = io_object_t;

pub type IOOptionBits = u32;

pub type io_string_t = [c_char; 512];

extern "C" {
    pub fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: IOOptionBits) -> IOHIDManagerRef;
    pub fn IOHIDManagerGetTypeID() -> CFTypeID;

    pub fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);

    pub fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;

    pub fn IOHIDDeviceGetTypeID() -> CFTypeID;
    pub fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;

    pub fn IOHIDDeviceGetService(device: IOHIDDeviceRef) -> io_service_t;

    pub fn IORegistryEntryGetPath(
        entry: io_registry_entry_t,
        plane: *const c_char, // 128 bytes
        path: *mut c_char,    // 512 bytes
    ) -> kern_return_t;

    pub fn IORegistryEntryGetRegistryEntryID(
        entry: io_registry_entry_t,
        entryID: *mut u64,
    ) -> kern_return_t;
}
