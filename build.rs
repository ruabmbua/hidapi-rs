// **************************************************************************
// Copyright (c) 2015 Roland Ruckerbauer All Rights Reserved.
//
// This file is part of hidapi_rust.
//
// hidapi_rust is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// hidapi_rust is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with hidapi_rust.  If not, see <http://www.gnu.org/licenses/>.
// *************************************************************************

extern crate cc;
extern crate pkg_config;

use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();

    if target.contains("linux") {
        compile_linux();
    } else if target.contains("windows") {
        compile_windows();
    } else if target.contains("darwin") {
        compile_macos();
    } else if target.contains("freebsd") {
        compile_freebsd();
    } else if target.contains("openbsd") {
        compile_openbsd();
    } else if target.contains("illumos") {
        compile_illumos();
    } else {
        panic!("Unsupported target os {target} for hidapi-rs");
    }
}

fn compile_linux() {
    // First check the features enabled for the crate.
    // Only one linux backend should be enabled at a time.

    let avail_backends: [(&'static str, Box<dyn Fn()>); 5] = [
        (
            "LINUX_STATIC_HIDRAW",
            Box::new(|| {
                let mut config = cc::Build::new();
                println!("cargo:rerun-if-changed=etc/hidapi/linux/hid.c");
                config
                    .file("etc/hidapi/linux/hid.c")
                    .include("etc/hidapi/hidapi");
                pkg_config::probe_library("libudev").expect("Unable to find libudev");
                config.compile("libhidapi.a");
                println!("cargo:rustc-cfg=hidapi");
            }),
        ),
        (
            "LINUX_STATIC_LIBUSB",
            Box::new(|| {
                let mut config = cc::Build::new();
                println!("cargo:rerun-if-changed=etc/hidapi/linux/hid.c");
                config
                    .file("etc/hidapi/libusb/hid.c")
                    .include("etc/hidapi/hidapi");
                let lib =
                    pkg_config::find_library("libusb-1.0").expect("Unable to find libusb-1.0");
                for path in lib.include_paths {
                    config.include(
                        path.to_str()
                            .expect("Failed to convert include path to str"),
                    );
                }
                config.compile("libhidapi.a");
                println!("cargo:rustc-cfg=libusb");
                println!("cargo:rustc-cfg=hidapi");
            }),
        ),
        (
            "LINUX_SHARED_HIDRAW",
            Box::new(|| {
                pkg_config::probe_library("hidapi-hidraw").expect("Unable to find hidapi-hidraw");
                println!("cargo:rustc-cfg=hidapi");
            }),
        ),
        (
            "LINUX_SHARED_LIBUSB",
            Box::new(|| {
                pkg_config::probe_library("libusb-1.0").expect("Unable to find libusb-1.0");
                pkg_config::probe_library("hidapi-libusb").expect("Unable to find hidapi-libusb");
                println!("cargo:rustc-cfg=libusb");
                println!("cargo:rustc-cfg=hidapi");
            }),
        ),
        (
            "LINUX_NATIVE",
            Box::new(|| {
                // The udev crate takes care of finding its library
            }),
        ),
    ];

    let mut backends = avail_backends
        .iter()
        .filter(|f| env::var(format!("CARGO_FEATURE_{}", f.0)).is_ok());

    if backends.clone().count() != 1 {
        panic!("Exactly one linux hidapi backend must be selected.");
    }

    // Build it!
    (backends.next().unwrap().1)();
}

//#[cfg(all(feature = "shared-libusb", not(feature = "shared-hidraw")))]
//fn compile_linux() {
//
//}
//
//#[cfg(all(feature = "shared-hidraw"))]
//fn compile_linux() {
//
//}

fn compile_freebsd() {
    pkg_config::probe_library("hidapi").expect("Unable to find hidapi");
    println!("cargo:rustc-cfg=libusb");
    println!("cargo:rustc-cfg=hidapi");
}

fn compile_openbsd() {
    pkg_config::probe_library("hidapi-libusb").expect("Unable to find hidapi");
    println!("cargo:rustc-cfg=libusb");
    println!("cargo:rustc-cfg=hidapi");
}

fn compile_illumos() {
    // First check the features enabled for the crate.
    // Only one illumos backend should be enabled at a time.

    let avail_backends: [(&'static str, Box<dyn Fn()>); 2] = [
        (
            "ILLUMOS_STATIC_LIBUSB",
            Box::new(|| {
                let mut config = cc::Build::new();
                config
                    .file("etc/hidapi/libusb/hid.c")
                    .include("etc/hidapi/hidapi");
                let lib =
                    pkg_config::find_library("libusb-1.0").expect("Unable to find libusb-1.0");
                for path in lib.include_paths {
                    config.include(
                        path.to_str()
                            .expect("Failed to convert include path to str"),
                    );
                }
                config.compile("libhidapi.a");
            }),
        ),
        (
            "ILLUMOS_SHARED_LIBUSB",
            Box::new(|| {
                pkg_config::probe_library("hidapi-libusb").expect("Unable to find hidapi-libusb");
            }),
        ),
    ];

    let mut backends = avail_backends
        .iter()
        .filter(|f| env::var(format!("CARGO_FEATURE_{}", f.0)).is_ok());

    if backends.clone().count() != 1 {
        panic!("Exactly one illumos hidapi backend must be selected.");
    }

    // Build it!
    (backends.next().unwrap().1)();

    println!("cargo:rustc-cfg=libusb");
    println!("cargo:rustc-cfg=hidapi");
}

fn compile_windows() {
    #[cfg(not(feature = "windows-native"))]
    {
        let linkage = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();

        let mut cc = cc::Build::new();
        cc.file("etc/hidapi/windows/hid.c")
            .include("etc/hidapi/hidapi");

        if linkage.contains("crt-static") {
            // https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes
            cc.static_crt(true);
        }
        cc.compile("libhidapi.a");
        println!("cargo:rustc-link-lib=setupapi");

        println!("cargo:rustc-cfg=hidapi");
    }
}

fn compile_macos() {
    let avail_backends: [(&'static str, Box<dyn Fn()>); 2] = [
        (
            "MACOS_HIDAPI",
            Box::new(|| {
                cc::Build::new()
                    .file("etc/hidapi/mac/hid.c")
                    .include("etc/hidapi/hidapi")
                    .compile("libhidapi.a");
                println!("cargo:rustc-cfg=hidapi");
                println!("cargo:rustc-link-lib=framework=IOKit");
                println!("cargo:rustc-link-lib=framework=CoreFoundation");
                println!("cargo:rustc-link-lib=framework=AppKit")
            }),
        ),
        (
            "MACOS_NATIVE",
            Box::new(|| {
                // The udev crate takes care of finding its library
                println!("cargo:rustc-link-lib=framework=IOKit");
                println!("cargo:rustc-link-lib=framework=CoreFoundation");
            }),
        ),
    ];

    let mut backends = avail_backends
        .iter()
        .filter(|f| env::var(format!("CARGO_FEATURE_{}", f.0)).is_ok());

    if backends.clone().count() != 1 {
        panic!("Exactly one macos hidapi backend must be selected.");
    }

    // Build it!
    (backends.next().unwrap().1)();
}
