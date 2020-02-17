// Enable hidapi backend, when one of the linux hidapi
// based linux backends, or a non linux platform.
#[cfg(any(
    feature = "linux-static-libusb",
    feature = "linux-static-hidraw",
    feature = "linux-shared-libusb",
    feature = "linux-shared-hidraw",
    not(target_os = "linux"),
))]
pub(crate) mod hidapi;

// Enable the pure rust_hidraw backend only when the feature is enabled,
// and the target_os is linux.
#[cfg(all(feature = "linux-rust-hidraw", target_os = "linux"))]
pub(crate) mod rust_hidraw;
