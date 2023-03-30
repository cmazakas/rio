#![allow(clippy::missing_safety_doc)]

use crate::{self as fiona};

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

    fn rio_make_pipe(pipefd: *mut i32) -> i32;
    fn rio_make_ipv4_tcp_server_socket(
        ipv4_addr: u32,
        port: u16,
        fdp: *mut i32,
    ) -> i32;

    fn rio_make_ipv6_tcp_server_socket(
        ipv6_addr: libc::in6_addr,
        port: u16,
        pfd: *mut i32,
    ) -> i32;

    pub fn io_uring_submit(ring: *mut io_uring) -> i32;
    pub fn io_uring_get_sqe(ring: *mut io_uring) -> *mut io_uring_sqe;
    pub fn io_uring_wait_cqe(
        ring: *mut io_uring,
        cqe_ptr: *mut *mut io_uring_cqe,
    ) -> i32;
    pub fn io_uring_peek_cqe(
        ring: *mut io_uring,
        cqe_ptr: *mut *mut io_uring_cqe,
    ) -> i32;
    pub fn io_uring_cqe_seen(ring: *mut io_uring, cqe: *mut io_uring_cqe);
    pub fn io_uring_sqe_set_data(
        sqe: *mut io_uring_sqe,
        data: *mut libc::c_void,
    );
    pub fn io_uring_cqe_get_data(cqe: *const io_uring_cqe)
        -> *mut libc::c_void;
    pub fn io_uring_sqe_set_flags(sqe: *mut io_uring_sqe, flags: u32);
    pub fn io_uring_prep_link_timeout(
        sqe: *mut io_uring_sqe,
        ts: *mut fiona::libc::kernel_timespec,
        flags: u32,
    );

    pub fn io_uring_prep_accept(
        sqe: *mut io_uring_sqe,
        fd: i32,
        addr: *mut libc::sockaddr,
        addrlen: *mut u32,
        flags: i32,
    );

    pub fn io_uring_prep_connect(
        sqe: *mut io_uring_sqe,
        sockfd: i32,
        addr: *const libc::sockaddr,
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
        buf: *const libc::c_void,
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
    /*
     * we use this function from our own libc to avoid having to spell out the
     * entire io_uring structure in our Rust code
     */
    unsafe { rio_setup(entries, flags) }
}

pub unsafe fn teardown(ring: *mut io_uring) {
    rio_teardown(ring);
}

#[must_use]
pub fn make_pipe(pipefd: &mut [i32; 2]) -> i32 {
    unsafe { rio_make_pipe(pipefd.as_mut_ptr()) }
}

pub fn make_ipv4_tcp_server_socket(
    ipv4_addr: u32,
    port: u16,
) -> Result<i32, i32> {
    let mut fd = -1;
    let err =
        unsafe { rio_make_ipv4_tcp_server_socket(ipv4_addr, port, &mut fd) };

    match err {
        0 => Ok(fd),
        e => Err(e),
    }
}

pub fn make_ipv6_tcp_server_socket(
    ipv6_addr: libc::in6_addr,
    port: u16,
) -> Result<i32, i32> {
    let mut fd = -1;
    let err =
        unsafe { rio_make_ipv6_tcp_server_socket(ipv6_addr, port, &mut fd) };

    match err {
        0 => Ok(fd),
        e => Err(e),
    }
}

pub fn make_ipv4_tcp_socket() -> Result<i32, i32> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if fd == -1 {
        return Err(std::io::Error::last_os_error().raw_os_error().unwrap());
    }

    Ok(fd)
}
