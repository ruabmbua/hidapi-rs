pub fn compile() {
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
