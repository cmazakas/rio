#![warn(clippy::pedantic)]
#![allow(
  clippy::missing_panics_doc,
  clippy::too_many_lines,
  clippy::module_name_repetitions,
  clippy::similar_names,
  clippy::cast_ptr_alignment
)]

use std::os::unix::prelude::AsRawFd;

extern crate rio;

mod task {
  use std::io::Write;

  // pub struct Sleeper {
  //   t: Option<std::thread::JoinHandle<()>>,
  // }

  // impl Sleeper {
  //   pub fn new() -> Self {
  //     Self { t: None }
  //   }
  // }

  // impl std::future::Future for Sleeper {
  //   type Output = ();
  //   fn poll(
  //     mut self: std::pin::Pin<&mut Self>,
  //     cx: &mut std::task::Context<'_>,
  //   ) -> std::task::Poll<Self::Output> {
  //     if self.as_mut().t.is_none() {
  //       let waker = cx.waker().clone();
  //       self.t = Some(std::thread::spawn(move || {
  //         std::thread::sleep(std::time::Duration::from_secs(3));
  //         waker.wake();
  //       }));

  //       std::task::Poll::Pending
  //     } else {
  //       let t = self.as_mut().t.take().unwrap();
  //       t.join().unwrap();
  //       std::task::Poll::Ready(())
  //     }
  //   }
  // }

  pub struct Waker {
    pub client: std::sync::Arc<std::sync::Mutex<std::os::unix::net::UnixStream>>,
  }

  impl std::task::Wake for Waker {
    fn wake(self: std::sync::Arc<Self>) {
      let mut client = self.client.lock().unwrap();
      client.write_all(b"rawr").unwrap();
    }
  }
}

mod io {
  pub struct IoContextState {
    pub ring: *mut rio::liburing::io_uring,
    pub task: Option<*mut dyn std::future::Future<Output = ()>>,
    pub fd_task_map: std::collections::HashMap<i32, *mut dyn std::future::Future<Output = ()>>,
  }

  impl Drop for IoContextState {
    fn drop(&mut self) {
      unsafe { rio::liburing::teardown(self.ring) }
    }
  }

  #[derive(Clone)]
  pub struct IoContext {
    p: std::rc::Rc<std::cell::UnsafeCell<IoContextState>>,
  }

  impl IoContext {
    pub fn new() -> Self {
      let ring = rio::liburing::setup(32, 0);

      Self {
        p: std::rc::Rc::new(std::cell::UnsafeCell::new(IoContextState {
          ring,
          task: None,
          fd_task_map: std::collections::HashMap::default(),
        })),
      }
    }

    pub unsafe fn get_state(&self) -> *mut IoContextState {
      (*std::rc::Rc::as_ptr(&self.p)).get()
    }

    pub unsafe fn get_ring(&self) -> *mut rio::liburing::io_uring {
      (*self.get_state()).ring
    }
  }

  pub struct FdFutureSharedState {
    pub done: bool,
    pub fd: i32,
    pub res: i32,
  }

  // pub struct FdFuture {
  //   state: std::rc::Rc<std::cell::UnsafeCell<FdFutureSharedState>>,
  // }

  // impl std::future::Future for FdFuture {
  //   type Output = ();

  //   fn poll(
  //     self: std::pin::Pin<&mut Self>,
  //     _cx: &mut std::task::Context<'_>,
  //   ) -> std::task::Poll<Self::Output> {
  //     let p = unsafe { (*std::rc::Rc::as_ptr(&(*self).state)).get() };
  //     let done = unsafe { (*p).done };
  //     if done {
  //       std::task::Poll::Ready(())
  //     } else {
  //       std::task::Poll::Pending
  //     }
  //   }
  // }

  pub struct TimerFuture {
    initiated: bool,
    ioc: IoContext,
    state: std::rc::Rc<std::cell::UnsafeCell<FdFutureSharedState>>,
    buf: std::pin::Pin<Box<u64>>,
  }

  impl TimerFuture {
    fn new(ioc: IoContext, state: std::rc::Rc<std::cell::UnsafeCell<FdFutureSharedState>>) -> Self {
      Self {
        initiated: false,
        state,
        ioc,
        buf: Box::pin(0_u64),
      }
    }
  }

  pub enum Err {
    ReadFailed,
  }

  impl std::future::Future for TimerFuture {
    type Output = Result<(), Err>;

    fn poll(
      mut self: std::pin::Pin<&mut Self>,
      _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
      let p = unsafe { (*std::rc::Rc::as_ptr(&(*self).state)).get() };

      if !self.initiated {
        let fd = unsafe { (*p).fd };
        let ioc_state = unsafe { &mut *self.ioc.get_state() };
        ioc_state.fd_task_map.insert(fd, ioc_state.task.unwrap());

        let ring = ioc_state.ring;
        let sqe = unsafe { rio::liburing::make_sqe(ring) };
        let buf = std::ptr::addr_of_mut!(*self.buf).cast::<rio::libc::c_void>();

        unsafe {
          rio::liburing::io_uring_sqe_set_data(
            sqe,
            std::rc::Rc::into_raw(self.state.clone()).cast::<rio::libc::c_void>()
              as *mut rio::libc::c_void,
          );
        }

        unsafe { rio::liburing::io_uring_prep_read(sqe, fd, buf, 8, 0) };
        unsafe { rio::liburing::io_uring_submit(ring) };

        self.initiated = true;

        return std::task::Poll::Pending;
      }

      let done = unsafe { (*p).done };
      if done {
        if unsafe { (*p).res < 0 } {
          return std::task::Poll::Ready(Err(Err::ReadFailed));
        }
        std::task::Poll::Ready(Ok(()))
      } else {
        std::task::Poll::Pending
      }
    }
  }

