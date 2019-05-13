// **************************************************************************
// Copyright (c) 2019 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs
// **************************************************************************

//! Work around kernel bugs (detect if bug present)

use crate::error::{HidResult, ResultExt};
use lazy_static::lazy_static;
use nix::errno::Errno;
use std::ffi::CStr;
use std::mem;

lazy_static! {
    static ref KERNEL_VERSION: KernelVersion = KernelVersion::detect().unwrap();
}

/// Previous kernels would return one extra byte, when the device uses numbered reports.
/// The extra byte is at the front of the returned read buffer. Remove it, when older
/// kernel is used.
pub const KERNEL_BUG_NUMBERED_REPORT_EXTRABYTE: KernelVersion = KernelVersion::new(2, 6, 34);

#[derive(PartialEq, Eq, Debug, Default)]
pub struct KernelVersion {
    major: u8,
    minor: u8,
    release: u8,
}

impl KernelVersion {
    pub const fn new(major: u8, minor: u8, release: u8) -> Self {
        Self {
            major,
            minor,
            release,
        }
    }

    fn detect() -> HidResult<KernelVersion> {
        let mut utsname;
        let r = unsafe {
            utsname = mem::uninitialized();
            libc::uname(&mut utsname)
        };
        Errno::result(r).convert()?;

        let s = unsafe { CStr::from_ptr(utsname.release.as_mut_ptr() as *mut libc::c_char) }
            .to_str()
            .convert()?;

        // Extract version parts
        let mut kversion = KernelVersion::default();
        let mut num_iter = s.split('.');

        fn next_version_num<'a>(iter: &mut impl Iterator<Item = &'a str>) -> HidResult<u8> {
            if let Some(s) = iter.next() {
                let mut slc = s;
                if let Some((idx, _)) = s.char_indices().find(|(_, c)| !c.is_digit(10)) {
                    slc = &s[..idx];
                }
                slc.parse::<u8>().convert()
            } else {
                Ok(0)
            }
        }

        kversion.major = next_version_num(&mut num_iter)?;
        kversion.minor = next_version_num(&mut num_iter)?;
        kversion.release = next_version_num(&mut num_iter)?;
        Ok(kversion)
    }
}

impl PartialOrd for KernelVersion {
    fn partial_cmp(&self, other: &KernelVersion) -> Option<std::cmp::Ordering> {
        let kversion = (self.major as u32) << 16 | (self.minor as u32) << 8 | (self.release as u32);
        let kversion2 =
            (other.major as u32) << 16 | (other.minor as u32) << 8 | (other.release as u32);
        Some(kversion.cmp(&kversion2))
    }
}

impl Ord for KernelVersion {
    fn cmp(&self, other: &KernelVersion) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_linux_kernel_version() {
        println!("{:?}", KernelVersion::detect().unwrap());
    }
}
