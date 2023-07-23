mod ffi;
use core_foundation::{
    array::CFArray,
    base::{kCFAllocatorDefault, CFGetTypeID, CFType, TCFType},
    data::CFData,
    dictionary::CFDictionary,
    mach_port::CFIndex,
    number::CFNumber,
    runloop::{
        kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopRunInMode, CFRunLoopRunResult, CFRunLoopSource,
        CFRunLoopSourceContext, CFRunLoopSourceCreate, CFRunLoopSourceSignal, CFRunLoopStop,
        CFRunLoopWakeUp,
    },
    string::CFString,
};
use mach2::port::MACH_PORT_NULL;

use std::{
    collections::VecDeque,
    ffi::{c_void, CStr, CString},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier, Condvar, Mutex,
    },
    time::Duration,
};

use crate::{
    macos_native::ffi::{
        kIOHIDProductIDKey, kIOHIDVendorIDKey, kIOReturnSuccess, IOHIDDevice,
        IORegistryEntryFromPath,
    },
    DeviceInfo, HidDeviceBackendBase, HidDeviceBackendMacos, HidError, HidResult, WcharString,
};

use self::ffi::{io_registry_entry_get_registry_entry_id, kIOHIDDeviceUsagePairsKey, IOOptionBits};
use self::ffi::{io_service_t, kIORegistryIterateParents};
use self::ffi::{kIOHIDManufacturerKey, kIOHIDSerialNumberKey, IORegistryEntryIDMatching};
use self::ffi::{kIOHIDProductKey, IORegistryEntrySearchCFProperty};
use self::ffi::{kIOHIDReportType, kIORegistryIterateRecursively};
use self::ffi::{kIOHIDTransportKey, IOServiceGetMatchingService};
use self::ffi::{kIOHIDVersionNumberKey, IOReturn};
use self::ffi::{kIOMainPortDefault, IOHIDManager};

#[derive(Debug)]
pub struct HidDevice {
    /// If set to true, reads will block until data is available
    blocking: bool,

    /// Options used to open the device
    open_options: IOOptionBits,

    /// Handle of thread responsible for reading from the device
    reader_thread_handle: Option<std::thread::JoinHandle<()>>,

    /// State shared beween reader thread and others
    shared_state: Arc<SharedState>,
}

#[derive(Debug)]
struct SharedState {
    // Run loop mode used to read from the device
    run_loop_mode: String,

    max_input_report_len: usize,

    // Reference to the run loop used to read from the device
    run_loop: Mutex<Option<WrappedCFRunLoop>>,
    source: Mutex<Option<LoopSource>>,

    device: Mutex<IOHIDDevice>,

    disconnected: AtomicBool,
    shutdown_thread: AtomicBool,
    shutdown_barrier: Barrier,

    // Condition variable linked to input_reports
    condition: std::sync::Condvar,
    input_reports: Mutex<VecDeque<Vec<u8>>>,
}

#[derive(Debug)]
struct WrappedCFRunLoop(CFRunLoop);

// This should be safe, according to documetation:
// https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/RunLoopManagement/RunLoopManagement.html#//apple_ref/doc/uid/10000057i-CH16-SW26>
//
// It is recommended that changing the configuration a run loop should be done from the thread that owns the run loop whenever possible.
// In the use here, we only use the wrapper to wake up the run loop, which is safe.
unsafe impl Send for WrappedCFRunLoop {}
unsafe impl Sync for WrappedCFRunLoop {}

/// Wrapper struct for a run loop source
struct LoopSource(CFRunLoopSource);

impl std::fmt::Debug for LoopSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source = format!("{:p}", self.0.as_CFTypeRef());

        f.debug_tuple("LoopSource").field(&source).finish()
    }
}

// The wrapper is only used to signal the source, which
// is safe to do from any thread.
unsafe impl Send for LoopSource {}
unsafe impl Sync for LoopSource {}

pub struct HidApiBackend;

