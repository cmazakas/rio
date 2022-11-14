#![allow(clippy::missing_safety_doc)]

use crate::libc;

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
  _data: [u8; 0],
  _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

extern "C" {
  fn rio_setup(entries: u32, flags: u32) -> *mut io_uring;
  fn rio_teardown(ring: *mut io_uring);
  fn rio_make_sqe(ring: *mut io_uring) -> *mut io_uring_sqe;
  fn rio_io_uring_sqe_set_data(sqe: *mut io_uring_sqe, data: *mut libc::c_void);
  fn rio_io_uring_submit(ring: *mut io_uring) -> i32;
  fn rio_io_uring_prep_accept(sqe: *mut io_uring_sqe, fd: i32);
  fn rio_io_uring_prep_read(
    sqe: *mut io_uring_sqe,
    fd: i32,
    buf: *mut libc::c_void,
    nbytes: u32,
    offset: i64,
  );
  fn rio_io_uring_prep_cancel(sqe: *mut io_uring_sqe, user_data: *mut libc::c_void, flags: i32);
  fn rio_io_uring_prep_nop(sqe: *mut io_uring_sqe);
  fn rio_io_uring_wait_cqe(ring: *mut io_uring, res: &mut i32) -> *mut io_uring_cqe;
  fn rio_io_uring_cqe_seen(ring: *mut io_uring, cqe: *mut io_uring_cqe);

  fn rio_timerfd_create() -> i32;
  fn rio_timerfd_settime(fd: i32, secs: u64, nanos: u64) -> i32;
  fn rio_io_uring_cqe_get_data(cqe: *const io_uring_cqe) -> *mut libc::c_void;

  fn rio_make_pipe(pipefd: *mut i32) -> i32;
  fn rio_make_ipv4_tcp_server_socket(ipv4_addr: u32, port: u16, fdp: *mut i32) -> i32;
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

pub unsafe fn io_uring_sqe_set_data(sqe: *mut io_uring_sqe, data: *mut libc::c_void) {
  rio_io_uring_sqe_set_data(sqe, data);
}

pub unsafe fn io_uring_prep_accept(sqe: *mut io_uring_sqe, fd: i32) {
  rio_io_uring_prep_accept(sqe, fd);
}

pub unsafe fn io_uring_prep_read(
  sqe: *mut io_uring_sqe,
  fd: i32,
  buf: *mut libc::c_void,
  nbytes: u32,
  offset: i64,
) {
  rio_io_uring_prep_read(sqe, fd, buf, nbytes, offset);
}

pub unsafe fn io_uring_prep_cancel(
  sqe: *mut io_uring_sqe,
  user_data: *mut libc::c_void,
  flags: i32,
) {
  rio_io_uring_prep_cancel(sqe, user_data, flags);
}

pub unsafe fn io_uring_prep_nop(sqe: *mut io_uring_sqe) {
  rio_io_uring_prep_nop(sqe);
}

pub unsafe fn io_uring_submit(ring: *mut io_uring) -> i32 {
  rio_io_uring_submit(ring)
}

pub unsafe fn io_uring_wait_cqe(ring: *mut io_uring, res: &mut i32) -> *mut io_uring_cqe {
  rio_io_uring_wait_cqe(ring, res)
}

#[must_use]
pub unsafe fn io_uring_cqe_get_data(cqe: *const io_uring_cqe) -> *mut libc::c_void {
  rio_io_uring_cqe_get_data(cqe)
}

pub unsafe fn io_uring_cqe_seen(ring: *mut io_uring, cqe: *mut io_uring_cqe) {
  rio_io_uring_cqe_seen(ring, cqe);
}

#[must_use]
pub fn timerfd_create() -> i32 {
  unsafe { rio_timerfd_create() }
}

pub unsafe fn timerfd_settime(fd: i32, secs: u64, nanos: u64) -> Result<(), libc::Errno> {
  libc::errno(rio_timerfd_settime(fd, secs, nanos))
}

#[must_use]
pub fn make_pipe(pipefd: &mut [i32; 2]) -> i32 {
  unsafe { rio_make_pipe(pipefd.as_mut_ptr()) }
}

pub fn make_ipv4_tcp_server_socket(ipv4_addr: u32, port: u16) -> Result<i32, libc::Errno> {
  let mut fd = -1;
  let err = unsafe { rio_make_ipv4_tcp_server_socket(ipv4_addr, port, &mut fd) };
  match libc::errno(err) {
    Err(e) => Err(e),
    Ok(()) => Ok(fd),
  }
}
