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

extern crate gcc;
extern crate pkg_config;

use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();

    if target.contains("windows") {
        gcc::Config::new()
            .file("etc/hidapi/windows/hid.c")
            .include("etc/hidapi/hidapi")
            .compile("libhidapi.a");
        println!("cargo:rustc-link-lib=setupapi");

    } else if target.contains("macos") {
        gcc::Config::new()
            .file("etc/hidapi/mac/hid.c")
            .include("etc/hidapi/hidapi")
            .compile("libhidapi.a");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");

    } else if target.contains("linux") {
        let mut config = gcc::Config::new();
        config.file("etc/hidapi/libusb/hid.c").include("etc/hidapi/hidapi");
        let lib = pkg_config::find_library("libusb-1.0").expect("Unable to find libusb-1.0");
        for path in lib.include_paths {
            config.include(path.to_str().expect("Failed to convert include path to str"));
        }
        config.compile("libhidapi.a");
    }
}
