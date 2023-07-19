mod ffi;

use core_foundation::{
    array::CFArray,
    base::{kCFAllocatorDefault, CFGetTypeID, TCFType},
    data::CFData,
    dictionary::CFDictionary,
    mach_port::CFIndex,
    number::CFNumber,
    runloop::{
        kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopRunInMode,
        CFRunLoopRunResult, CFRunLoopSource, CFRunLoopSourceContext, CFRunLoopSourceCreate,
        CFRunLoopStop, CFRunLoopWakeUp,
    },
    set::{CFSet, CFSetGetValues},
    string::CFString,
};
use libc::{c_void, KERN_SUCCESS};
use mach2::port::MACH_PORT_NULL;
use std::{
    collections::VecDeque,
    ffi::{CStr, CString},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier, Condvar, Mutex,
    },
    time::Duration,
};

use crate::macos_native::ffi::kIOReturnSuccess;
use crate::macos_native::ffi::{
    kIOHIDProductIDKey, kIOHIDVendorIDKey, IOHIDDevice, IOHIDDeviceRef,
};
use crate::DeviceInfo;
use crate::HidDeviceBackendBase;
use crate::HidDeviceBackendMacos;
use crate::HidError;
use crate::HidResult;
use crate::WcharString;

use self::ffi::IOHIDDeviceRegisterRemovalCallback;
use self::ffi::IOHIDDeviceScheduleWithRunLoop;
use self::ffi::IOHIDDeviceSetReport;
use self::ffi::IOHIDReportType;
use self::ffi::IORegistryEntryIDMatching;
use self::ffi::IOReturn;
use self::ffi::IOServiceGetMatchingService;
use self::ffi::{io_service_t, IOHIDDeviceUnscheduleFromRunLoop};
use self::ffi::{IOHIDDeviceClose, IOHIDDeviceCreate};
use self::ffi::{IOHIDDeviceGetReport, IOOptionBits};
use self::ffi::{IOHIDDeviceRegisterInputReportCallback, IOHIDManager};

pub struct HidDevice {
    /// If set to true, reads will block until data is available
    blocking: bool,

    open_options: IOOptionBits,

    /// Handle of thread responsible for reading from the device
    reader_thread_handle: Option<std::thread::JoinHandle<()>>,

    /// State shared beween reader thread and others
    shared_state: Arc<SharedState>,
}

struct SharedState {
    // Run loop mode used to read from the device
    run_loop_mode: String,

    max_input_report_len: usize,

    // Reference to the run loop used to read from the device
    run_loop: Mutex<Option<LoopRef>>,

    device: Mutex<IOHIDDevice>,

    disconnected: AtomicBool,
    shutdown_thread: AtomicBool,
    shutdown_barrier: Barrier,

    // Condition variable linked to input_reports
    condition: std::sync::Condvar,
    input_reports: Mutex<VecDeque<InputReport>>,
}

struct LoopRef(CFRunLoopRef);

// TODO: Confirm that this is safe
unsafe impl Send for LoopRef {}
unsafe impl Sync for LoopRef {}

pub struct HidApiBackend;

impl HidApiBackend {
    pub fn get_hid_device_info_vector() -> HidResult<Vec<DeviceInfo>> {
        let manager = IOHIDManager::create();

        // Enumerate all devices
        manager.set_device_matching(None);

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
                    unsafe { Some(IOHIDDevice::wrap_under_get_rule(r)) }
                }
            })
            .collect();

        let mut result_list = Vec::with_capacity(num_devices);

        for device in device_list {
            let device_infos = get_device_infos(&device);

            result_list.extend_from_slice(&device_infos[..]);
        }

        Ok(result_list)
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

// Get device information for all usages
fn get_device_infos(device: &IOHIDDevice) -> Vec<DeviceInfo> {
    let mut result_list = Vec::new();

    #[allow(non_snake_case)]
    let kIOHIDPrimaryUsagePageKey = "PrimaryUsagePage";
    #[allow(non_snake_case)]
    let kIOHIDPrimaryUsageKey = "PrimaryUsage";

    let primary_usage_page = device
        .get_i32_property(kIOHIDPrimaryUsagePageKey)
        .unwrap_or_default();
    let primary_usage = device
        .get_i32_property(kIOHIDPrimaryUsageKey)
        .unwrap_or_default();

    let dev_info =
        create_device_info_with_usage(&device, primary_usage_page as u16, primary_usage as u16);

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

        let dev_info = create_device_info_with_usage(&device, usage_page as u16, usage as u16);
        result_list.push(dev_info);
    }

    result_list
}

