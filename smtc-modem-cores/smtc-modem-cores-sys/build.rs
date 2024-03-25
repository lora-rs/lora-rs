fn main() {
    use cmake::Config;
    use std::env;
    use std::path::PathBuf;

    let dst = Config::new("./")
        .define("BUILD_TESTING", "OFF")
        .define("CMAKE_C_COMPILER_WORKS", "1")
        .define("CMAKE_CXX_COMPILER_WORKS", "1")
        .pic(false)
        .build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=smtc-modem-cores");

    let bindings = bindgen::Builder::default()
        .raw_line("use cty;")
        .use_core()
        .ctypes_prefix("cty")
        .detect_include_paths(true)
        .header("SWL2001/lbm_lib/smtc_modem_core/radio_drivers/sx126x_driver/src/sx126x.h")
        .clang_arg(format!("-I{}/include", dst.display()))
        .trust_clang_mangling(false)
        .allowlist_type("sx126x_status_t")
        .allowlist_function("sx126x_set_sleep")
        .generate()
        .expect("Failed to generate sx12xx bindings!");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("smtc-modem-cores.rs")).expect("Couldn't write bindings!");
}
