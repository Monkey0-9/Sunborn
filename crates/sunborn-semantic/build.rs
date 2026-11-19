fn main() {
    let mut build = cc::Build::new();
    build
        .file("c/sunborn_simd_accelerator.c")
        .include("c")
        .opt_level(3);

    // AVX2 is deliberately NOT enabled by default.
    //
    // Passing `-mavx2` to the C compiler defines `__AVX2__` for the whole
    // translation unit, so the resulting binary requires AVX2 on every host that
    // runs it. On any x86_64 machine without AVX2 that is an illegal-instruction
    // fault (SIGILL) at runtime, not a compile-time error. The portable scalar
    // path in `sunborn_simd_accelerator.c` is correct for every CPU.
    //
    // Proper native acceleration requires runtime CPU dispatch
    // (`__builtin_cpu_supports` on GCC/Clang, different machinery on MSVC), which
    // this build does not implement yet. Until that exists, AVX2 is opt-in for
    // benchmark builds only: set SUNBORN_NATIVE_SIMD=1.
    let native_simd = std::env::var("SUNBORN_NATIVE_SIMD")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("x86_64") && native_simd {
        if target.contains("msvc") {
            build.flag_if_supported("/arch:AVX2");
        } else {
            build.flag_if_supported("-mavx2");
            build.flag_if_supported("-mfma");
            build.flag_if_supported("-msse4.2");
            build.flag_if_supported("-O3");
        }
    } else if target.contains("aarch64") {
        // NEON is architecturally guaranteed on aarch64, so the NEON path is
        // safe to enable unconditionally there.
        build.flag_if_supported("-O3");
    }

    build.compile("sunborn_simd_accelerator");

    println!("cargo:rerun-if-changed=c/sunborn_simd_accelerator.c");
    println!("cargo:rerun-if-changed=c/sunborn_simd_accelerator.h");
}
