extern crate cc;

fn main() {
    let liburing_include = "/home/exbigboss/cpp/installed/include";
    let liburing = "/home/exbigboss/cpp/installed/lib";
    let out_dir = std::env::var("OUT_DIR").unwrap();
    println!("cargo:rustc-link-search={out_dir}");
    println!("cargo:rustc-link-search={liburing}");
    println!("cargo:rustc-link-lib=static=uring");

    println!("cargo:rerun-if-changed=src/liburing/lib.c");
    cc::Build::new()
        .file("src/liburing/lib.c")
        .include(liburing_include)
        .compile("rio-iouring");

    println!("cargo:rustc-link-lib=static=rio-iouring");
}
