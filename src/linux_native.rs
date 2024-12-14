//! This backend uses libudev to discover devices and then talks to hidraw directly

mod ioctl;

use std::{
    cell::{Cell, Ref, RefCell},
    ffi::{CStr, CString, OsStr, OsString},
    fs::{File, OpenOptions},
    io::{Cursor, Read, Seek, SeekFrom},
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
        unix::{ffi::OsStringExt, fs::OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

use nix::{
    errno::Errno,
    poll::{poll, PollFd, PollFlags},
    sys::stat::{fstat, major, minor},
    unistd::{read, write},
};

use super::{BusType, DeviceInfo, HidDeviceBackendBase, HidError, HidResult, WcharString};
use ioctl::{
    hidraw_ioc_get_feature, hidraw_ioc_get_input, hidraw_ioc_grdescsize, hidraw_ioc_set_feature,
    hidraw_ioc_set_output,
};

// Bus values from linux/input.h
const BUS_USB: u16 = 0x03;
const BUS_BLUETOOTH: u16 = 0x05;
const BUS_I2C: u16 = 0x18;
const BUS_SPI: u16 = 0x1C;

pub struct HidApiBackend;

impl HidApiBackend {
    pub fn get_hid_device_info_vector(vid: u16, pid: u16) -> HidResult<Vec<DeviceInfo>> {
        // The C version assumes these can't fail, and they should only fail in case
        // of memory allocation issues, at which point maybe we should panic
        let mut enumerator = match udev::Enumerator::new() {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };
        enumerator.match_subsystem("hidraw").unwrap();
        let scan = match enumerator.scan_devices() {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };

        let devices = scan
            .filter_map(|device| device_to_hid_device_info(&device))
            .flatten()
            .filter(|device| vid == 0 || device.vendor_id == vid)
            .filter(|device| pid == 0 || device.product_id == pid)
            .collect::<Vec<_>>();

        Ok(devices)
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

fn device_to_hid_device_info(raw_device: &udev::Device) -> Option<Vec<DeviceInfo>> {
    let mut infos = Vec::new();

    // We're given the hidraw device, but we actually want to go and check out
    // the info for the parent hid device.
    let device = match raw_device.parent_with_subsystem("hid") {
        Ok(Some(dev)) => dev,
        _ => return None,
    };

    let (bus, vid, pid) = match device
        .property_value("HID_ID")
        .and_then(|s| s.to_str())
        .and_then(parse_hid_vid_pid)
    {
        Some(t) => t,
        None => return None,
    };
    let bus_type = match bus {
        BUS_USB => BusType::Usb,
        BUS_BLUETOOTH => BusType::Bluetooth,
        BUS_I2C => BusType::I2c,
        BUS_SPI => BusType::Spi,
        _ => return None,
    };
    let name = match device.property_value("HID_NAME") {
        Some(name) => name,
        None => return None,
    };
    let serial = match device.property_value("HID_UNIQ") {
        Some(serial) => serial,
        None => return None,
    };
    let path = match raw_device
        .devnode()
        .map(|p| p.as_os_str().to_os_string().into_vec())
        .map(CString::new)
    {
        Some(Ok(s)) => s,
        None | Some(Err(_)) => return None,
    };

    // Thus far we've gathered all the common attributes.
    let info = DeviceInfo {
        path,
        vendor_id: vid,
        product_id: pid,
        serial_number: osstring_to_string(serial.into()),
        release_number: 0,
        manufacturer_string: WcharString::None,
        product_string: WcharString::None,
        usage_page: 0,
        usage: 0,
        interface_number: -1,
        bus_type,
    };

    // USB has a bunch more information but everything else gets the same empty
    // manufacturer and the product we read from the property above.
    let info = match bus_type {
        BusType::Usb => fill_in_usb(raw_device, info, name),
        _ => DeviceInfo {
            manufacturer_string: WcharString::String("".into()),
            product_string: osstring_to_string(name.into()),
            ..info
        },
    };

    if let Ok(descriptor) = HidrawReportDescriptor::from_syspath(raw_device.syspath()) {
        let mut usages = descriptor.usages();

        // Get the first usage page and usage for our current DeviceInfo
        if let Some((usage_page, usage)) = usages.next() {
            infos.push(DeviceInfo {
                usage_page,
                usage,
                ..info
            });

            // Now we can create DeviceInfo for all the other usages
            for (usage_page, usage) in usages {
                let prev = infos.last().unwrap();

                infos.push(DeviceInfo {
                    usage_page,
                    usage,
                    ..prev.clone()
                })
            }
        }
    } else {
        infos.push(info);
    }

    Some(infos)
}

/// Fill in the extra information that's available for a USB device.
fn fill_in_usb(device: &udev::Device, info: DeviceInfo, name: &OsStr) -> DeviceInfo {
    let usb_dev = match device.parent_with_subsystem_devtype("usb", "usb_device") {
        Ok(Some(dev)) => dev,
        Ok(None) | Err(_) => {
            return DeviceInfo {
                manufacturer_string: WcharString::String("".into()),
                product_string: osstring_to_string(name.into()),
                ..info
            }
        }
    };
    let manufacturer_string = attribute_as_wchar(&usb_dev, "manufacturer");
    let product_string = attribute_as_wchar(&usb_dev, "product");
    let release_number = attribute_as_u16(&usb_dev, "bcdDevice").unwrap_or(0);
    let interface_number = device
        .parent_with_subsystem_devtype("usb", "usb_interface")
        .ok()
        .flatten()
        .and_then(|ref dev| attribute_as_i32(dev, "bInterfaceNumber"))
        .unwrap_or(-1);

    DeviceInfo {
        release_number,
        manufacturer_string,
        product_string,
        interface_number,
        ..info
    }
}

#[derive(Default)]
struct HidrawReportDescriptor(Vec<u8>);

impl HidrawReportDescriptor {
    /// Open and parse given the "base" sysfs of the device
    pub fn from_syspath(syspath: &Path) -> HidResult<Self> {
        let path = syspath.join("device/report_descriptor");
        let mut f = File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;

        Ok(HidrawReportDescriptor(buf))
    }

    /// Create a descriptor from a slice
    ///
    /// It returns an error if the value slice is too large for it to be a HID
    /// descriptor
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_slice(value: &[u8]) -> HidResult<Self> {
        Ok(HidrawReportDescriptor(value.to_vec()))
    }

    pub fn usages(&self) -> impl Iterator<Item = (u16, u16)> + '_ {
        UsageIterator {
            usage_page: 0,
            cursor: Cursor::new(&self.0),
        }
    }
}

/// Iterates over the values in a HidrawReportDescriptor
struct UsageIterator<'a> {
    usage_page: u16,
    cursor: Cursor<&'a Vec<u8>>,
}

impl<'a> Iterator for UsageIterator<'a> {
    type Item = (u16, u16);

    fn next(&mut self) -> Option<Self::Item> {
        let (usage_page, page) = match next_hid_usage(&mut self.cursor, self.usage_page) {
            Some(n) => n,
            None => return None,
        };

        self.usage_page = usage_page;
        Some((usage_page, page))
    }
}

// This comes from hidapi which apparently comes from Apple's implementation of
// this
fn next_hid_usage(cursor: &mut Cursor<&Vec<u8>>, mut usage_page: u16) -> Option<(u16, u16)> {
    let mut usage = None;
    let mut usage_pair = None;
    let initial = cursor.position() == 0;

    while let Some(Ok(key)) = cursor.bytes().next() {
        // The amount to skip is calculated based off of the start of the
        // iteration so we need to keep track of that.
        let position = cursor.position() - 1;
        let key_cmd = key & 0xfc;

        let (data_len, key_size) = match hid_item_size(key, cursor) {
            Some(v) => v,
            None => return None,
        };

        match key_cmd {
            // Usage Page 6.2.2.7 (Global)
            0x4 => {
                usage_page = match hid_report_bytes(cursor, data_len) {
                    Ok(v) => v as u16,
                    Err(_) => break,
                }
            }
            // Usage 6.2.2.8 (Local)
            0x8 => {
                usage = match hid_report_bytes(cursor, data_len) {
                    Ok(v) => Some(v as u16),
                    Err(_) => break,
                }
            }
            // Collection 6.2.2.4 (Main)
            0xa0 => {
                // Usage is a Local Item, unset it
                if let Some(u) = usage.take() {
                    usage_pair = Some((usage_page, u))
                }
            }
            // Input 6.2.2.4 (Main)
		        0x80 |
            // Output 6.2.2.4 (Main)
		        0x90 |
            // Feature 6.2.2.4 (Main)
		        0xb0 |
            // End Collection 6.2.2.4 (Main)
	    0xc0  =>  {
		// Usage is a Local Item, unset it
                usage.take();
            }
            _ => {}
        }

        if cursor
            .seek(SeekFrom::Start(position + (data_len + key_size) as u64))
            .is_err()
        {
            return None;
        }

        if let Some((usage_page, usage)) = usage_pair {
            return Some((usage_page, usage));
        }
    }

    if let (true, Some(usage)) = (initial, usage) {
        return Some((usage_page, usage));
    }

    None
}

/// Gets the size of the HID item at the given position
///
/// Returns data_len and key_size when successful
fn hid_item_size(key: u8, cursor: &mut Cursor<&Vec<u8>>) -> Option<(usize, usize)> {
    // Long Item. Next byte contains the length of the data section.
    if (key & 0xf0) == 0xf0 {
        if let Some(Ok(len)) = cursor.bytes().next() {
            return Some((len.into(), 3));
        }

        // Malformed report
        return None;
    }

    // Short Item. Bottom two bits contains the size code
    match key & 0x03 {
        v @ 0..=2 => Some((v.into(), 1)),
        3 => Some((4, 1)),
        _ => unreachable!(), // & 0x03 means this can't happen
    }
}

/// Get the bytes from a HID report descriptor
///
/// Must only be called with `num_bytes` 0, 1, 2 or 4.
fn hid_report_bytes(cursor: &mut Cursor<&Vec<u8>>, num_bytes: usize) -> HidResult<u32> {
    let mut bytes: [u8; 4] = [0; 4];
    cursor.read_exact(&mut bytes[..num_bytes])?;

    Ok(u32::from_le_bytes(bytes))
}

/// Get the attribute from the device and convert it into a [`WcharString`].
fn attribute_as_wchar(dev: &udev::Device, attr: &str) -> WcharString {
    dev.attribute_value(attr)
        .map(Into::into)
        .map(osstring_to_string)
        .unwrap_or(WcharString::None)
}

/// Get the attribute from the device and convert it into a i32
///
/// On error or if the attribute is not found, it returns None.
fn attribute_as_i32(dev: &udev::Device, attr: &str) -> Option<i32> {
    dev.attribute_value(attr)
        .and_then(OsStr::to_str)
        .and_then(|v| i32::from_str_radix(v, 16).ok())
}

/// Get the attribute from the device and convert it into a u16
///
/// On error or if the attribute is not found, it returns None.
fn attribute_as_u16(dev: &udev::Device, attr: &str) -> Option<u16> {
    dev.attribute_value(attr)
        .and_then(OsStr::to_str)
        .and_then(|v| u16::from_str_radix(v, 16).ok())
}

/// Convert a [`OsString`] into a [`WcharString`]
fn osstring_to_string(s: OsString) -> WcharString {
    match s.into_string() {
        Ok(s) => WcharString::String(s),
        Err(_) => panic!("udev strings should always be utf8"),
    }
}

/// Parse a HID_ID string to find the bus type, the vendor and product id
///
/// These strings would be of the format
///     type vendor   product
///     0003:000005AC:00008242
fn parse_hid_vid_pid(s: &str) -> Option<(u16, u16, u16)> {
    let mut elems = s.split(':').map(|s| u16::from_str_radix(s, 16));
    let devtype = elems.next()?.ok()?;
    let vendor = elems.next()?.ok()?;
    let product = elems.next()?.ok()?;

    Some((devtype, vendor, product))
}

/// Object for accessing the HID device
pub struct HidDevice {
    blocking: Cell<bool>,
    fd: OwnedFd,
    info: RefCell<Option<DeviceInfo>>,
}

unsafe impl Send for HidDevice {}

// API for the library to call us, or for internal uses
impl HidDevice {
    pub(crate) fn open(vid: u16, pid: u16, sn: Option<&str>) -> HidResult<Self> {
        for device in HidApiBackend::get_hid_device_info_vector(0, 0)?
            .iter()
            .filter(|device| device.vendor_id == vid && device.product_id == pid)
        {
            match (sn, &device.serial_number) {
                (None, _) => return Self::open_path(&device.path),
                (Some(sn), WcharString::String(serial_number)) if sn == serial_number => {
                    return Self::open_path(&device.path)
                }
                _ => continue,
            };
        }

        Err(HidError::HidApiError {
            message: "device not found".into(),
        })
    }

    pub(crate) fn open_path(device_path: &CStr) -> HidResult<HidDevice> {
        // Paths on Linux can be anything but devnode paths are going to be ASCII
        let path = device_path.to_str().expect("path must be utf-8");
        let fd: OwnedFd = match OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
        {
            Ok(f) => f.into(),
            Err(e) => {
                return Err(HidError::HidApiError {
                    message: format!("failed to open device with path {path}: {e}"),
                });
            }
        };

        let mut size = 0_i32;
        if let Err(e) = unsafe { hidraw_ioc_grdescsize(fd.as_raw_fd(), &mut size) } {
            return Err(HidError::HidApiError {
                message: format!("ioctl(GRDESCSIZE) error for {path}, not a HIDRAW device?: {e}"),
            });
        }

        Ok(Self {
            blocking: Cell::new(true),
            fd,
            info: RefCell::new(None),
        })
    }

    fn info(&self) -> HidResult<Ref<DeviceInfo>> {
        if self.info.borrow().is_none() {
            let info = self.get_device_info()?;
            self.info.replace(Some(info));
        }

        let info = self.info.borrow();
        Ok(Ref::map(info, |i: &Option<DeviceInfo>| i.as_ref().unwrap()))
    }
}

impl AsFd for HidDevice {
    fn as_fd(&self) -> BorrowedFd {
        self.fd.as_fd()
    }
}

impl HidDeviceBackendBase for HidDevice {
    fn write(&self, data: &[u8]) -> HidResult<usize> {
        if data.is_empty() {
            return Err(HidError::InvalidZeroSizeData);
        }

        Ok(write(self.fd.as_raw_fd(), data)?)
    }

    fn read(&self, buf: &mut [u8]) -> HidResult<usize> {
        // If the caller asked for blocking, -1 makes us wait forever
        let timeout = if self.blocking.get() { -1 } else { 0 };
        self.read_timeout(buf, timeout)
    }

    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> HidResult<usize> {
        let pollfd = PollFd::new(&self.fd, PollFlags::POLLIN);
        let res = poll(&mut [pollfd], timeout)?;

        if res == 0 {
            return Ok(0);
        }

        let events = pollfd
            .revents()
            .map(|e| e.intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL));

        if events.is_none() || events == Some(true) {
            return Err(HidError::HidApiError {
                message: "unexpected poll error (device disconnected)".into(),
            });
        }

        match read(self.fd.as_raw_fd(), buf) {
            Ok(w) => Ok(w),
            Err(Errno::EAGAIN) | Err(Errno::EINPROGRESS) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    fn send_feature_report(&self, data: &[u8]) -> HidResult<()> {
        if data.is_empty() {
            return Err(HidError::InvalidZeroSizeData);
        }

        let res = match unsafe { hidraw_ioc_set_feature(self.fd.as_raw_fd(), data) } {
            Ok(n) => n as usize,
            Err(e) => {
                return Err(HidError::HidApiError {
                    message: format!("ioctl (GFEATURE): {e}"),
                })
            }
        };

        if res != data.len() {
            return Err(HidError::IncompleteSendError {
                sent: res,
                all: data.len(),
            });
        }

        Ok(())
    }

    fn get_feature_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        let res = match unsafe { hidraw_ioc_get_feature(self.fd.as_raw_fd(), buf) } {
            Ok(n) => n as usize,
            Err(e) => {
                return Err(HidError::HidApiError {
                    message: format!("ioctl (GFEATURE): {e}"),
                })
            }
        };

        Ok(res)
    }

    fn send_output_report(&self, buf: &[u8]) -> HidResult<()> {
        let res = match unsafe { hidraw_ioc_set_output(self.fd.as_raw_fd(), buf) } {
            Ok(n) => n,
            Err(e) => {
                return Err(HidError::HidApiError {
                    message: format!("ioctl (SOUTPUT): {e}"),
                });
            }
        };

        if res as usize != buf.len() {
            return Err(HidError::IncompleteSendError {
                sent: res as usize,
                all: buf.len(),
            });
        }

        Ok(())
    }

    fn get_input_report(&self, data: &mut [u8]) -> HidResult<usize> {
        match unsafe { hidraw_ioc_get_input(self.fd.as_raw_fd(), data) } {
            Ok(n) => Ok(n as usize),
            Err(e) => Err(HidError::HidApiError {
                message: format!("ioctl (GINPUT): {e}"),
            }),
        }
    }

    fn set_blocking_mode(&self, blocking: bool) -> HidResult<()> {
        self.blocking.set(blocking);
        Ok(())
    }

    fn get_manufacturer_string(&self) -> HidResult<Option<String>> {
        let info = self.info()?;
        Ok(info.manufacturer_string().map(str::to_string))
    }

    fn get_product_string(&self) -> HidResult<Option<String>> {
        let info = self.info()?;
        Ok(info.product_string().map(str::to_string))
    }

    fn get_serial_number_string(&self) -> HidResult<Option<String>> {
        let info = self.info()?;
        Ok(info.serial_number().map(str::to_string))
    }

    fn get_device_info(&self) -> HidResult<DeviceInfo> {
        // What we have is a descriptor to a file in /dev but we need a syspath
        // so we get the major/minor from there and generate our syspath
        let devnum = fstat(self.fd.as_raw_fd())?.st_rdev;
        let syspath: PathBuf = format!("/sys/dev/char/{}:{}", major(devnum), minor(devnum)).into();

        // The clone is a bit silly but we can't implement Copy. Maybe it's not
        // much worse than doing the conversion to Rust from interacting with C.
        let device = udev::Device::from_syspath(&syspath)?;
        match device_to_hid_device_info(&device) {
            Some(info) => Ok(info[0].clone()),
            None => Err(HidError::HidApiError {
                message: "failed to create device info".into(),
            }),
        }
    }

    fn get_report_descriptor(&self, buf: &mut [u8]) -> HidResult<usize> {
        let devnum = fstat(self.fd.as_raw_fd())?.st_rdev;
        let syspath: PathBuf = format!("/sys/dev/char/{}:{}", major(devnum), minor(devnum)).into();

        let descriptor = HidrawReportDescriptor::from_syspath(&syspath)?;
        let min_size = buf.len().min(descriptor.0.len());
        buf[..min_size].copy_from_slice(&descriptor.0[..min_size]);
        Ok(min_size)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_hid_vid_pid() {
        assert_eq!(None, parse_hid_vid_pid("Hello World"));
        assert_eq!(Some((1, 1, 1)), parse_hid_vid_pid("1:1:1"));
        assert_eq!(Some((0x11, 0x17, 0x18)), parse_hid_vid_pid("11:0017:00018"));
    }

    #[test]
    fn test_hidraw_report_descriptor_1() {
        let data = include_bytes!("../tests/assets/mouse1.data");
        let desc = HidrawReportDescriptor::from_slice(&data[..]).expect("descriptor");
        let values = desc.usages().collect::<Vec<_>>();

        assert_eq!(vec![(65468, 136)], values);
    }

    #[test]
    fn test_hidraw_report_descriptor_2() {
        let data = include_bytes!("../tests/assets/mouse2.data");
        let desc = HidrawReportDescriptor::from_slice(&data[..]).expect("descriptor");
        let values = desc.usages().collect::<Vec<_>>();

        let expected = vec![(1, 2), (1, 1), (1, 128), (12, 1), (65280, 14)];
        assert_eq!(expected, values);
    }
}
