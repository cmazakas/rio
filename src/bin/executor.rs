#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]
#![allow(clippy::missing_panics_doc)]

use std::{future::Future, io::Write, os::unix::prelude::AsRawFd};

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

    let mut client =
      if let Ok(client) = std::os::unix::net::UnixStream::connect("/tmp/asdfasdfasfasdf.socket") {
        client
      } else {
        println!("Client Unix domain socket failed to connect");
        return;
      };

    let mut res = -1;
    let cqe = rio::liburing::io_uring_wait_cqe(ring, &mut res);
    rio::liburing::io_uring_cqe_seen(ring, cqe);

    println!("Client Unix domain socket connected successfully!");
    println!("Connected socket is: {res}");

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

    // let mut tasks = Vec::<std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>>::new();
    // client.write_all(b"hello, world!").unwrap();

    let waker = std::sync::Arc::new(task::Waker {
      client: std::sync::Arc::new(std::sync::Mutex::new(client)),
    })
    .into();

    let mut fut = Box::pin(async {
      println!("Starting future!");
      println!("Going to sleep now");
      task::Sleeper::new().await;
      println!("Sleep is done!");
    });

    let mut cx = std::task::Context::from_waker(&waker);

    while fut.as_mut().poll(&mut cx).is_pending() {
      res = -1;
      let cqe = rio::liburing::io_uring_wait_cqe(ring, &mut res);
      rio::liburing::io_uring_cqe_seen(ring, cqe);
    }

    println!("Read is done!");
    if res == -1 {
      println!("Failed to read from Unix domain socket");
    } else {
      let nread = res as usize;
      println!("Read {res} bytes");
      println!("Message: {}", std::str::from_utf8(&buf[..nread]).unwrap());
    }

    rio::liburing::teardown(ring);
  }
}
