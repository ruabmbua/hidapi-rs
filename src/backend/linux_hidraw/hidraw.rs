//! Linux hidraw syscall interface abstraction

use crate::error::{HidResult, ResultExt};
use libc::{c_char, c_int, O_NONBLOCK, O_RDWR};
use nix::errno::Errno;
use std::ffi::{OsStr, OsString, CStr, CString};
use std::fs::File;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;

// Taken from the Linux kernel source tree (include/uapi/linux/hidraw.h, input.h, hid.h):
//
// #define BUS_USB			0x03
// #define BUS_HIL			0x04
// #define BUS_BLUETOOTH	0x05
// #define BUS_VIRTUAL		0x06
//
// #define HID_MAX_DESCRIPTOR_SIZE 4096
//
// struct hidraw_report_descriptor {
// 	__u32 size;
// 	__u8 value[HID_MAX_DESCRIPTOR_SIZE];
// };

// struct hidraw_devinfo {
// 	__u32 bustype;
// 	__s16 vendor;
// 	__s16 product;
// };

// /* ioctl interface */
// #define HIDIOCGRDESCSIZE	_IOR('H', 0x01, int)
// #define HIDIOCGRDESC		_IOR('H', 0x02, struct hidraw_report_descriptor)
// #define HIDIOCGRAWINFO		_IOR('H', 0x03, struct hidraw_devinfo)
// #define HIDIOCGRAWNAME(len)     _IOC(_IOC_READ, 'H', 0x04, len)
// #define HIDIOCGRAWPHYS(len)     _IOC(_IOC_READ, 'H', 0x05, len)
// /* The first byte of SFEATURE and GFEATURE is the report number */
// #define HIDIOCSFEATURE(len)    _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
// #define HIDIOCGFEATURE(len)    _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)

// #define HIDRAW_FIRST_MINOR 0
// #define HIDRAW_MAX_DEVICES 64
// /* number of reports to buffer */
// #define HIDRAW_BUFFER_SIZE 64

const BUS_USB: u8 = 0x03;
const BUS_HIL: u8 = 0x04;
const BUS_BLUETOOTH: u8 = 0x05;
const BUS_VIRTUAL: u8 = 0x06;

const HID_MAX_DESCRIPTOR_SIZE: usize = 4096;
const HIDRAW_IOC_MAGIC: u8 = b'H';

#[repr(C)]
pub struct hidraw_report_descriptor {
    size: u32,
    value: [u8; HID_MAX_DESCRIPTOR_SIZE],
}

#[repr(C)]
pub struct hidraw_devinfo {
    bustype: u32,
    vendor: i16,
    product: i16,
}

const HIDRAW_IOC_HIDIOCGRDESCSIZE: u8 = 0x01;
ioctl_read!(
    hidraw_ioc_getrdescsize,
    HIDRAW_IOC_MAGIC,
    HIDRAW_IOC_HIDIOCGRDESCSIZE,
    c_int
);

const HIDRAW_IOC_HIDIOCGRDESC: u8 = 0x02;
ioctl_read!(
    hidraw_ioc_getrdesc,
    HIDRAW_IOC_MAGIC,
    HIDRAW_IOC_HIDIOCGRDESC,
    hidraw_report_descriptor
);

const HIDRAW_IOC_GETRAWINFO: u8 = 0x03;
ioctl_read!(
    hidraw_ioc_getrawinfo,
    HIDRAW_IOC_MAGIC,
    HIDRAW_IOC_GETRAWINFO,
    hidraw_devinfo
);

const HIDRAW_IOC_GETRAWNAME: u8 = 0x04;
ioctl_read_buf!(
    hidraw_ioc_getrawname,
    HIDRAW_IOC_MAGIC,
    HIDRAW_IOC_GETRAWNAME,
    c_char
);

const HIDRAW_IOC_GETRAWPHYS: u8 = 0x05;
ioctl_read_buf!(
    hidraw_ioc_getrawphys,
    HIDRAW_IOC_MAGIC,
    HIDRAW_IOC_GETRAWPHYS,
    c_char
);

const HIDRAW_IOC_SETFEATURE: u8 = 0x06;
ioctl_readwrite_buf!(
    hidraw_ioc_setfeature,
    HIDRAW_IOC_MAGIC,
    HIDRAW_IOC_SETFEATURE,
    c_char
);

const HIDRAW_IOC_GETFEATURE: u8 = 0x07;
ioctl_readwrite_buf!(
    hidraw_ioc_getfeature,
    HIDRAW_IOC_MAGIC,
    HIDRAW_IOC_GETFEATURE,
    c_char
);

pub struct HidrawDevice {
    file: File,
}

#[derive(Default, Debug)]
struct Info {
    raw_descriptor: Vec<u8>,
    vendor_id: u16,
    product_id: u16,
    bus_type: u32,
    raw_name: OsString,
    raw_phys: OsString,
}

impl HidrawDevice {
    pub fn from_path<P: AsRef<Path>>(path: P) -> HidResult<Self> {
        let path = path.as_ref();
        let raw_path = CString::new(path.as_os_str().as_bytes()).convert()?;
        let fd = unsafe {
            libc::open(
                raw_path.as_ptr(),
                O_RDWR | O_NONBLOCK,
            )
        };
        // Check errno
        let fd = Errno::result(fd).convert()?;
        let file = unsafe { File::from_raw_fd(fd) };

        Ok(Self { file })
    }

    /// Fetches all the available info, which can be interpreted
    /// independently.
    fn fetch_info(&self) -> HidResult<Info> {
        let mut info = Info::default();

        let mut rpt_desc: hidraw_report_descriptor = unsafe { mem::zeroed() };
        let mut devinfo: hidraw_devinfo = unsafe { mem::zeroed() };
        let mut rpt_desc_size: c_int = 0;
        let mut buf = [0u8; 256];
        let buf_char_view = unsafe { mem::transmute::<_, &mut [i8]>(&mut buf[..]) };

        let fd = self.file.as_raw_fd();

        unsafe {
            // Get raw descriptor
            hidraw_ioc_getrdescsize(fd, &mut rpt_desc_size).convert()?;
            rpt_desc.size = rpt_desc_size as u32;
            hidraw_ioc_getrdesc(fd, &mut rpt_desc).convert()?;
            info.raw_descriptor = Vec::from(&rpt_desc.value[..rpt_desc.size as usize]);

            // Get devinfo
            hidraw_ioc_getrawinfo(fd, &mut devinfo).convert()?;
            info.vendor_id = devinfo.vendor as u16;
            info.product_id = devinfo.product as u16;
            info.bus_type = devinfo.bustype;

            // Get raw name
            hidraw_ioc_getrawname(fd, buf_char_view).convert()?;
            let cstr = CStr::from_ptr(buf_char_view.as_ptr());
            info.raw_name = OsStr::from_bytes(cstr.to_bytes()).to_os_string();

            // Get raw PHY
            hidraw_ioc_getrawphys(fd, buf_char_view).convert()?;
            let cstr = CStr::from_ptr(buf_char_view.as_ptr());
            info.raw_phys = OsStr::from_bytes(cstr.to_bytes()).to_os_string();
        };
        Ok(info)
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_fetch_info() {
        let dev = HidrawDevice::from_path("/dev/hidraw1").unwrap();
        println!("{:?}", dev.fetch_info().unwrap());
    }
}