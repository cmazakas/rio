extern crate libc;

#[repr(C)]
pub struct sockaddr {
  _data: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct kernel_timespec {
  pub tv_sec: i64,
  pub tv_nsec: i64,
}

extern "C" {
  pub fn close(fd: i32) -> i32;
  pub fn errno_to_int(e: i32) -> i32;
  pub fn write(fd: i32, buf: *const libc::c_void, count: usize);
  pub fn rio_sockaddr_in_test(addr: libc::sockaddr_in) -> libc::sockaddr_in;
  pub fn rio_make_sockaddr_in(ipv4_addr: u32, port: u16) -> libc::sockaddr_in;

  pub fn rio_timespec_test(ts: kernel_timespec) -> kernel_timespec;
}
