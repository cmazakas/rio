#![warn(clippy::pedantic)]
#![allow(
  clippy::similar_names,
  clippy::missing_safety_doc,
  clippy::missing_panics_doc,
  clippy::cast_ptr_alignment,
  clippy::module_name_repetitions,
  clippy::missing_errors_doc,
  clippy::match_wildcard_for_single_variants
)]
#![allow(non_camel_case_types)]

pub mod ip;
pub mod libc;
pub mod liburing;
pub mod op;
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
      libc::write(
        *fd_guard,
        std::ptr::addr_of!(taskp).cast::<libc::c_void>(),
        std::mem::size_of::<*mut Task>(),
      );
    }
  }
}

pub struct WakerFuture {}

impl std::future::Future for WakerFuture {
  type Output = std::task::Waker;

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    std::task::Poll::Ready(cx.waker().clone())
  }
}

struct IoContextState {
  ring: *mut liburing::io_uring,
  task_ctx: Option<*mut Task>,
  tasks: std::collections::VecDeque<std::pin::Pin<Box<Task>>>,
}

pub struct IoContext {
  p: std::rc::Rc<std::cell::UnsafeCell<IoContextState>>,
}

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

  pub fn post(&mut self, task: std::pin::Pin<Box<Task>>) {
    let mut ex = self.get_executor();
    ex.post(task);
  }

  #[must_use]
  pub fn get_executor(&self) -> Executor {
    Executor { p: self.p.clone() }
  }

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
          libc::close(self.pipefd[0]);
          libc::close(self.pipefd[1]);
        };
      }
    }

    let mut pipefd = [-1_i32; 2];
    assert!(
      liburing::make_pipe(&mut pipefd) != -1,
      "unable to establish pipe!"
    );

    let _pipe_guard = PipeGuard { pipefd };
    let write_pipe = WritePipe {
      fdp: std::sync::Arc::new(std::sync::Mutex::new(pipefd[1])),
    };

    let state = unsafe { &mut *self.get_state() };
    let ring = state.ring;

    let mut buf = std::mem::MaybeUninit::<*mut Task>::uninit();
    #[allow(clippy::cast_possible_truncation)]
    unsafe {
      let sqe = liburing::make_sqe(ring);
      liburing::io_uring_prep_read(
        sqe,
        pipefd[0],
        buf.as_mut_ptr().cast::<_>(),
        std::mem::size_of::<*mut Task>() as u32,
        0,
      );
      liburing::io_uring_submit(ring);
    };

    while !state.tasks.is_empty() {
      let mut res = -1;
      let cqe = unsafe { liburing::io_uring_wait_cqe(ring, &mut res) };
      assert!(!cqe.is_null());

      let _guard = CQESeenGuard { ring, cqe };
      let p = unsafe { liburing::io_uring_cqe_get_data(cqe) };

      let taskp: *mut Task = if p.is_null() {
        if res != std::mem::size_of::<*mut Task>().try_into().unwrap() {
          continue;
        }

        let p = unsafe { buf.assume_init_read() };

        #[allow(clippy::cast_possible_truncation)]
        unsafe {
          let sqe = liburing::make_sqe(ring);
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
        let fds = unsafe { op::FdState::from_raw(p) };
        let p = fds.get();

        unsafe {
          (*p).done = true;
          (*p).res = res;
        }
        unsafe { (*p).task.take().unwrap() }
      };

      let mut it = state.tasks.iter_mut();
      let idx = match it.position(|p| {
        (std::ptr::addr_of!(**p) as *const dyn std::future::Future<Output = ()>).cast::<()>()
          == taskp.cast::<()>()
      }) {
        Some(idx) => idx,
        None => {
          continue;
        }
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
        drop(state.tasks.remove(idx));
      }
    }

    state.task_ctx = None;
  }
}

impl std::default::Default for IoContext {
  fn default() -> Self {
    Self::new()
  }
}

impl Executor {
  unsafe fn get_state(&self) -> *mut IoContextState {
    (*std::rc::Rc::as_ptr(&self.p)).get()
  }

  pub fn post(&mut self, mut task: std::pin::Pin<Box<Task>>) {
    let state = unsafe { &mut *self.get_state() };
    let taskp = unsafe { task.as_mut().get_unchecked_mut() as *mut _ };

    // let statep = std::rc::Rc::new(std::cell::UnsafeCell::new(FdFutureSharedState {
    //   done: false,
    //   fd: -1,
    //   res: -1,
    //   task: Some(taskp),
    //   disarmed: false,
    // }));

    let fds = op::FdState::new(-1, op::Op::Null);
    let p = fds.get();
    unsafe { (*p).task = Some(taskp) };

    let ring = state.ring;
    let sqe = unsafe { liburing::make_sqe(ring) };
    let user_data = fds.into_raw().cast::<libc::c_void>();

    unsafe { liburing::io_uring_sqe_set_data(sqe, user_data) };
    unsafe { liburing::io_uring_prep_nop(sqe) };
    unsafe { liburing::io_uring_submit(ring) };

    state.tasks.push_back(task);
  }
}
