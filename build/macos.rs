pub fn compile() {
    cc::Build::new()
        .file("etc/hidapi/mac/hid.c")
        .include("etc/hidapi/hidapi")
        .compile("libhidapi.a");
    println!("cargo:rustc-cfg=hidapi");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=AppKit")
}
