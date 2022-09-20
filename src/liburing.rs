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
  fn rio_io_uring_submit(ring: *mut io_uring) -> i32;
  fn rio_io_uring_prep_accept_af_unix(sqe: *mut io_uring_sqe, fd: i32);
  fn rio_io_uring_prep_read(
    sqe: *mut io_uring_sqe,
    fd: i32,
    buf: *mut libc::c_void,
    nbytes: u32,
    offset: i64,
  );
  fn rio_io_uring_wait_cqe(ring: *mut io_uring, res: &mut i32) -> *mut io_uring_cqe;
  fn rio_io_uring_cqe_seen(ring: *mut io_uring, cqe: *mut io_uring_cqe);
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

pub unsafe fn io_uring_prep_accept_af_unix(sqe: *mut io_uring_sqe, fd: i32) {
  rio_io_uring_prep_accept_af_unix(sqe, fd);
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

pub unsafe fn io_uring_submit(ring: *mut io_uring) -> i32 {
  rio_io_uring_submit(ring)
}

pub unsafe fn io_uring_wait_cqe(ring: *mut io_uring, res: &mut i32) -> *mut io_uring_cqe {
  rio_io_uring_wait_cqe(ring, res)
}

pub unsafe fn io_uring_cqe_seen(ring: *mut io_uring, cqe: *mut io_uring_cqe) {
  rio_io_uring_cqe_seen(ring, cqe);
}
