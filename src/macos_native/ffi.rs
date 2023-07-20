use std::ffi::c_char;

use core_foundation::{
    base::{kCFAllocatorDefault, mach_port_t, CFAllocatorRef, CFType, CFTypeRef, TCFType},
    dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionaryRef},
    mach_port::CFIndex,
    number::CFNumber,
    runloop::{CFRunLoop, CFRunLoopRef},
    set::{CFSet, CFSetGetValues, CFSetRef},
    string::{CFString, CFStringRef},
    ConcreteCFType,
};
use libc::c_void;
use mach2::kern_return::{kern_return_t, KERN_SUCCESS};

pub use self::io_hid_manager::IOHIDManager;
pub use self::io_hid_manager::IOHIDManagerRef;

pub use self::io_hid_device::IOHIDDevice;
pub use self::io_hid_device::IOHIDDeviceRef;

#[allow(non_upper_case_globals)]
pub const kIOHIDVendorIDKey: &str = "VendorID";
#[allow(non_upper_case_globals)]
pub const kIOHIDProductIDKey: &str = "ProductID";

/// Seperate module for the IOHIDManager type,
/// so that we can allow non-snake case names.
#[allow(non_snake_case)]
mod io_hid_manager {
    use core_foundation::{
        base::TCFType, declare_TCFType, impl_CFTypeDescription, impl_TCFType, mach_port::CFTypeID,
    };
    use std::os::raw::c_void;

    #[repr(C)]
    pub struct __IOHIDManager(c_void);
    pub type IOHIDManagerRef = *const __IOHIDManager;

    declare_TCFType!(IOHIDManager, IOHIDManagerRef);
    impl_TCFType!(IOHIDManager, IOHIDManagerRef, IOHIDManagerGetTypeID);
    impl_CFTypeDescription!(IOHIDManager);

    extern "C" {
        fn IOHIDManagerGetTypeID() -> CFTypeID;
    }
}

impl IOHIDManager {
    pub fn create() -> Self {
        let manager = unsafe { IOHIDManagerCreate(kCFAllocatorDefault, 0) };

        // TODO: Error handling
        assert!(!manager.is_null());

        unsafe { IOHIDManager::wrap_under_create_rule(manager) }
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

    pub fn copy_devices(&self) -> Vec<IOHIDDevice> {
        let set: CFSet<IOHIDDeviceRef> = unsafe {
            let set = IOHIDManagerCopyDevices(self.as_concrete_TypeRef());

            CFSet::wrap_under_create_rule(set)
        };

        let num_devices = set.len();

        let mut device_refs = Vec::with_capacity(num_devices);

        unsafe {
            CFSetGetValues(
                set.as_concrete_TypeRef(),
                device_refs.as_mut_ptr() as *mut _,
            );
            device_refs.set_len(num_devices);
        }

        // Create a copy of the set, and wrap each device in a `IOHIDDevice` object.
        //
        // The documentation (https://developer.apple.com/documentation/corefoundation/1520437-cfsetgetvalues?language=objc)
        // seems to inidicate the `wrap_under_create_rule` should be used, but that leads to a crash.
        //
        // But, we create a copy of the set here, and this means we have to increase the ref count.
        // When the CFSet is dropped at the end of this function, the ref count will be decreased, and the values
        // in `device_refs` would become invalid. To avoid this, we increase the ref count here.
        device_refs
            .into_iter()
            .map(|d| unsafe { IOHIDDevice::wrap_under_get_rule(d) })
            .collect()
    }
}

#[allow(non_snake_case)]
mod io_hid_device {
    use std::ffi::c_void;

    use core_foundation::base::TCFType;
    use core_foundation::mach_port::CFTypeID;
    use core_foundation::{declare_TCFType, impl_CFTypeDescription, impl_TCFType};

    #[repr(C)]
    pub struct __IOHIDDevice(c_void);
    pub type IOHIDDeviceRef = *const __IOHIDDevice;

    declare_TCFType!(IOHIDDevice, IOHIDDeviceRef);
    impl_TCFType!(IOHIDDevice, IOHIDDeviceRef, IOHIDDeviceGetTypeID);
    impl_CFTypeDescription!(IOHIDDevice);

