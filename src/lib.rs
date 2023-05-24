//! Fiona is a fast I/O runtime built on top of [`io_uring`](https://en.wikipedia.org/wiki/Io_uring). Originally, Fiona
//! began as a port of [Boost.Asio](https://www.boost.org/doc/libs/1_82_0/doc/html/boost_asio.html) so the APIs are
//! somewhat similar.
//!
//! Fiona aims to leverage the advantages of `io_uring` hence it is not cross-platform and only works on Linux.
//!
//! Fiona will eventually _require_ kernel v6.1 and above though right now, 5.15 should still work.
//!
//! Fiona is purposefully a single-threaded I/O runtime. Users are encouraged to handle multithreading by using multiple
//! TCP listeners, each on their own thread. This avoids the jittery latency introduced by synchronization primitives
//! and aligns with the liburing author's intention for the library. There are plans for a better multithreaded
//! solution.
//!
//! The library's main APIs for abstracting read(2) and write(2) are ownership-based. This is done for soundness
//! reasons. A user relinquishes ownership over a `Vec<u8>` to the `io_uring` runtime which is then restored upon
//! completion of the async operation, even in the case of errors.
//!
//! This library depends on liburing v2.4. The crate's build.rs does not attempt to download and build liburing for the
//! user. If the user does not already have an installation of liburing, it can be built easily by doing:
//!
//! ```bash
//! mkdir ~/__install__ \
//! && git clone https://github.com/axboe/liburing.git \
//! && cd liburing \
//! && ./configure --prefix=~/__install__
//! && make -j 4 \
//! && make install
//! ```
//!
//! Then in the consuming projects, one can add to their `.cargo/config.toml`:
//!
//! ```toml
//! [env]
//! RIO_LIBURING_INCLUDE_DIR = "/home/exbigboss/cpp/__install__/include"
//! RIO_LIBURING_LIBRARY_DIR = "/home/exbigboss/cpp/__install__/lib"
//! ```
//!
//! These environment variables must be set for the build to succeed.

#![warn(clippy::pedantic, missing_docs)]
#![allow(
    clippy::similar_names,
    clippy::missing_safety_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]
#![allow(non_camel_case_types)]

extern crate libc as ext_libc;
extern crate nix;

pub use nix::errno::Errno;
pub use rustls::Error as TLSError;

/// The main error type used by the library and directly exposed to the user.
#[derive(Debug)]
pub enum Error {
    /// Support the most ubiquitous Error type
    IO(std::io::Error),
    /// A pair of [`Errno`] and [`Vec<u8>`]. Used to transfer the caller's buffer back to them when an async operation
    /// completes with an error. This error occurs when underlying `io_uring` events themselves do not complete
    /// successfully.
    Errno(Errno, Vec<u8>),
    /// A pair of [`Errno`] and [`Vec<u8>`]. Used to tansfer the caller's buffer back to them when an async operation
    /// completes with an error. This error occurs during various TLS operations.
    TLS(TLSError, Vec<u8>),
}

/// Contains functions and classes associated with the internet protocol.
pub mod ip;
/// Contains simple helpers for working with various C APIs
pub mod libc;
/// An FFI wrapper for [liburing](https://github.com/axboe/liburing).
pub mod liburing;
mod op;
/// Contains functions and classes that support timers and timeouts.
pub mod time;
// pub mod probe;

type Task = dyn std::future::Future<Output = ()>;

struct NopWaker {}

#[repr(transparent)]
struct SyncUnsafeCell<T: ?Sized> {
    value: std::cell::UnsafeCell<T>,
}

impl<T> SyncUnsafeCell<T> {
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            value: std::cell::UnsafeCell::new(value),
        }
    }
}

unsafe impl<T> Send for SyncUnsafeCell<T> {}
unsafe impl<T> Sync for SyncUnsafeCell<T> {}

#[derive(Clone)]
struct WritePipe {
    fdp: std::sync::Arc<std::sync::Mutex<i32>>,
}

struct PipeWaker {
    p: WritePipe,
    task: SyncUnsafeCell<*mut Task>,
}

impl std::task::Wake for PipeWaker {
    fn wake(self: std::sync::Arc<Self>) {
        let fd_guard = self.p.fdp.lock().unwrap();

        unsafe {
            let taskp = *self.task.value.get();
            ext_libc::write(
                *fd_guard,
                std::ptr::addr_of!(taskp).cast::<ext_libc::c_void>(),
                std::mem::size_of::<*mut Task>(),
            );
        }
    }
}

