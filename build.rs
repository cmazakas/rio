extern crate cc;

fn main() {
  let msg = "RIO_LIBURING_INCLUDE_DIR was not defined by the environment. It must be set to the include dir of the liburing installation.";
  let include_dir = std::env::var("RIO_LIBURING_INCLUDE_DIR").expect(msg);

  let msg = "RIO_LIBURING_LIBRARY_DIR was not defined by the environment. It must be set to the library dir of the liburing installation.";
  let library_dir = std::env::var("RIO_LIBURING_LIBRARY_DIR").expect(msg);

  println!("cargo:rustc-link-search={library_dir}");
  println!("cargo:rustc-link-lib=static=uring-ffi");

  println!("cargo:rerun-if-changed=src/liburing/lib.c");
  cc::Build::new()
    .file("src/liburing/lib.c")
    .include(include_dir)
    .compile("rio-iouring");

  let out_dir = std::env::var("OUT_DIR").unwrap();
  println!("cargo:rustc-link-search={out_dir}");
  println!("cargo:rustc-link-lib=static=rio-iouring");
}
