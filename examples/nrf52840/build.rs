//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Put linker configuration in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    if cfg!(feature = "link-to-ram") {
        File::create(out.join("link_ram_cortex_m.x"))
            .unwrap()
            .write_all(include_bytes!("../link_ram_cortex_m.x"))
            .unwrap();
        println!("cargo:rustc-link-search={}", out.display());

        println!("cargo:rustc-link-arg-bins=-Tlink_ram.x");
        println!("cargo:rerun-if-changed=link_ram.x");
    } else {
        File::create(out.join("memory.x"))
            .unwrap()
            .write_all(include_bytes!("memory.x"))
            .unwrap();
        println!("cargo:rustc-link-search={}", out.display());

        println!("cargo:rustc-link-arg-bins=-Tlink.x");
        println!("cargo:rerun-if-changed=link.x");
    }

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