/// A helper type designed to get the Waker associated with the Context supplied to an `async` block.
///
/// Example:
/// ```rust
/// let mut ioc = fiona::IoContext::new();
/// ioc.post(async {
///   let _waker = fiona::WakerFuture {}.await;
/// });
/// ioc.run();
/// ```
pub struct WakerFuture {}

impl std::future::Future for WakerFuture {
    type Output = std::task::Waker;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        std::task::Poll::Ready(cx.waker().clone())
    }
}

struct IoContextState {
    ring: *mut liburing::io_uring,
    task_ctx: Option<*mut Task>,
    tasks: std::collections::VecDeque<std::pin::Pin<Box<Task>>>,
}

/// The Fiona runtime.
///
/// Can be used to acquire an [`Executor`] which enables the creation of
/// I/O objects and enables the creation of work for the `IoContext` to run.
pub struct IoContext {
    p: std::rc::Rc<std::cell::UnsafeCell<IoContextState>>,
}

/// A lightweight handle to an [`IoContext`].
///
/// Can be used to create I/O objects and also schedule work on the associated `IoContext`.
#[derive(Clone)]
pub struct Executor {
    p: std::rc::Rc<std::cell::UnsafeCell<IoContextState>>,
}

impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
}

impl IoContextState {}

impl Drop for IoContextState {
    fn drop(&mut self) {
        unsafe { liburing::teardown(self.ring) }
    }
}

impl IoContext {
    /// Creates a default `IoContext`.
    #[must_use]
    pub fn new() -> Self {
        let ring = liburing::setup(32, 0);

        Self {
            p: std::rc::Rc::new(std::cell::UnsafeCell::new(IoContextState {
                ring,
                task_ctx: None,
                tasks: std::collections::VecDeque::default(),
            })),
        }
    }

    unsafe fn get_state(&self) -> *mut IoContextState {
        (*std::rc::Rc::as_ptr(&self.p)).get()
    }

    /// Submit a task `F` to the `IoContext` for completion. This function is non-blocking. Note, the supplied task will
    /// automatically be moved into a heap allocation so there's no need for `F` to be boxed.
    pub fn post<F>(&mut self, task: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        let mut ex = self.get_executor();
        ex.post(task);
    }

    /// Obtain a copy of the `Executor` associated with this `IoContext`.
    #[must_use]
    pub fn get_executor(&self) -> Executor {
        Executor { p: self.p.clone() }
    }

    /// Begin the main I/O processing loop. `run` blocks until all tasks are completed. Can be called multiple times.
    #[allow(clippy::too_many_lines)]
    pub fn run(&mut self) {
        pub struct CQESeenGuard {
            ring: *mut liburing::io_uring,
            cqe: *mut liburing::io_uring_cqe,
        }

        impl Drop for CQESeenGuard {
            fn drop(&mut self) {
                unsafe { liburing::io_uring_cqe_seen(self.ring, self.cqe) };
            }
        }

        struct PipeGuard {
            pipefd: [i32; 2],
        }

        impl Drop for PipeGuard {
            fn drop(&mut self) {
                unsafe {
                    ext_libc::close(self.pipefd[0]);
                    ext_libc::close(self.pipefd[1]);
                };
            }
        }

        struct TaskGuard<'a> {
            tasks: &'a mut std::collections::VecDeque<std::pin::Pin<Box<Task>>>,
        }

        impl<'a> Drop for TaskGuard<'a> {
            fn drop(&mut self) {
                self.tasks.clear();
            }
        }

        let mut pipefd = [-1_i32; 2];
        assert!(liburing::make_pipe(&mut pipefd) != -1, "unable to establish pipe!");

        let _pipe_guard = PipeGuard { pipefd };
        let write_pipe = WritePipe {
            fdp: std::sync::Arc::new(std::sync::Mutex::new(pipefd[1])),
        };

        let state = unsafe { &mut *self.get_state() };
        let ring = state.ring;

        let mut buf = std::mem::MaybeUninit::<*mut Task>::uninit();
        unsafe {
            std::ptr::write_bytes(std::ptr::addr_of_mut!(buf), 0, 1);
        }

        #[allow(clippy::cast_possible_truncation)]
        unsafe {
            let sqe = liburing::io_uring_get_sqe(ring);
            liburing::io_uring_prep_read(
                sqe,
                pipefd[0],
                buf.as_mut_ptr().cast::<_>(),
                std::mem::size_of::<*mut Task>() as u32,
                0,
            );
            liburing::io_uring_submit(ring);
        };

        let task_guard = TaskGuard {
            tasks: &mut state.tasks,
        };

