fn main() {
    // rustc-link-arg (not -bins): integration tests are not bins, and they
    // are the only artifacts this crate links.
    println!("cargo::rustc-link-arg=--nmagic");
    println!("cargo::rustc-link-arg=-Tlink.x");
    println!("cargo::rustc-link-arg=-Tdefmt.x");
    println!("cargo::rustc-link-arg=-Tembedded-test.x");
    println!("cargo::rustc-check-cfg=cfg(rust_analyzer)");
}
