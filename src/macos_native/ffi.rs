use core_foundation::{
    array::CFArray,
    base::{
        kCFAllocatorDefault, mach_port_t, CFAllocator, CFAllocatorRef, CFType, CFTypeID, CFTypeRef,
        TCFType,
    },
    declare_TCFType,
    dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionaryRef},
    impl_CFTypeDescription, impl_TCFType,
    mach_port::CFIndex,
    number::CFNumber,
    runloop::CFRunLoopRef,
    set::CFSetRef,
    string::{CFString, CFStringRef},
};
use libc::c_void;
use mach2::kern_return::{kern_return_t, KERN_SUCCESS};

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

impl IOHIDManager {
    pub fn create() -> Self {
        let manager = unsafe { IOHIDManagerCreate(kCFAllocatorDefault, 0) };

        // TODO: Error handling
        assert!(!manager.is_null());

        let manager = unsafe { IOHIDManager::wrap_under_create_rule(manager) };

        manager
    }

    pub fn set_device_matching(&self, matching: Option<&CFDictionary>) {
        unsafe {
            IOHIDManagerSetDeviceMatching(
                self.as_concrete_TypeRef(),
                matching
                    .map(|m| m.as_concrete_TypeRef())
                    .unwrap_or(std::ptr::null()),
            );
        }
    }
}

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

// TODO: Verify this
unsafe impl Send for IOHIDDevice {}

#[allow(non_camel_case_types)]
pub type io_object_t = mach_port_t;
#[allow(non_camel_case_types)]
pub type io_service_t = io_object_t;
#[allow(non_camel_case_types)]
pub type io_registry_entry_t = io_object_t;

pub type IOOptionBits = u32;

pub type IOReturn = kern_return_t;

#[allow(non_camel_case_types, non_upper_case_globals)]
pub const kIOReturnSuccess: kern_return_t = KERN_SUCCESS;

extern "C" {
    pub fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: IOOptionBits) -> IOHIDManagerRef;
    pub fn IOHIDManagerGetTypeID() -> CFTypeID;

    pub fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);

    pub fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;

    pub fn IORegistryEntryGetRegistryEntryID(
        entry: io_registry_entry_t,
        entryID: *mut u64,
    ) -> kern_return_t;

    pub fn IOServiceGetMatchingService(
        mainPort: mach_port_t,
        matching: CFDictionaryRef,
    ) -> io_service_t;

    pub fn IORegistryEntryIDMatching(entryID: u64) -> CFMutableDictionaryRef;

    pub fn IOHIDDeviceGetTypeID() -> CFTypeID;
    pub fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;

    pub fn IOHIDDeviceGetService(device: IOHIDDeviceRef) -> io_service_t;

    pub fn IOHIDDeviceCreate(allocator: CFAllocatorRef, service: io_service_t) -> IOHIDDeviceRef;
    pub fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;

    pub fn IOHIDDeviceScheduleWithRunLoop(
        device: IOHIDDeviceRef,
        runLoop: CFRunLoopRef,
        runLoopMode: CFStringRef,
    );
    pub fn IOHIDDeviceUnscheduleFromRunLoop(
        device: IOHIDDeviceRef,
        runLoop: CFRunLoopRef,
        runLoopMode: CFStringRef,
    );

    pub fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        reportLength: CFIndex,
        callback: Option<IOHIDReportCallback>,
        context: *mut c_void,
    );
    pub fn IOHIDDeviceRegisterRemovalCallback(
        device: IOHIDDeviceRef,
        callback: Option<IOHIDCallback>,
        context: *mut c_void,
    );
    pub fn IOHIDDeviceSetReport(
        device: IOHIDDeviceRef,
        reportType: IOHIDReportType,
        reportID: CFIndex,
        report: *const u8,
        reportLength: CFIndex,
    ) -> IOReturn;

    pub fn IOHIDDeviceGetReport(
        device: IOHIDDeviceRef,
        reportType: IOHIDReportType,
        reportID: CFIndex,
        report: *mut u8,
        pReportLength: *mut CFIndex,
    ) -> IOReturn;

    pub fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;
}

pub type IOHIDReportCallback = extern "C" fn(
    context: *mut c_void,
    result: IOReturn,
    sender: *mut c_void,
    type_: IOHIDReportType,
    reportID: u32,
    report: *mut u8,
    reportLength: CFIndex,
);

pub type IOHIDCallback = extern "C" fn(context: *mut c_void, result: IOReturn, sender: *mut c_void);

#[repr(C)]
#[allow(non_camel_case_types, dead_code)]
pub enum IOHIDReportType {
    kIOHIDReportTypeInput = 0,
    kIOHIDReportTypeOutput = 1,
    kIOHIDReportTypeFeature = 2,
}
