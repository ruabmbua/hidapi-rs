pub fn compile() {
    // First check the features enabled for the crate.
    // Only one illumos backend should be enabled at a time.

    let avail_backends: [(&str, &dyn Fn()); 2] = [
        ("ILLUMOS_STATIC_LIBUSB", &|| {
            let mut config = cc::Build::new();
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
        }),
        ("ILLUMOS_SHARED_LIBUSB", &|| {
            pkg_config::probe_library("hidapi-libusb").expect("Unable to find hidapi-libusb");
        }),
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
