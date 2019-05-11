//! Linux hidraw syscall interface abstraction

use crate::error::{HidResult, ResultExt};
use libc::{c_char, c_int, O_NONBLOCK, O_RDWR};
use nix::errno::Errno;
use std::fs::File;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;

// Taken from the Linux kernel source tree (include/uapi/linux/hidraw.h):
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

struct Info {
    
}

impl HidrawDevice {
    pub fn from_path(path: &Path) -> HidResult<Self> {
        let fd = unsafe {
            libc::open(
                path.as_os_str().as_bytes().as_ptr() as *const i8,
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
    fn fetch_info(&mut self, info: &mut Info) -> HidResult<()> {
        let mut rpt_desc: hidraw_report_descriptor = unsafe { mem::zeroed() };
        let mut devinfo: hidraw_devinfo = unsafe { mem::zeroed() };
        let mut rpt_desc_size: c_int = 0;
        let mut buf = [0u8; 256];
        let but_char_view = unsafe { mem::transmute(&mut buf[..]) };

        let fd = self.file.as_raw_fd();

        unsafe {
            hidraw_ioc_getrdescsize(fd, &mut rpt_desc_size).convert()?;
            rpt_desc.size = rpt_desc_size as u32;
            hidraw_ioc_getrdesc(fd, &mut rpt_desc).convert()?;
            hidraw_ioc_getrawname(fd, but_char_view).convert()?;
        };
        Ok(())
    }
}
