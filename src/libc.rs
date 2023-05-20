extern crate libc;

/// A simple port of the timespec struct that's preferred by `io_uring`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct kernel_timespec {
    /// The time value in seconds.
    pub tv_sec: i64,
    /// The time value in nanoseconds. Cannot be more than 9 digits.
    pub tv_nsec: i64,
}

extern "C" {
    // TODO: find out if this function even needs to be here specifically.
    #[doc(hidden)]
    pub fn rio_timespec_test(ts: kernel_timespec) -> kernel_timespec;
}
