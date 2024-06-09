pub fn compile() {
    // First check the features enabled for the crate.
    // Only one linux backend should be enabled at a time.

    let avail_backends: &[(&str, &dyn Fn())] = &[
        ("LINUX_STATIC_HIDRAW", &|| {
            let mut config = cc::Build::new();
            println!("cargo:rerun-if-changed=etc/hidapi/linux/hid.c");
            config
                .file("etc/hidapi/linux/hid.c")
                .include("etc/hidapi/hidapi");
            pkg_config::probe_library("libudev").expect("Unable to find libudev");
            config.compile("libhidapi.a");

            println!("cargo:rustc-cfg=hidapi");
        }),
        ("LINUX_SHARED_HIDRAW", &|| {
            pkg_config::probe_library("hidapi-hidraw").expect("Unable to find hidapi-hidraw");

            println!("cargo:rustc-cfg=hidapi");
        }),
        ("LINUX_STATIC_LIBUSB", &|| {
            let mut config = cc::Build::new();
            println!("cargo:rerun-if-changed=etc/hidapi/linux/hid.c");
            config
                .file("etc/hidapi/libusb/hid.c")
                .include("etc/hidapi/hidapi");
            let lib = pkg_config::find_library("libusb-1.0").expect("Unable to find libusb-1.0");
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
        ("LINUX_SHARED_LIBUSB", &|| {
            pkg_config::probe_library("libusb-1.0").expect("Unable to find libusb-1.0");
            pkg_config::probe_library("hidapi-libusb").expect("Unable to find hidapi-libusb");

            println!("cargo:rustc-cfg=libusb");
            println!("cargo:rustc-cfg=hidapi");
        }),
        ("LINUX_NATIVE", &|| ()),
    ];

    let mut backends = avail_backends
        .iter()
        .filter(|(name, _)| std::env::var(format!("CARGO_FEATURE_{}", name)).is_ok());

    // Build it!
    match (backends.next(), backends.next()) {
        (Some((_, func)), None) => func(),
        _ => panic!("Exactly one linux hidapi backend must be selected."),
    }
}