  pub struct Timer {
    fd: i32,
    millis: i32,
    ioc: IoContext,
  }

  impl Timer {
    pub fn new(ioc: IoContext) -> Self {
      let fd = rio::liburing::timerfd_create();
      assert!(fd != -1, "Can't create a timer");

      Self { fd, millis: 0, ioc }
    }

    pub fn expires_after(&mut self, millis: i32) {
      self.millis = millis;
    }

    pub fn async_wait(&mut self) -> impl std::future::Future<Output = Result<(), Err>> + '_ {
      unsafe {
        assert!(
          -1 != rio::liburing::timerfd_settime(self.fd, self.millis),
          "Failed to set timer on FD"
        );

        let fd = self.fd;

        let shared_statep = std::rc::Rc::new(std::cell::UnsafeCell::new(FdFutureSharedState {
          done: false,
          fd,
          res: -1,
        }));

        TimerFuture::new(self.ioc.clone(), shared_statep)
      }
    }
  }
}

pub fn main() {
  pub struct CQESeenGuard {
    ring: *mut rio::liburing::io_uring,
    cqe: *mut rio::liburing::io_uring_cqe,
  }

  impl Drop for CQESeenGuard {
    fn drop(&mut self) {
      unsafe { rio::liburing::io_uring_cqe_seen(self.ring, self.cqe) };
    }
  }

  unsafe {
    let ioc = io::IoContext::new();
    let ring = ioc.get_ring();

    let sqe = rio::liburing::make_sqe(ring);

    let listener =
      if let Ok(listener) = std::os::unix::net::UnixListener::bind("/tmp/asdfasdfasfasdf.socket") {
        listener
      } else {
        println!("failed to acquire local unix socket!");
        return;
      };

    let listener_fd = listener.as_raw_fd();

    rio::liburing::io_uring_prep_accept_af_unix(sqe, listener_fd);
    rio::liburing::io_uring_submit(ring);

    let client =
      if let Ok(client) = std::os::unix::net::UnixStream::connect("/tmp/asdfasdfasfasdf.socket") {
        client
      } else {
        println!("Client Unix domain socket failed to connect");
        return;
      };

    let mut res = -1;
    let cqe = rio::liburing::io_uring_wait_cqe(ring, &mut res);
    rio::liburing::io_uring_cqe_seen(ring, cqe);

    let server_fd = res;
    let mut buf = [0_u8; 512];

    let sqe = rio::liburing::make_sqe(ring);
    rio::liburing::io_uring_prep_read(
      sqe,
      server_fd,
      (&mut buf)[..].as_mut_ptr().cast::<rio::libc::c_void>(),
      buf.len().try_into().unwrap(),
      0,
    );

    rio::liburing::io_uring_submit(ring);

    let mut tasks = Vec::<Box<dyn std::future::Future<Output = ()>>>::new();

    for idx in 0..5 {
      let ioc = ioc.clone();
      tasks.push(Box::new(async move {
        println!("Starting the timer coro...");

        let mut timer = io::Timer::new(ioc);
        let time = (idx + 1) * 1000;
        timer.expires_after(time);

        println!("Suspending now...");
        match timer.async_wait().await {
          Ok(_) => {
            println!("waited successfully for {} seconds!", idx + 1);
          }
          Err(_) => {
            println!("Timer read failed!");
          }
        }

        println!("Going to wait again...");
        match timer.async_wait().await {
          Ok(_) => {
            println!("waited succesfully, again, for {} seconds", idx + 1);
          }
          Err(_) => {
            println!("Timer read failed!");
          }
        }
      }));
    }

    let waker = std::sync::Arc::new(task::Waker {
      client: std::sync::Arc::new(std::sync::Mutex::new(client)),
    })
    .into();

    let mut idx = 0;
    while idx < tasks.len() {
      let state = &mut *ioc.get_state();
      let task: *mut _ = std::ptr::addr_of_mut!(*tasks[idx]);
      state.task = Some(task);

      let mut fut = std::pin::Pin::new_unchecked(&mut *tasks[idx]);
      let mut cx = std::task::Context::from_waker(&waker);

      if fut.as_mut().poll(&mut cx).is_ready() {
        drop(tasks.remove(idx));
        if tasks.is_empty() {
          break;
        }
      } else {
        idx += 1;
      }
    }

    while !tasks.is_empty() {
      let cqe = rio::liburing::io_uring_wait_cqe(ring, &mut res);
      let _guard = CQESeenGuard { ring, cqe };

      let p = rio::liburing::io_uring_cqe_get_data(cqe);
      if !p.is_null() {
        let p = p.cast::<std::cell::UnsafeCell<io::FdFutureSharedState>>();
        let state = std::rc::Rc::from_raw(p);

        let p = (*std::rc::Rc::as_ptr(&state)).get();
        (*p).done = true;
        (*p).res = res;

        let fd = (*p).fd;

        let ioc_state = &mut *ioc.get_state();

        let task = ioc_state.fd_task_map.get(&fd).unwrap();
        let taskp = *task;
        ioc_state.fd_task_map.remove(&fd);

        ioc_state.task = Some(taskp);

        let is_done = {
          let task = std::pin::Pin::new_unchecked(&mut *taskp);
          let mut cx = std::task::Context::from_waker(&waker);

          task.poll(&mut cx).is_ready()
        };

        if is_done {
          let mut idx = 0;
          while idx < tasks.len() {
            let a = (std::ptr::addr_of!(*tasks[idx])
              as *const dyn std::future::Future<Output = ()>)
              .cast::<()>();

            let b = taskp as *const ();
            if a == b {
              drop(tasks.remove(idx));
              break;
            }
            idx += 1;
          }
        }
      }
    }

    println!("All tasks completed running");
  }
}
