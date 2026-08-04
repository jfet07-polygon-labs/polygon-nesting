fn main() {
    napi_build::setup();

    println!("cargo:rerun-if-env-changed=TARGET");
    let target = std::env::var("TARGET").expect("TARGET must be set by cargo during build");
    println!("cargo:rustc-env=IRREGULAR_NATIVE_TARGET={target}");
}
