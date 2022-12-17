extern crate rio;

use std::future::Future;

struct NopWaker {}
impl std::task::Wake for NopWaker {
  fn wake(self: std::sync::Arc<Self>) {}
}

#[test]
fn sockaddr_in_ffi_check() {
  let addr = rio::ip::tcp::sockaddr_in::default();
  let _addr2 = unsafe { rio::libc::rio_sockaddr_in_test(addr) };
}

#[test]
fn kernel_timespec_ffi_check() {
  let timespec = rio::libc::kernel_timespec {
    tv_nsec: 1337,
    tv_sec: 7331,
  };

  let t2 = unsafe { rio::libc::rio_timespec_test(timespec) };

  assert_eq!(t2.tv_sec, timespec.tv_sec);
  assert_eq!(t2.tv_nsec, timespec.tv_nsec);
}

#[test]
fn tcp_acceptor() {
  static mut NUM_RUNS: i32 = 0;

  async fn server(ex: rio::Executor) {
    let mut acceptor = rio::ip::tcp::Acceptor::new(ex);
    acceptor.listen(0x7f000001, 3300).unwrap();
    let mut stream = acceptor.async_accept().await.unwrap();

    let mut buf = vec![0_u8; 4096];
    unsafe {
      buf.set_len(0);
    }

    let buf = stream.async_read(buf).await.unwrap();

    assert_eq!(buf.len(), 13);
    let str = unsafe { std::str::from_utf8_unchecked(&buf[0..buf.len()]) };
    assert_eq!(str, "Hello, world!");

    unsafe { NUM_RUNS += 1 };
  }

  async fn client(ex: rio::Executor) {
    let mut client = rio::ip::tcp::Socket::new(ex);
    client.async_connect(0x7f000001, 3300).await.unwrap();

    let str = String::from("Hello, world!").into_bytes();
    client.async_write(str).await.unwrap();

    unsafe { NUM_RUNS += 1 };
  }

  let mut ioc = rio::IoContext::new();
  let ex = ioc.get_executor();

  ioc.post(Box::pin(server(ex.clone())));
  ioc.post(Box::pin(client(ex)));
  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn econnrefused_connect_future() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  let ex = ioc.get_executor();

  let mut timer = rio::time::Timer::new(ex.clone());
  ioc.post(Box::pin(async move {
    timer.expires_after(std::time::Duration::from_millis(1500));
    timer.async_wait().await.unwrap();

    unsafe { NUM_RUNS += 1 };
  }));

  let mut client = rio::ip::tcp::Socket::new(ex);
  ioc.post(Box::pin(async move {
    let r = client.async_connect(0x7f000001, 3301).await;

    match r {
      Err(e) => match e {
        rio::libc::Errno::ECONNREFUSED => {}
        _ => panic!("incorrect errno value, should be ECONNREFUSED"),
      },
      _ => panic!("expected an error when connecting"),
    }

    unsafe { NUM_RUNS += 1 };
  }));

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn connect_timeout() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  let ex = ioc.get_executor();

  let mut client = rio::ip::tcp::Socket::new(ex);
  ioc.post(Box::pin(async move {
    // use one of the IP addresses from the test networks:
    // 192.0.2.0/24
    // https://en.wikipedia.org/wiki/Internet_Protocol_version_4#Special-use_addresses
    let r = client.async_connect(0xc0000201, 3301).await;

    match r {
      Err(e) => match e {
        rio::libc::Errno::ECANCELED => {}
        _ => panic!("incorrect errno value, should be ECANCELED"),
      },
      _ => panic!("expected an error when connecting"),
    }

    println!("do I really get here????");

    unsafe { NUM_RUNS += 1 };
  }));

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
fn drop_accept_pending() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  let ex = ioc.get_executor();

  ioc.post(Box::pin(async {
    let mut acceptor = rio::ip::tcp::Acceptor::new(ex.clone());
    acceptor.listen(0x7f000001, 3302).unwrap();
    let mut f = acceptor.async_accept();

    let waker = std::sync::Arc::new(NopWaker {}).into();
    let mut cx = std::task::Context::from_waker(&waker);

    assert!(
      unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }
        .is_pending()
    );

    let mut timer = rio::time::Timer::new(ex);
    timer.expires_after(std::time::Duration::from_millis(500));
    timer.async_wait().await.unwrap();

    std::mem::drop(f);

    timer.async_wait().await.unwrap();

    unsafe { NUM_RUNS += 1 };
  }));

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
fn cancel_accept() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  let mut ex = ioc.get_executor();

  ioc.post(Box::pin(async move {
    let mut acceptor = rio::ip::tcp::Acceptor::new(ex.clone());
    acceptor.listen(0x7f000001, 3303).unwrap();
    let f = acceptor.async_accept();
    let c = f.get_cancel_handle();

    ex.post(Box::pin({
      let ex = ex.clone();
      async move {
        let mut timer = rio::time::Timer::new(ex);
        timer.expires_after(std::time::Duration::from_millis(500));
        timer.async_wait().await.unwrap();

        c.cancel();
      }
    }));

    let result = f.await;
    match result {
      Ok(_) => panic!("Ok is not valid for cancellation"),
      Err(err) => match err {
        rio::libc::Errno::ECANCELED => {}
        _ => panic!("incorrect errno value"),
      },
    }

    unsafe { NUM_RUNS += 1 };
  }));

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 1);
}
