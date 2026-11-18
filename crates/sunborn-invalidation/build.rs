fn main() {
    let mut build = cc::Build::new();
    build
        .file("c/sunborn_graph_accelerator.c")
        .include("c")
        .opt_level(3);

    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("x86_64") && (target.contains("gnu") || target.contains("linux")) {
        build.flag_if_supported("-msse4.2");
        build.flag_if_supported("-mpopcnt");
        build.flag_if_supported("-O3");
    }

    build.compile("sunborn_graph_accelerator");

    println!("cargo:rerun-if-changed=c/sunborn_graph_accelerator.c");
    println!("cargo:rerun-if-changed=c/sunborn_graph_accelerator.h");
}
