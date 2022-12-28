#![allow(clippy::missing_safety_doc)]

use crate::{self as fiona, libc};

#[repr(C)]
pub struct io_uring {
  _data: [u8; 0],
  _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[repr(C)]
pub struct io_uring_sqe {
  _data: [u8; 0],
  _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[repr(C)]
pub struct io_uring_cqe {
  pub user_data: u64,
  pub res: i32,
  pub flags: u32,

  big_cqe: [u64; 0],
}

extern "C" {
  fn rio_setup(entries: u32, flags: u32) -> *mut io_uring;
  fn rio_teardown(ring: *mut io_uring);
  fn rio_make_sqe(ring: *mut io_uring) -> *mut io_uring_sqe;

  fn rio_timerfd_create() -> i32;
  fn rio_timerfd_settime(fd: i32, secs: u64, nanos: u64) -> i32;

  fn rio_make_pipe(pipefd: *mut i32) -> i32;
  fn rio_make_ipv4_tcp_server_socket(
    ipv4_addr: u32,
    port: u16,
    fdp: *mut i32,
  ) -> i32;
  fn rio_make_ipv4_tcp_socket(fdp: *mut i32) -> i32;

  pub fn io_uring_submit(ring: *mut io_uring) -> i32;
  pub fn io_uring_wait_cqe(
    ring: *mut io_uring,
    cqe_ptr: *mut *mut io_uring_cqe,
  ) -> i32;
  pub fn io_uring_peek_cqe(
    ring: *mut io_uring,
    cqe_ptr: *mut *mut io_uring_cqe,
  ) -> i32;
  pub fn io_uring_cqe_seen(ring: *mut io_uring, cqe: *mut io_uring_cqe);
  pub fn io_uring_sqe_set_data(sqe: *mut io_uring_sqe, data: *mut libc::c_void);
  pub fn io_uring_cqe_get_data(cqe: *const io_uring_cqe) -> *mut libc::c_void;
  pub fn io_uring_sqe_set_flags(sqe: *mut io_uring_sqe, flags: u32);

  pub fn io_uring_prep_accept(
    sqe: *mut io_uring_sqe,
    fd: i32,
    addr: *mut fiona::ip::tcp::sockaddr_in,
    addrlen: *mut u32,
    flags: i32,
  );

  pub fn io_uring_prep_connect(
    sqe: *mut io_uring_sqe,
    sockfd: i32,
    addr: *const fiona::libc::sockaddr,
    addrlen: u32,
  );

  pub fn io_uring_prep_read(
    sqe: *mut io_uring_sqe,
    fd: i32,
    buf: *mut libc::c_void,
    nbytes: u32,
    offset: u64,
  );

  pub fn io_uring_prep_write(
    sqe: *mut io_uring_sqe,
    fd: i32,
    buf: *const fiona::libc::c_void,
    nbytes: u32,
    offset: u64,
  );

  pub fn io_uring_prep_cancel(
    sqe: *mut io_uring_sqe,
    user_data: *mut libc::c_void,
    flags: i32,
  );

  pub fn io_uring_prep_timeout(
    sqe: *mut io_uring_sqe,
    ts: *mut fiona::libc::kernel_timespec,
    count: u32,
    flags: u32,
  );

  pub fn io_uring_prep_nop(sqe: *mut io_uring_sqe);
}

#[must_use]
pub fn setup(entries: u32, flags: u32) -> *mut io_uring {
  unsafe { rio_setup(entries, flags) }
}

pub unsafe fn teardown(ring: *mut io_uring) {
  rio_teardown(ring);
}

pub unsafe fn make_sqe(ring: *mut io_uring) -> *mut io_uring_sqe {
  rio_make_sqe(ring)
}

#[must_use]
pub fn timerfd_create() -> i32 {
  unsafe { rio_timerfd_create() }
}

pub unsafe fn timerfd_settime(
  fd: i32,
  secs: u64,
  nanos: u64,
) -> Result<(), libc::Errno> {
  match rio_timerfd_settime(fd, secs, nanos) {
    0 => Ok(()),
    e => Err(fiona::libc::errno(e)),
  }
}

#[must_use]
pub fn make_pipe(pipefd: &mut [i32; 2]) -> i32 {
  unsafe { rio_make_pipe(pipefd.as_mut_ptr()) }
}

pub fn make_ipv4_tcp_server_socket(
  ipv4_addr: u32,
  port: u16,
) -> Result<i32, libc::Errno> {
  let mut fd = -1;
  let err =
    unsafe { rio_make_ipv4_tcp_server_socket(ipv4_addr, port, &mut fd) };

  match err {
    0 => Ok(fd),
    e => Err(libc::errno(e)),
  }
}

pub fn make_ipv4_tcp_socket() -> Result<i32, libc::Errno> {
  let mut fd = -1;
  let err = unsafe { rio_make_ipv4_tcp_socket(&mut fd) };
  match err {
    0 => Ok(fd),
    e => Err(libc::errno(e)),
  }
}
