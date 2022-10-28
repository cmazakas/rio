#![warn(clippy::pedantic)]
#![allow(
  clippy::similar_names,
  clippy::missing_panics_doc,
  clippy::cast_ptr_alignment,
  clippy::module_name_repetitions,
  clippy::missing_errors_doc
)]
#![allow(non_camel_case_types)]

pub mod io;
pub mod libc;
pub mod liburing;
// pub mod probe;

type Task = dyn std::future::Future<Output = ()>;

struct NopWaker {}

struct FdFutureSharedState {
  pub done: bool,
  pub fd: i32,
  pub res: i32,
  pub task: Option<*mut Task>,
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

    let waker = std::sync::Arc::new(NopWaker {}).into();
    let state = unsafe { &mut *self.get_state() };

    while !state.tasks.is_empty() {
      let mut res = -1;
      let ring = state.ring;
      let cqe = unsafe { liburing::io_uring_wait_cqe(ring, &mut res) };

      let _guard = CQESeenGuard { ring, cqe };
      let p = unsafe { liburing::io_uring_cqe_get_data(cqe) };
      if p.is_null() {
        continue;
      }

      let p = p.cast::<std::cell::UnsafeCell<FdFutureSharedState>>();
      let cqe_state = unsafe { std::rc::Rc::from_raw(p) };

      let p: *mut FdFutureSharedState = unsafe { (*std::rc::Rc::as_ptr(&cqe_state)).get() };
      unsafe {
        (*p).done = true;
        (*p).res = res;
      }

      let taskp = unsafe { (*p).task.take().unwrap() };

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

      let is_done = {
        let task = unsafe { std::pin::Pin::new_unchecked(&mut *taskp) };
        let mut cx = std::task::Context::from_waker(&waker);

        task.poll(&mut cx).is_ready()
      };

      if is_done {
        drop(state.tasks.remove(idx));
      }
    }
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

    let statep = std::rc::Rc::new(std::cell::UnsafeCell::new(FdFutureSharedState {
      done: false,
      fd: -1,
      res: -1,
      task: Some(taskp),
    }));

    let ring = state.ring;
    let sqe = unsafe { liburing::make_sqe(ring) };
    let user_data = std::rc::Rc::into_raw(statep).cast::<libc::c_void>();

    unsafe { liburing::io_uring_sqe_set_data(sqe, user_data as *mut _) };
    unsafe { liburing::io_uring_prep_nop(sqe) };
    unsafe { liburing::io_uring_submit(ring) };

    state.tasks.push_back(task);
  }
}
