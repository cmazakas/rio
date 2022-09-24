#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use std::{future::Future, os::unix::prelude::AsRawFd};

extern crate rio;

mod task {
  use std::io::Write;

  pub struct Sleeper {
    t: Option<std::thread::JoinHandle<()>>,
  }

  impl Sleeper {
    pub fn new() -> Self {
      Self { t: None }
    }
  }

  impl std::future::Future for Sleeper {
    type Output = ();
    fn poll(
      mut self: std::pin::Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
      if self.as_mut().t.is_none() {
        let waker = cx.waker().clone();
        self.t = Some(std::thread::spawn(move || {
          std::thread::sleep(std::time::Duration::from_secs(3));
          waker.wake();
        }));

        std::task::Poll::Pending
      } else {
        let t = self.as_mut().t.take().unwrap();
        t.join().unwrap();
        std::task::Poll::Ready(())
      }
    }
  }

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

  pub struct IoContext {
    pub ring: *mut rio::liburing::io_uring,
  }

  pub struct TimerFutureSharedState {
    pub done: bool,
  }

  struct TimerFuture {
    state: std::rc::Rc<std::cell::UnsafeCell<TimerFutureSharedState>>,
  }

  impl std::future::Future for TimerFuture {
    type Output = ();

    fn poll(
      self: std::pin::Pin<&mut Self>,
      _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
      let p = unsafe { (*std::rc::Rc::as_ptr(&(*self).state)).get() };
      let done = unsafe { (*p).done };

      if done {
        std::task::Poll::Ready(())
      } else {
        std::task::Poll::Pending
      }
    }
  }

  pub struct Timer {
    fd: i32,
    millis: i32,
    ioc: std::rc::Rc<IoContext>,
    buf: std::pin::Pin<Box<u64>>,
  }

  impl Timer {
    pub fn new(ioc: std::rc::Rc<IoContext>) -> Self {
      let fd = rio::liburing::timerfd_create();
      assert!(fd != -1, "Can't create a timer");

      Self {
        fd,
        millis: 0,
        ioc,
        buf: Box::pin(0_u64),
      }
    }

    pub fn expires_after(&mut self, millis: i32) {
      self.millis = millis;
    }

    pub fn async_wait(&mut self) -> impl std::future::Future<Output = ()> {
      unsafe {
        assert!(
          -1 != rio::liburing::timerfd_settime(self.fd, self.millis),
          "Failed to set timer on FD"
        );

        let fut = TimerFuture {
          state: std::rc::Rc::new(std::cell::UnsafeCell::new(TimerFutureSharedState {
            done: false,
          })),
        };

        let ring = (*std::rc::Rc::as_ptr(&self.ioc)).ring;
        let sqe = rio::liburing::make_sqe(ring);
        let buf = std::ptr::addr_of_mut!(*self.buf).cast::<rio::libc::c_void>();

        let p = fut.state.clone();
        rio::liburing::io_uring_sqe_set_data(
          sqe,
          std::rc::Rc::into_raw(p).cast::<rio::libc::c_void>() as *mut rio::libc::c_void,
        );

        rio::liburing::io_uring_prep_read(sqe, self.fd, buf, 8, 0);
        rio::liburing::io_uring_submit(ring);

        fut
      }
    }
  }
}

pub fn main() {
  unsafe {
    let ring = rio::liburing::setup(16, 0);
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

    let mut tasks = Vec::<std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>>::new();

    let waker = std::sync::Arc::new(task::Waker {
      client: std::sync::Arc::new(std::sync::Mutex::new(client)),
    })
    .into();

    tasks.push(Box::pin(async {
      println!("Starting the timer coro...");
      let ioc = std::rc::Rc::new(io::IoContext { ring });

      let mut timer = io::Timer::new(ioc);
      timer.expires_after(5000);

      println!("Suspending now...");
      timer.async_wait().await;

      println!("Holy shit, it actually works");
    }));

    tasks.push(Box::pin(async {
      println!("Starting the timer coro...");
      let ioc = std::rc::Rc::new(io::IoContext { ring });

      let mut timer = io::Timer::new(ioc);
      timer.expires_after(5000);

      println!("Suspending now...");
      timer.async_wait().await;

      println!("Holy shit, it actually works");
    }));

    tasks.push(Box::pin(async {
      println!("Starting the timer coro...");
      let ioc = std::rc::Rc::new(io::IoContext { ring });

      let mut timer = io::Timer::new(ioc);
      timer.expires_after(5000);

      println!("Suspending now...");
      timer.async_wait().await;

      println!("Holy shit, it actually works");
    }));

    tasks.push(Box::pin(async {
      println!("Starting the timer coro...");
      let ioc = std::rc::Rc::new(io::IoContext { ring });

      let mut timer = io::Timer::new(ioc);
      timer.expires_after(5000);

      println!("Suspending now...");
      timer.async_wait().await;

      println!("Holy shit, it actually works");
    }));

    'outer: while !tasks.is_empty() {
      let mut idx = 0;
      while idx < tasks.len() {
        let fut = &mut tasks[idx];
        let mut cx = std::task::Context::from_waker(&waker);
        if fut.as_mut().poll(&mut cx).is_ready() {
          tasks.remove(idx);
          if tasks.is_empty() {
            break 'outer;
          }
        } else {
          idx += 1;
        }
      }

      let cqe = rio::liburing::io_uring_wait_cqe(ring, &mut res);
      let p = rio::liburing::io_uring_cqe_get_data(cqe);
      if !p.is_null() {
        let p = p as *const _ as *const std::cell::UnsafeCell<io::TimerFutureSharedState>;
        let state = std::rc::Rc::from_raw(p);

        let p = (*std::rc::Rc::as_ptr(&state)).get();
        (*p).done = true;
      }
      rio::liburing::io_uring_cqe_seen(ring, cqe);
    }

    rio::liburing::teardown(ring);
  }
}