    extern "C" {
        fn IOHIDDeviceGetTypeID() -> CFTypeID;
    }
}

impl IOHIDDevice {
    pub fn untyped_property(&self, key: &CFString) -> Option<CFType> {
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

    pub fn property<T: ConcreteCFType>(&self, key: &CFString) -> Option<T> {
        self.untyped_property(key)?.downcast_into::<T>()
    }

    pub fn get_i32_property(&self, key: &'static str) -> Option<i32> {
        self.property::<CFNumber>(&CFString::from_static_string(key))
            .and_then(|v| v.to_i32())
    }

    pub fn get_string_property(&self, key: &'static str) -> Option<String> {
        self.property::<CFString>(&CFString::from_static_string(key))
            .map(|v| v.to_string())
    }

    /// Create a new IOHIDDevice from an IOService.
    ///
    /// # Panic
    /// This function will panic if IOHIDDeviceCreate returns a null pointer.
    pub fn create(allocator: CFAllocatorRef, service: io_service_t) -> Self {
        unsafe {
            let device = IOHIDDeviceCreate(allocator, service);
            IOHIDDevice::wrap_under_create_rule(device)
        }
    }

    pub fn open(&self, options: IOOptionBits) -> IOReturn {
        unsafe { IOHIDDeviceOpen(self.as_concrete_TypeRef(), options) }
    }

    pub fn close(&self, options: IOOptionBits) -> IOReturn {
        unsafe { IOHIDDeviceClose(self.as_concrete_TypeRef(), options) }
    }

    pub fn service(&self) -> io_service_t {
        unsafe { IOHIDDeviceGetService(self.as_concrete_TypeRef()) }
    }

    /// Register a callback to be called when a report is received.
    ///
    /// # Safety
    ///
    /// `report` and `context` must live as long as the callback is registered.
    ///
    /// The callback will be called from the CFRunLoop on which the device is registered,
    /// see `IOHIDDeviceScheduleWithRunLoop``
    pub unsafe fn register_input_report_callback(
        &self,
        report: &mut [u8],
        callback: IOHIDReportCallback,
        context: *mut c_void,
    ) {
        unsafe {
            IOHIDDeviceRegisterInputReportCallback(
                self.as_concrete_TypeRef(),
                report.as_mut_ptr(),
                report.len() as _,
                callback,
                context,
            );
        }
    }

    /// Register a callback to be called when a device is removed.
    ///
    /// # Safety
    ///
    /// `report` and `context` must live as long as the callback is registered.
    ///
    /// The callback will be called from the CFRunLoop on which the device is registered,
    /// see `IOHIDDeviceScheduleWithRunLoop``
    pub unsafe fn register_removal_callback(&self, callback: IOHIDCallback, context: *mut c_void) {
        unsafe {
            IOHIDDeviceRegisterRemovalCallback(self.as_concrete_TypeRef(), callback, context);
        }
    }

    pub fn set_report(
        &self,
        report_type: kIOHIDReportType,
        report_id: CFIndex,
        report: &[u8],
    ) -> IOReturn {
        unsafe {
            IOHIDDeviceSetReport(
                self.as_concrete_TypeRef(),
                report_type,
                report_id,
                report.as_ptr(),
                report.len() as _,
            )
        }
    }

    pub fn get_report(
        &self,
        report_type: kIOHIDReportType,
        report_id: CFIndex,
        report: &mut [u8],
    ) -> (CFIndex, IOReturn) {
        let mut length: CFIndex = report.len() as _;

        let res = unsafe {
            IOHIDDeviceGetReport(
                self.as_concrete_TypeRef(),
                report_type,
                report_id,
                report.as_mut_ptr(),
                &mut length,
            )
        };

        (length, res)
    }

    pub fn schedule_with_run_loop(&self, run_loop: &CFRunLoop, run_loop_mode: &CFString) {
        unsafe {
            IOHIDDeviceScheduleWithRunLoop(
                self.as_concrete_TypeRef(),
                run_loop.as_concrete_TypeRef(),
                run_loop_mode.as_concrete_TypeRef(),
            )
        }
    }

    pub fn unschedule_from_run_loop(&self, run_loop: &CFRunLoop, run_loop_mode: &CFString) {
        unsafe {
            IOHIDDeviceUnscheduleFromRunLoop(
                self.as_concrete_TypeRef(),
                run_loop.as_concrete_TypeRef(),
                run_loop_mode.as_concrete_TypeRef(),
            )
        }
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
    fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: IOOptionBits) -> IOHIDManagerRef;

    fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);

    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;

    pub fn IORegistryEntryGetRegistryEntryID(
        entry: io_registry_entry_t,
        entryID: *mut u64,
    ) -> kern_return_t;

    pub fn IOServiceGetMatchingService(
        mainPort: mach_port_t,
        matching: CFDictionaryRef,
    ) -> io_service_t;

    pub fn IORegistryEntryIDMatching(entryID: u64) -> CFMutableDictionaryRef;

    pub fn IORegistryEntrySearchCFProperty(
        entry: io_registry_entry_t,
        plane: *const c_char, // 128 bytes
        key: CFStringRef,
        allocator: CFAllocatorRef,
        options: IOOptionBits,
    ) -> CFTypeRef;

    fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;

    fn IOHIDDeviceGetService(device: IOHIDDeviceRef) -> io_service_t;

    fn IOHIDDeviceCreate(allocator: CFAllocatorRef, service: io_service_t) -> IOHIDDeviceRef;
    fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;

    fn IOHIDDeviceScheduleWithRunLoop(
        device: IOHIDDeviceRef,
        runLoop: CFRunLoopRef,
        runLoopMode: CFStringRef,
    );

    fn IOHIDDeviceUnscheduleFromRunLoop(
        device: IOHIDDeviceRef,
        runLoop: CFRunLoopRef,
        runLoopMode: CFStringRef,
    );

    fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        reportLength: CFIndex,
        callback: IOHIDReportCallback,
        context: *mut c_void,
    );

    fn IOHIDDeviceRegisterRemovalCallback(
        device: IOHIDDeviceRef,
        callback: IOHIDCallback,
        context: *mut c_void,
    );

    fn IOHIDDeviceSetReport(
        device: IOHIDDeviceRef,
        reportType: kIOHIDReportType,
        reportID: CFIndex,
        report: *const u8,
        reportLength: CFIndex,
    ) -> IOReturn;

    fn IOHIDDeviceGetReport(
        device: IOHIDDeviceRef,
        reportType: kIOHIDReportType,
        reportID: CFIndex,
        report: *mut u8,
        pReportLength: *mut CFIndex,
    ) -> IOReturn;

    fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;
}

pub type IOHIDReportCallback = Option<
    extern "C" fn(
        context: *mut c_void,
        result: IOReturn,
        sender: *mut c_void,
        type_: kIOHIDReportType,
        reportID: u32,
        report: *mut u8,
        reportLength: CFIndex,
    ),
>;

pub type IOHIDCallback =
    Option<extern "C" fn(context: *mut c_void, result: IOReturn, sender: *mut c_void)>;

#[repr(C)]
#[allow(non_camel_case_types, dead_code)]
pub enum kIOHIDReportType {
    Input = 0,
    Output = 1,
    Feature = 2,
}
