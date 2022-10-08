#![warn(clippy::pedantic)]
#![allow(
  clippy::similar_names,
  clippy::missing_panics_doc,
  clippy::cast_ptr_alignment,
  clippy::module_name_repetitions
)]
#![allow(non_camel_case_types)]

pub mod io;
pub mod libc;
pub mod liburing;
// pub mod probe;

type Task = dyn std::future::Future<Output = ()>;

struct NopWaker {}

impl std::task::Wake for NopWaker {
  fn wake(self: std::sync::Arc<Self>) {}
}

struct FdFutureSharedState {
  pub done: bool,
  pub fd: i32,
  pub res: i32,
}

struct IoContextState {
  ring: *mut liburing::io_uring,
  task_ctx: Option<*mut dyn std::future::Future<Output = ()>>,
  tasks: Vec<Box<Task>>,
  fd_task_map: std::collections::HashMap<i32, *mut dyn std::future::Future<Output = ()>>,
}

impl IoContextState {}

impl Drop for IoContextState {
  fn drop(&mut self) {
    unsafe { liburing::teardown(self.ring) }
  }
}

#[derive(Clone)]
pub struct IoContext {
  p: std::rc::Rc<std::cell::UnsafeCell<IoContextState>>,
}

impl IoContext {
  #[must_use]
  pub fn new() -> Self {
    let ring = liburing::setup(32, 0);

    Self {
      p: std::rc::Rc::new(std::cell::UnsafeCell::new(IoContextState {
        ring,
        task_ctx: None,
        tasks: Vec::default(),
        fd_task_map: std::collections::HashMap::default(),
      })),
    }
  }

  unsafe fn get_state(&self) -> *mut IoContextState {
    (*std::rc::Rc::as_ptr(&self.p)).get()
  }

  pub fn post(&mut self, task: Box<Task>) {
    let state = unsafe { &mut *self.get_state() };
    state.tasks.push(task);
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

    let waker = std::sync::Arc::new(NopWaker {}).into();

    let state = unsafe { &mut *self.get_state() };

    let mut idx = 0;
    while idx < state.tasks.len() {
      let task: *mut _ = std::ptr::addr_of_mut!(*state.tasks[idx]);
      state.task_ctx = Some(task);

      let mut fut = unsafe { std::pin::Pin::new_unchecked(&mut *state.tasks[idx]) };
      let mut cx = std::task::Context::from_waker(&waker);

      if fut.as_mut().poll(&mut cx).is_ready() {
        drop(state.tasks.remove(idx));
        if state.tasks.is_empty() {
          break;
        }
      } else {
        idx += 1;
      }
    }

    let mut res = -1;
    while !state.tasks.is_empty() {
      let ring = state.ring;
      let cqe = unsafe { liburing::io_uring_wait_cqe(ring, &mut res) };
      let _guard = CQESeenGuard { ring, cqe };

      let p = unsafe { liburing::io_uring_cqe_get_data(cqe) };

      if !p.is_null() {
        let p = p.cast::<std::cell::UnsafeCell<FdFutureSharedState>>();
        let cqe_state = unsafe { std::rc::Rc::from_raw(p) };

        let p = unsafe { (*std::rc::Rc::as_ptr(&cqe_state)).get() };
        unsafe {
          (*p).done = true;
          (*p).res = res;
        }

        let fd = unsafe { (*p).fd };

        let task = state.fd_task_map.get(&fd).unwrap();
        let taskp = *task;
        state.fd_task_map.remove(&fd);

        state.task_ctx = Some(taskp);

        let is_done = {
          let task = unsafe { std::pin::Pin::new_unchecked(&mut *taskp) };
          let mut cx = std::task::Context::from_waker(&waker);

          task.poll(&mut cx).is_ready()
        };

        if is_done {
          let mut idx = 0;
          while idx < state.tasks.len() {
            let a = (std::ptr::addr_of!(*state.tasks[idx])
              as *const dyn std::future::Future<Output = ()>)
              .cast::<()>();

            let b = taskp as *const ();
            if a == b {
              drop(state.tasks.remove(idx));
              break;
            }
            idx += 1;
          }
        }
      }
    }
  }
}

impl std::default::Default for IoContext {
  fn default() -> Self {
    Self::new()
  }
}