impl HidApiBackend {
    pub fn get_hid_device_info_vector() -> HidResult<Vec<DeviceInfo>> {
        let manager = IOHIDManager::create();

        // Enumerate all devices
        manager.set_device_matching(None);

        let device_list = manager.copy_devices();

        let mut result_list = Vec::with_capacity(device_list.len());

        for device in device_list {
            // Some devices can appear multiple times, if they have multiple usage pairs
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
        create_device_info_with_usage(device, primary_usage_page as u16, primary_usage as u16);

    result_list.push(dev_info);

    let usage_pairs = get_usage_pairs(device);

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

        let dev_info = create_device_info_with_usage(device, usage_page as u16, usage as u16);
        result_list.push(dev_info);
    }

    result_list
}

fn create_device_info_with_usage(device: &IOHIDDevice, usage_page: u16, usage: u16) -> DeviceInfo {
    let vendor_id = device.get_i32_property(kIOHIDVendorIDKey);
    let product_id = device.get_i32_property(kIOHIDProductIDKey);

    let iokit_dev = device.service();

    let path = if let Some(service) = iokit_dev {
        let entry_id = io_registry_entry_get_registry_entry_id(service);

        entry_id
            .map(|id| format!("DevSrvsID:{id}"))
            .unwrap_or_default()
    } else {
        String::new()
    };

    let serial_number = device
        .get_string_property(kIOHIDSerialNumberKey)
        .unwrap_or_default();

    let manufacturer = device
        .get_string_property(kIOHIDManufacturerKey)
        .unwrap_or_default();

    let product = device
        .get_string_property(kIOHIDProductKey)
        .unwrap_or_default();

    let release_number = device
        .get_i32_property(kIOHIDVersionNumberKey)
        .unwrap_or_default();

    let transport_str = device.get_string_property(kIOHIDTransportKey);

    let bus_type = match transport_str.as_deref() {
        Some("USB") => crate::BusType::Usb,
        Some("Bluetooth") => crate::BusType::Bluetooth,
        Some("I2C") => crate::BusType::I2c,
        Some("SPI") => crate::BusType::Spi,
        _ => crate::BusType::Unknown,
    };

    let is_usb_hid = bus_type == crate::BusType::Usb;

    // TODO: This should not be represented as -1, but as None.
    //       -1 is used to stay compatible with hidapi.
    let interface_number = if is_usb_hid {
        get_usb_interface_number(device).unwrap_or(-1)
    } else {
        -1
    };

    DeviceInfo {
        vendor_id: vendor_id.unwrap_or_default() as u16,
        product_id: product_id.unwrap_or_default() as u16,
        bus_type,
        path: CString::new(path).unwrap(),
        serial_number: crate::WcharString::String(serial_number),
        release_number: release_number as u16,
        manufacturer_string: WcharString::String(manufacturer),
        product_string: WcharString::String(product),
        usage_page,
        usage,
        interface_number,
    }
}

fn get_usb_interface_number(device: &IOHIDDevice) -> Option<i32> {
    let registry_entry = device.service()?;

    let plane = CString::new("IOService").unwrap();

    // This property is not available for a IOHIDDevice. It is available for a IOUSBHostInterface,
    // so we need to get the parent interface and check the property there.
    let property = unsafe {
        let ret_val = IORegistryEntrySearchCFProperty(
            registry_entry,
            plane.as_ptr(),
            CFString::from_static_string("bInterfaceNumber").as_concrete_TypeRef(),
            kCFAllocatorDefault,
            kIORegistryIterateParents | kIORegistryIterateRecursively,
        );

        if !ret_val.is_null() {
            Some(CFType::wrap_under_create_rule(ret_val))
        } else {
            None
        }
    };

    property
        .and_then(|rv| rv.downcast_into::<CFNumber>())
        .and_then(|n| n.to_i32())
}

fn get_usage_pairs(device: &IOHIDDevice) -> CFArray {
    device
        .property(&CFString::from_static_string(kIOHIDDeviceUsagePairsKey))
        .unwrap()
}

impl HidDeviceBackendBase for HidDevice {
    fn write(&self, data: &[u8]) -> HidResult<usize> {
        self.set_report(kIOHIDReportType::Output, data)
    }

