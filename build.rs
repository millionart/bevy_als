fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_CFG_TARGET_OS").is_some_and(|os| os == "macos") {
        // Large test binaries exceed the compact unwind table's 16 MiB range.
        println!("cargo::rustc-link-arg=-Wl,-no_compact_unwind");
    }
}
