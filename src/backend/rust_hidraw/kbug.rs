// **************************************************************************
// Copyright (c) 2020 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi-rs
// **************************************************************************

//! Work around kernel bugs (detect if bug present)

use super::error::{Error, Result};
use lazy_static::lazy_static;
use nix::errno::Errno;
use std::ffi::CStr;
use std::mem;
use std::str::FromStr;

lazy_static! {
    static ref KERNEL_VERSION: KernelVersion = KernelVersion::detect().unwrap();
}

pub enum KernelBug {
    /// Previous kernels would return one extra byte, when the device uses numbered reports.
    /// The extra byte is at the front of the returned read buffer. Remove it, when older
    /// kernel is used.
    NumberedReportExtrabyte,
}

impl KernelBug {
    pub fn check(self) -> bool {
        match self {
            KernelBug::NumberedReportExtrabyte => *KERNEL_VERSION < KernelVersion::new(2, 6, 34),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Default)]
struct KernelVersion {
    major: u8,
    minor: u8,
    release: u8,
}

impl KernelVersion {
    const fn new(major: u8, minor: u8, release: u8) -> Self {
        Self {
            major,
            minor,
            release,
        }
    }

    fn detect() -> Result<KernelVersion> {
        let mut utsname = mem::MaybeUninit::uninit();
        let r = unsafe { libc::uname(utsname.as_mut_ptr()) };
        Errno::result(r)?;
        let utsname = unsafe { utsname.assume_init() };

        let s =
            unsafe { CStr::from_ptr(utsname.release.as_ptr() as *const libc::c_char).to_str()? };

        s.parse()
    }
}

impl FromStr for KernelVersion {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        // Extract version parts
        let mut kversion = KernelVersion::default();
        let mut num_iter = s.split('.');

        fn next_version_num<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<u8> {
            if let Some(s) = iter.next() {
                let mut slc = s;
                if let Some((idx, _)) = s.char_indices().find(|(_, c)| !c.is_digit(10)) {
                    slc = &s[..idx];
                }
                slc.parse::<u8>().map_err(|e| e.into())
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

    #[test]
    fn test_parse() {
        assert_eq!(
            "5.5.3-arch1-1".parse::<KernelVersion>().unwrap(),
            KernelVersion::new(5, 5, 3)
        );
    }

    #[test]
    fn test_version_cmp() {
        assert!(KernelVersion::new(5, 5, 3) > KernelVersion::new(2, 6, 0));
    }
}