    fn read(&self, buf: &mut [u8]) -> HidResult<usize> {
        let timeout = if self.blocking { -1 } else { 0 };

        self.read_timeout(buf, timeout)
    }

    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> HidResult<usize> {
        let mut report_list = self.shared_state.input_reports.lock().unwrap();

        if let Some(report) = report_list.pop_front() {
            let copy_len = buf.len().min(report.len());

            buf[..copy_len].copy_from_slice(&report[..copy_len]);

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

                    Ok(return_data(&report, buf))
                }
                Err(_e) => Err(HidError::HidApiError {
                    message: "hid_read_timeout: error waiting for more data".to_string(),
                }),
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
        let _ = self.set_report(kIOHIDReportType::Feature, data)?;

        Ok(())
    }

    fn get_feature_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        self.get_report(kIOHIDReportType::Feature, buf)
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

        let Some(data) = device.property::<CFData>(&CFString::from_static_string("ReportDescriptor")) else {
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
        let run_loop_mode = CFString::new(&self.shared_state.run_loop_mode);

        // We currently assume that macOS is newer than 10.10 (see hid_close in hidapi)
        if !self.shared_state.disconnected.load(Ordering::Relaxed) {
            // Callbacks are removed in the thread

            let run_loop_guard = self.shared_state.run_loop.lock().unwrap();
            let run_loop = run_loop_guard.as_ref().unwrap();
            let device = self.shared_state.device.lock().unwrap();

            // Move device to run loop of main thread
            device.unschedule_from_run_loop(&run_loop.0, &run_loop_mode);

            // unsafe because of access to kCFRunLoopDefaultMode static
            let default_mode = unsafe {
                // create rule is used because this is a static string,
                // the reference count should not be incremented.
                CFString::wrap_under_create_rule(kCFRunLoopDefaultMode)
            };
            device.schedule_with_run_loop(&CFRunLoop::get_main(), &default_mode);
        }

        self.shared_state
            .shutdown_thread
            .store(true, Ordering::Relaxed);

        {
            let source = self.shared_state.source.lock().unwrap();

            if let Some(source) = source.as_ref() {
                unsafe { CFRunLoopSourceSignal(source.0.as_concrete_TypeRef()) }
            }
        }

        {
            let run_loop = self.shared_state.run_loop.lock().unwrap();

            if let Some(run_loop) = run_loop.as_ref() {
                unsafe {
                    CFRunLoopWakeUp(run_loop.0.as_concrete_TypeRef());
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

                // Error is also ignored in hid_close
                let _result = device.close(self.open_options);
            }
        }

        let mut input_reports = self.shared_state.input_reports.lock().unwrap();
        input_reports.clear();
    }
}

fn return_data(report: &[u8], buf: &mut [u8]) -> usize {
    let copy_len = buf.len().min(report.len());

    buf[..copy_len].copy_from_slice(&report[..copy_len]);

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
        let entry =
            open_service_registry_from_path(device_path).ok_or_else(|| HidError::HidApiError {
                message: format!("Failed to open IOHIDDevice from path {device_path:?}"),
            })?;

        let device = IOHIDDevice::create(None, entry);

        let ret = device.open(0);

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
            source: Mutex::new(None),
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
    fn set_report(&self, report_type: kIOHIDReportType, data: &[u8]) -> HidResult<usize> {
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

        let res = device.set_report(report_type, report_id as _, data_to_send);

        if res != kIOReturnSuccess {
            return Err(HidError::HidApiError {
                message: format!("IOHIDDeviceSetReport failed: {res:#010x}"),
            });
        }

        Ok(data_to_send.len())
    }

    fn get_report(&self, report_type: kIOHIDReportType, buf: &mut [u8]) -> HidResult<usize> {
        let report_id = buf[0];

        let mut report_data = &mut buf[..];

        if report_id == 0 {
            // Not using numbered reports, don't send the report number
            report_data = &mut buf[1..];
        }

        println!("Report id: {}", report_id);

        if self.shared_state.disconnected.load(Ordering::Relaxed) {
            return Err(HidError::HidApiError {
                message: "Device is disconnected".to_string(),
            });
        }

        let device = self.shared_state.device.lock().unwrap();

        let (mut report_length, res) = device.get_report(report_type, report_id as _, report_data);

        if res != kIOReturnSuccess {
            return Err(HidError::HidApiError {
                message: format!("IOHIDDeviceGetReport failed: {res:#010x}"),
            });
        }

        if report_id == 0 {
            // 0 report number still present at the beginning
            report_length += 1;

            assert_eq!(buf[0], 0);
        }

        Ok(report_length as usize)
    }
}

// TODO: Figure out why parameters are unused
extern "C" fn hid_report_callback(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    _report_type: kIOHIDReportType,
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

    input_reports.push_back(data.to_vec());

    shared_state.condition.notify_one();
}

fn read_thread_fun(barrier: Arc<Barrier>, shared_state: Arc<SharedState>) {
    // This must live as long as the callback is registered
    let mut input_report_buffer = vec![0u8; shared_state.max_input_report_len];

    // This must live as long as the callback is registered
    let ctx_ptr = Arc::as_ptr(&shared_state) as *const c_void as *mut c_void;

    let input_report_context = shared_state.clone();

    let run_loop_mode = CFString::new(&shared_state.run_loop_mode);

    let mut _input_report_callback = None;

    {
        let device = shared_state.device.lock().unwrap();

        _input_report_callback = Some(device.register_input_report_callback(
            &mut input_report_buffer,
            Some(hid_report_callback),
            input_report_context,
        ));

        unsafe { device.register_removal_callback(Some(hid_removal_callback), ctx_ptr) }

        device.schedule_with_run_loop(&CFRunLoop::get_current(), &run_loop_mode);

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

        let mut shared_source = shared_state.source.lock().unwrap();
        *shared_source = Some(LoopSource(source));
    }

    {
        let mut run_loop = shared_state.run_loop.lock().unwrap();

        *run_loop = Some(WrappedCFRunLoop(CFRunLoop::get_current()));
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

        // unregister the input report callback
        drop(_input_report_callback);

        unsafe {
            device.register_removal_callback(None, std::ptr::null_mut());
        }
    }

    shared_state.shutdown_barrier.wait();
}

extern "C" fn perform_signal_callback(context: *const c_void) {
    let shared_state = unsafe { &*(context as *const SharedState) };

    let run_loop_ref = shared_state.run_loop.lock().unwrap();

    if let Some(ref run_loop_ref) = *run_loop_ref {
        unsafe {
            CFRunLoopStop(run_loop_ref.0.as_concrete_TypeRef());
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
            CFRunLoopStop(run_lop.0.as_concrete_TypeRef());
        }
    }
}

fn open_service_registry_from_path(path: &CStr) -> Option<io_service_t> {
    // TODO: Handle malformed path
    let path = path.to_str().ok()?;

    if path.starts_with("DevSrvsID:") {
        // TODO: Handle malformed path
        let entry_id: u64 = path.trim_start_matches("DevSrvsID:").parse().ok()?;

        let service = unsafe {
            IOServiceGetMatchingService(kIOMainPortDefault, IORegistryEntryIDMatching(entry_id))
        };

        Some(service)
    } else {
        // TODO: Compatibility with old HIDAPI versions (old IOService: format)
        let service = unsafe { IORegistryEntryFromPath(kIOMainPortDefault, path.as_ptr() as _) };

        if service != MACH_PORT_NULL {
            Some(service)
        } else {
            None
        }
    }
}
