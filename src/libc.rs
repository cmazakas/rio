extern crate libc;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct kernel_timespec {
  pub tv_sec: i64,
  pub tv_nsec: i64,
}

extern "C" {
  pub fn rio_timespec_test(ts: kernel_timespec) -> kernel_timespec;
}
