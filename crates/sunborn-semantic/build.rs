fn main() {
    let mut build = cc::Build::new();
    build
        .file("c/sunborn_simd_accelerator.c")
        .include("c")
        .opt_level(3);

    // If compiling under MinGW GNU on x86_64, enable SSE4.2 / AVX vector extensions safely
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("x86_64") && (target.contains("gnu") || target.contains("linux")) {
        build.flag_if_supported("-msse4.2");
        build.flag_if_supported("-O3");
    }

    build.compile("sunborn_simd_accelerator");

    println!("cargo:rerun-if-changed=c/sunborn_simd_accelerator.c");
    println!("cargo:rerun-if-changed=c/sunborn_simd_accelerator.h");
}