fn create_device_info_with_usage(device: &IOHIDDevice, usage_page: u16, usage: u16) -> DeviceInfo {
    let vendor_id = device.get_i32_property(kIOHIDVendorIDKey);
    let product_id = device.get_i32_property(kIOHIDProductIDKey);

    let iokit_dev = unsafe { ffi::IOHIDDeviceGetService(device.as_concrete_TypeRef()) };

    let path = if iokit_dev != MACH_PORT_NULL {
        let mut entry_id = 0u64;
        let res = unsafe { ffi::IORegistryEntryGetRegistryEntryID(iokit_dev, &mut entry_id) };

        if res == KERN_SUCCESS {
            format!("DevSrvsID:{entry_id}")
        } else {
            String::new()
        }

        // TODO: Check resulting value
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
        self.set_report(IOHIDReportType::kIOHIDReportTypeOutput, data)
    }

    fn read(&self, buf: &mut [u8]) -> HidResult<usize> {
        let timeout = if self.blocking { -1 } else { 0 };

        self.read_timeout(buf, timeout)
    }

    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> HidResult<usize> {
        let mut report_list = self.shared_state.input_reports.lock().unwrap();

        if let Some(report) = report_list.pop_front() {
            let copy_len = buf.len().min(report.data.len());

            buf[..copy_len].copy_from_slice(&report.data[..copy_len]);

            return Ok(copy_len);
        }

        if self.shared_state.disconnected.load(Ordering::Relaxed) {
            return Err(HidError::HidApiError {
                message: "hid_read_timeout: device disconnected".to_string(),
            });
        }

        if self.shared_state.shutdown_thread.load(Ordering::Relaxed) {
            return Err(HidError::HidApiError {
                message: "hid_read_timeout: thread shutdown".to_string(),
            });
        }

        if timeout == -1 {
            // Blocking wait
            let res = self.shared_state.condition.wait(report_list);

            match res {
                Ok(mut report_list) => {
                    let report = report_list.pop_front().unwrap();

                    return Ok(return_data(&report, buf));
                }
                Err(_e) => {
                    return Err(HidError::HidApiError {
                        message: "hid_read_timeout: error waiting for more data".to_string(),
                    });
                }
            }
        } else if timeout > 0 {
            let res = self
                .shared_state
                .condition
                .wait_timeout(report_list, Duration::from_millis(timeout as u64));

            match res {
                Ok((mut report_list, _timeout_result)) => {
                    if let Some(report) = report_list.pop_front() {
                        return Ok(return_data(&report, buf));
                    } else {
                        // timeout
                        return Ok(0);
                    }
                }
                Err(_e) => {
                    return Err(HidError::HidApiError {
                        message: "hid_read_timeout: error waiting for more data".to_string(),
                    });
                }
            }
        } else {
            // Purely non-blocking
            Ok(0)
        }
    }

    fn send_feature_report(&self, data: &[u8]) -> HidResult<()> {
        let _ = self.set_report(IOHIDReportType::kIOHIDReportTypeFeature, data)?;

        Ok(())
    }

    fn get_feature_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        self.get_report(IOHIDReportType::kIOHIDReportTypeFeature, buf)
    }

    fn set_blocking_mode(&mut self, blocking: bool) -> HidResult<()> {
        self.blocking = blocking;
        Ok(())
    }

    // Get device information, for primary usage page
    fn get_device_info(&self) -> HidResult<DeviceInfo> {
        let dev = self.shared_state.device.lock().unwrap();

        let device_infos = get_device_infos(&dev);

        if device_infos.is_empty() {
            return Err(HidError::HidApiError {
                message: "hid_get_device_info: device not found".to_string(),
            });
        }

        Ok(device_infos[0].clone())
    }

    fn get_manufacturer_string(&self) -> HidResult<Option<String>> {
        let device_info = self.get_device_info()?;

        Ok(device_info.manufacturer_string.into())
    }

    fn get_product_string(&self) -> HidResult<Option<String>> {
        let device_info = self.get_device_info()?;

        Ok(device_info.product_string.into())
    }

    fn get_serial_number_string(&self) -> HidResult<Option<String>> {
        let device_info = self.get_device_info()?;

        Ok(device_info.serial_number.into())
    }

    fn get_report_descriptor(&self, buf: &mut [u8]) -> HidResult<usize> {
        let device = self.shared_state.device.lock().unwrap();

        let Some(data) = device.get_property(&CFString::from_static_string("ReportDescriptor")).and_then(|d| d.downcast_into::<CFData>()) else {
            return Err(HidError::HidApiError {
                message: "Failed to get kIOHIDReportDescriptorKey property".to_string(),
            });
        };

        let copy_len = buf.len().min(data.len() as usize);

        buf[..copy_len].copy_from_slice(&data[..copy_len]);

        Ok(copy_len)
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

impl Drop for HidDevice {
    fn drop(&mut self) {
        // TODO: Add check for macos version

        let run_loop_mode = CFString::new(&self.shared_state.run_loop_mode);

        if !self.shared_state.disconnected.load(Ordering::Relaxed) {
            // Callbacks are removed in the thread

            let run_loop_guard = self.shared_state.run_loop.lock().unwrap();
            let run_loop = run_loop_guard.as_ref().unwrap();
            let device = self.shared_state.device.lock().unwrap();

            // Move device to run loop of main thread
            unsafe {
                IOHIDDeviceUnscheduleFromRunLoop(
                    device.as_concrete_TypeRef(),
                    run_loop.0,
                    run_loop_mode.as_concrete_TypeRef(),
                );

                IOHIDDeviceScheduleWithRunLoop(
                    device.as_concrete_TypeRef(),
                    CFRunLoop::get_main().as_concrete_TypeRef(),
                    kCFRunLoopDefaultMode,
                );
            }
        }

        self.shared_state
            .shutdown_thread
            .store(true, Ordering::Relaxed);

        // TODO: Signal source
        {
            let run_loop = self.shared_state.run_loop.lock().unwrap();

            if let Some(run_loop) = run_loop.as_ref() {
                unsafe {
                    CFRunLoopWakeUp(run_loop.0);
                }
            }
        }

        // Wait for thread to shutdown
        self.shared_state.shutdown_barrier.wait();

        if let Some(handle) = self.reader_thread_handle.take() {
            handle.join().unwrap();
        }

        if !self.shared_state.disconnected.load(Ordering::Relaxed) {
            {
                let device = self.shared_state.device.lock().unwrap();

                let result =
                    unsafe { IOHIDDeviceClose(device.as_concrete_TypeRef(), self.open_options) };
            }
        }

        let mut input_reports = self.shared_state.input_reports.lock().unwrap();
        input_reports.clear();
    }
}

fn return_data(report: &InputReport, buf: &mut [u8]) -> usize {
    let copy_len = buf.len().min(report.data.len());

    buf[..copy_len].copy_from_slice(&report.data[..copy_len]);

    copy_len
}

impl HidDevice {
    pub(crate) fn open(vid: u16, pid: u16, sn: Option<&str>) -> HidResult<Self> {
        // TODO: Filter devices when enumerating

        let devices = HidApiBackend::get_hid_device_info_vector()?;

        let target_sn = match sn {
            Some(sn) => WcharString::String(sn.to_string()),
            None => WcharString::None,
        };

        let target_dev = devices.iter().find(|dev| {
            (dev.vendor_id == vid)
                && (dev.product_id == pid)
                && (sn.is_none() || (dev.serial_number == target_sn))
        });

        if let Some(dev) = target_dev {
            Self::open_path(dev.path.as_c_str())
        } else {
            Err(HidError::HidApiError {
                message: "device not found".into(),
            })
        }
    }

    pub(crate) fn open_path(device_path: &CStr) -> HidResult<Self> {
        let entry = open_service_registry_from_path(device_path);

        let handle = unsafe { IOHIDDeviceCreate(kCFAllocatorDefault, entry) };

        let device = unsafe { IOHIDDevice::wrap_under_create_rule(handle) };

        let ret = unsafe { ffi::IOHIDDeviceOpen(device.as_concrete_TypeRef(), 0) };

        if ret != kIOReturnSuccess {
            return Err(HidError::HidApiError {
                message: "failed to open IOHIDDevice from mach entry".into(),
            });
        }

        let max_input_report_len = device
            .get_i32_property("MaxInputReportSize")
            .unwrap_or_default();

        let run_loop_mode = format!("HIDAPI_{:p}", device.as_concrete_TypeRef());

        let barrier = Arc::new(std::sync::Barrier::new(2));

        let thread_barrier = barrier.clone();

        let thread_name = format!("hidapi-read-{:p}", device.as_concrete_TypeRef());

        let shared_state = Arc::new(SharedState {
            run_loop_mode,
            max_input_report_len: max_input_report_len as usize,
            device: Mutex::new(device),
            run_loop: Mutex::new(None),
            disconnected: AtomicBool::new(false),
            shutdown_thread: AtomicBool::new(false),
            shutdown_barrier: Barrier::new(2),
            condition: Condvar::new(),
            input_reports: Mutex::new(VecDeque::new()),
        });

        let thread_shared_state = shared_state.clone();

        let reader_handle = std::thread::Builder::new()
            .name(thread_name)
            .spawn(|| read_thread_fun(thread_barrier, thread_shared_state))
            .unwrap();

        // We don't care about the result here
        barrier.wait();

        Ok(Self {
            // TODO: Default value here
            blocking: false,
            // TODO: Set open options
            open_options: 0,
            reader_thread_handle: Some(reader_handle),
            shared_state,
        })
    }

    // See hidapi set_report()
    fn set_report(&self, report_type: IOHIDReportType, data: &[u8]) -> HidResult<usize> {
        if data.is_empty() {
            return Err(HidError::InvalidZeroSizeData);
        }

        let mut data_to_send = data;

        let report_id = data[0];

        if report_id == 0 {
            // Not using numbered reports, don't send the report number
            data_to_send = &data[1..];
        }

        if self.shared_state.disconnected.load(Ordering::SeqCst) {
            return Err(HidError::HidApiError {
                message: "Device is disconnected".to_string(),
            });
        }

        let device = self.shared_state.device.lock().unwrap();

        let res = unsafe {
            IOHIDDeviceSetReport(
                device.as_concrete_TypeRef(),
                report_type,
                report_id as _,
                data_to_send.as_ptr(),
                data_to_send.len() as _,
            )
        };

        if res != kIOReturnSuccess {
            return Err(HidError::HidApiError {
                message: format!("IOHIDDeviceSetReport failed: {res:#010x}"),
            });
        }

        Ok(data_to_send.len())
    }

    fn get_report(&self, report_type: IOHIDReportType, buf: &mut [u8]) -> HidResult<usize> {
        let report_id = buf[0];

        let mut report_data = &mut buf[..];

        if report_id == 0 {
            // Not using numbered reports, don't send the report number
            report_data = &mut buf[1..];
        }

        if self.shared_state.disconnected.load(Ordering::Relaxed) {
            return Err(HidError::HidApiError {
                message: "Device is disconnected".to_string(),
            });
        }

        let device = self.shared_state.device.lock().unwrap();

        let mut report_length = report_data.len() as isize;

        let res = unsafe {
            IOHIDDeviceGetReport(
                device.as_concrete_TypeRef(),
                report_type,
                report_id as _,
                report_data.as_mut_ptr(),
                &mut report_length,
            )
        };

        if res != kIOReturnSuccess {
            return Err(HidError::HidApiError {
                message: format!("IOHIDDeviceGetReport failed: {res:#010x}"),
            });
        }

        if report_id == 0 {
            // 0 report number still present at the beginning
            report_length += 1;
        }

        Ok(report_length as usize)
    }
}

struct InputReport {
    data: Vec<u8>,
}

// TODO: Figure out why parameters are unused
extern "C" fn hid_report_callback(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    _report_type: IOHIDReportType,
    _report_id: u32,
    report: *mut u8,
    report_length: CFIndex,
) {
    let shared_state = unsafe { &*(context as *const SharedState) };

    let data = unsafe { std::slice::from_raw_parts(report, report_length as usize) };

    let mut input_reports = shared_state.input_reports.lock().unwrap();

    // Ensure there are never more than 30 reports in the queue
    // Copied from hidapi
    if input_reports.len() == 30 {
        input_reports.pop_front();
    }

    input_reports.push_back(InputReport {
        data: data.to_vec(),
    });

    shared_state.condition.notify_one();
}

fn read_thread_fun(barrier: Arc<Barrier>, shared_state: Arc<SharedState>) {
    // This must live as long as the callback is registered
    let mut input_report_buffer = vec![0u8; shared_state.max_input_report_len];

    let ctx_ptr = Arc::as_ptr(&shared_state);
    let run_loop_mode = CFString::new(&shared_state.run_loop_mode);

    {
        let device = shared_state.device.lock().unwrap();

        // TODO: setup callbacks
        unsafe {
            IOHIDDeviceRegisterInputReportCallback(
                device.as_concrete_TypeRef(),
                input_report_buffer.as_mut_ptr(),
                input_report_buffer.len() as _,
                Some(hid_report_callback),
                ctx_ptr as *const c_void as *mut _,
            )
        }

        unsafe {
            IOHIDDeviceRegisterRemovalCallback(
                device.as_concrete_TypeRef(),
                Some(hid_removal_callback),
                ctx_ptr as *const c_void as *mut _,
            )
        }

        unsafe {
            IOHIDDeviceScheduleWithRunLoop(
                device.as_concrete_TypeRef(),
                CFRunLoopGetCurrent(),
                run_loop_mode.as_concrete_TypeRef(),
            );
        }

        let mut ctx = CFRunLoopSourceContext {
            version: 0,
            info: ctx_ptr as *const c_void as *mut _,
            retain: None,
            release: None,
            copyDescription: None,
            equal: None,
            hash: None,
            schedule: None,
            cancel: None,
            perform: perform_signal_callback,
        };

        let source = unsafe {
            CFRunLoopSourceCreate(kCFAllocatorDefault, 0 /* order */, &mut ctx)
        };

        let source = unsafe { CFRunLoopSource::wrap_under_create_rule(source) };

        let current_run_loop = CFRunLoop::get_current();

        current_run_loop.add_source(&source, run_loop_mode.as_concrete_TypeRef());
    }

    {
        let mut run_loop = shared_state.run_loop.lock().unwrap();

        *run_loop = Some(unsafe { LoopRef(CFRunLoopGetCurrent()) });
    }

    barrier.wait();

    while (!shared_state.shutdown_thread.load(Ordering::Relaxed))
        && (!shared_state.disconnected.load(Ordering::Relaxed))
    {
        // TODO: Verify timeout value
        let code = unsafe { CFRunLoopRunInMode(run_loop_mode.as_concrete_TypeRef(), 1000.0, 0) };

        // Return if the device has been disconnected
        if code == CFRunLoopRunResult::Finished as i32 {
            shared_state.disconnected.store(true, Ordering::Relaxed);
            break;
        }

        // Break if the run loop returns finished or stopped
        if code != CFRunLoopRunResult::HandledSource as i32
            && code != CFRunLoopRunResult::TimedOut as i32
        {
            shared_state.shutdown_thread.store(true, Ordering::Relaxed);
            break;
        }
    }

    // Notify that the thread is stopping
    {
        let _guard = shared_state.input_reports.lock().unwrap();
        shared_state.condition.notify_all();
    }

    {
        let device = shared_state.device.lock().unwrap();

        // TODO: Remove the callbacks
        unsafe {
            IOHIDDeviceRegisterInputReportCallback(
                device.as_concrete_TypeRef(),
                input_report_buffer.as_mut_ptr(),
                input_report_buffer.len() as _,
                None,
                std::ptr::null_mut(),
            );

            IOHIDDeviceRegisterRemovalCallback(
                device.as_concrete_TypeRef(),
                None,
                std::ptr::null_mut(),
            );
        }
    }

    shared_state.shutdown_barrier.wait();
}

extern "C" fn perform_signal_callback(context: *const c_void) {
    let shared_state = unsafe { &*(context as *const SharedState) };

    let run_loop_ref = shared_state.run_loop.lock().unwrap();

    if let Some(ref run_loop_ref) = *run_loop_ref {
        unsafe {
            CFRunLoopStop(run_loop_ref.0);
        }
    }
}

extern "C" fn hid_removal_callback(context: *mut c_void, _result: IOReturn, _sender: *mut c_void) {
    let shared_state = unsafe { &*(context as *const SharedState) };

    shared_state.disconnected.store(true, Ordering::Relaxed);

    // Stop the run loop for the device
    let run_loop = shared_state.run_loop.lock().unwrap();
    if let Some(ref run_lop) = *run_loop {
        unsafe {
            CFRunLoopStop(run_lop.0);
        }
    }
}

fn open_service_registry_from_path(path: &CStr) -> io_service_t {
    let path = path.to_str().unwrap();

    if path.starts_with("DevSrvsID:") {
        // TODO: Handle malformed path
        let entry_id: Option<u64> = path.trim_start_matches("DevSrvsID:").parse().ok();

        if let Some(entry_id) = entry_id {
            // 0 = kIOMasterPortDefault
            unsafe { IOServiceGetMatchingService(0, IORegistryEntryIDMatching(entry_id)) }
        } else {
            MACH_PORT_NULL
        }
    } else {
        // TODO: Compatibility with old HIDAPI versions (old IOService: format)
        MACH_PORT_NULL
    }
}
