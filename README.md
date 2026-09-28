# hidapi [![Version](https://img.shields.io/crates/v/hidapi.svg)](https://crates.io/crates/hidapi) [![License: MIT AND BSD-3-Clause](https://img.shields.io/badge/License-MIT%20AND%20BSD--3--Clause-yellow.svg)](#license) [![Documentation](https://docs.rs/hidapi/badge.svg)](https://docs.rs/hidapi) [![Chat](https://img.shields.io/badge/discord-devroom-blue.svg)](https://discordapp.com/invite/3ahhJGN)

This crate provides a rust abstraction over the features of the C library
[hidapi](https://github.com/libusb/hidapi). Based off of
[hidapi-rs](https://github.com/Osspial/hidapi-rs) by Osspial.

# Usage

This crate is on [crates.io](https://crates.io/crates/hidapi) and can be
used by adding `hidapi` to the dependencies in your project's `Cargo.toml`.

In case you are downloading / checking out this repository directly via git,
make sure to initialize the git submodules after cloning!

```sh
git submodule update --init
```

# Example

```rust
extern crate hidapi;

let api = hidapi::HidApi::new().unwrap();
// Print out information about all connected devices
for device in api.device_list() {
    println!("{:#?}", device);
}

// Connect to device using its VID and PID
let (VID, PID) = (0x0123, 0x3456);
let device = api.open(VID, PID).unwrap();

// Read data from device
let mut buf = [0u8; 8];
let res = device.read(&mut buf[..]).unwrap();
println!("Read: {:?}", &buf[..res]);

// Write data to device
let buf = [0u8, 1, 2, 3, 4];
let res = device.write(&buf).unwrap();
println!("Wrote: {:?} byte(s)", res);
```

# Documentation
Available at [docs.rs](https://docs.rs/hidapi).

# License

The Rust code in this crate is licensed under the MIT license, see [LICENSE.txt](LICENSE.txt).

The bundled [hidapi](https://github.com/libusb/hidapi) C library in `etc/hidapi` is used under its BSD-3-Clause license option, see `etc/hidapi/LICENSE*.txt`.