        while !task_guard.tasks.is_empty() {
            let mut cqe = std::ptr::null_mut();
            let e = unsafe { liburing::io_uring_wait_cqe(ring, &mut cqe) };
            if e != 0 {
                assert!(e < 0);
                panic!("waiting internally for the cqe failed with: {:?}", -e);
            }
            assert!(!cqe.is_null());

            let res = unsafe { (*cqe).res };

            let _guard = CQESeenGuard { ring, cqe };
            let p = unsafe { liburing::io_uring_cqe_get_data(cqe) };

            let taskp: *mut Task = if p.is_null() {
                if res != std::mem::size_of::<*mut Task>().try_into().unwrap() {
                    continue;
                }

                let p = unsafe { buf.assume_init_read() };
                assert!(!p.is_null());

                #[allow(clippy::cast_possible_truncation)]
                unsafe {
                    let sqe = liburing::io_uring_get_sqe(ring);
                    liburing::io_uring_prep_read(
                        sqe,
                        pipefd[0],
                        buf.as_mut_ptr().cast::<_>(),
                        std::mem::size_of::<*mut Task>() as u32,
                        0,
                    );
                    liburing::io_uring_submit(ring);
                };

                p
            } else {
                let p = p.cast::<op::FdStateImpl>();
                // println!("the following task came in: {:?}", p);

                let fds = unsafe { op::FdState::from_raw(p) };
                let p = fds.get();

                unsafe {
                    (*p).done = true;
                    (*p).res = res;

                    // println!("res is: {res}");
                    // if res < 0 {
                    //   println!("errno: {:?}", libc::errno(-res));
                    // }
                }
                unsafe { (*p).task.take().unwrap() }
            };

            let mut it = task_guard.tasks.iter_mut();
            let Some(idx) =  it.position(|p| {
        (std::ptr::addr_of!(**p) as *const dyn std::future::Future<Output = ()>)
          .cast::<()>()
          == taskp.cast::<()>()
      }) else {
        continue;
      };

            state.task_ctx = Some(taskp);

            let pipe_waker = std::sync::Arc::new(PipeWaker {
                p: write_pipe.clone(),
                task: SyncUnsafeCell::new(taskp),
            })
            .into();

            let task = unsafe { std::pin::Pin::new_unchecked(&mut *taskp) };
            let mut cx = std::task::Context::from_waker(&pipe_waker);

            if task.poll(&mut cx).is_ready() {
                drop(task_guard.tasks.remove(idx));
            }
        }

        state.task_ctx = None;

        let mut cqe = std::ptr::null_mut::<liburing::io_uring_cqe>();
        while 0 == unsafe { liburing::io_uring_peek_cqe(ring, &mut cqe) } {
            assert!(!cqe.is_null());

            // println!("leftover cqe is: {:?}", unsafe {
            //   liburing::io_uring_cqe_get_data(cqe)
            // });

            let p = unsafe { liburing::io_uring_cqe_get_data(cqe) };
            if !p.is_null() {
                let p = p.cast::<op::FdStateImpl>();
                let _fds = unsafe { op::FdState::from_raw(p) };
            }
            unsafe { liburing::io_uring_cqe_seen(ring, cqe) };
        }
    }
}

impl std::default::Default for IoContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    fn get_state(&self) -> *mut IoContextState {
        unsafe { (*std::rc::Rc::as_ptr(&self.p)).get() }
    }

    /// Obtain a raw pointer to the underlying `io_uring` structure used by liburing.
    #[must_use]
    pub fn get_ring(&self) -> *mut liburing::io_uring {
        unsafe { (*self.get_state()).ring }
    }

    /// Post work to the `IoContext` associated with the current `Executor`. This function is non-blocking. Note, the
    /// supplied task will automatically be moved into a heap allocation so there's no need for `F` to be boxed.
    pub fn post<F>(&mut self, task: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        let mut task = Box::pin(task);
        let state = unsafe { &mut *self.get_state() };
        let taskp = unsafe { task.as_mut().get_unchecked_mut() as *mut _ };

        let fds = op::FdState::new(-1, op::Op::Null);
        let p = fds.get();
        unsafe { (*p).task = Some(taskp) };

        let ring = state.ring;
        let sqe = unsafe { liburing::io_uring_get_sqe(ring) };
        let user_data = fds.into_raw().cast::<ext_libc::c_void>();

        unsafe { liburing::io_uring_sqe_set_data(sqe, user_data) };
        unsafe { liburing::io_uring_prep_nop(sqe) };
        unsafe { liburing::io_uring_submit(ring) };

        state.tasks.push_back(task);
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<Errno> for Error {
    fn from(value: Errno) -> Self {
        Self::Errno(value, Vec::new())
    }
}

impl From<TLSError> for Error {
    fn from(value: TLSError) -> Self {
        Self::TLS(value, Vec::new())
    }
}
