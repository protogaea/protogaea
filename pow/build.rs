// Builds the reference yespower implementation (vendored in `yespower/`) when the `c` feature is
// on; the crate's own hash is the Rust port and needs no C compiler.
//
// The build is portable by default: no CPU-specific flags, so one binary runs on every x86-64 or
// ARM64 machine, as spec §16 requires. PROTOGAEA_POW_NATIVE=1 adds -march=native, for measuring
// what a tuned build gains.

#[cfg(not(feature = "c"))]
fn main() {}

#[cfg(feature = "c")]
fn main() {
    println!("cargo:rerun-if-changed=yespower");
    println!("cargo:rerun-if-env-changed=PROTOGAEA_POW_NATIVE");
    let mut build = cc::Build::new();
    build
        .file("yespower/yespower-opt.c")
        .file("yespower/sha256.c")
        .include("yespower")
        .opt_level(2)
        .flag_if_supported("-fomit-frame-pointer")
        .warnings(false);
    if std::env::var("PROTOGAEA_POW_NATIVE").as_deref() == Ok("1") {
        build.flag_if_supported("-march=native");
    }
    build.compile("yespower");
}
