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
fn tcp_acceptor() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  let ex = ioc.get_executor();
  ioc.post({
    let ex = ex.clone();
    Box::pin(async {
      let mut acceptor = rio::ip::tcp::Acceptor::new(ex);
      acceptor.listen(0x7f000001, 3300).unwrap();
      let _stream = acceptor.async_accept().await.unwrap();

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.post(Box::pin(async {
    let mut client = rio::ip::tcp::Socket::new(ex);
    client.async_connect(0x7f000001, 3300).await.unwrap();
    unsafe { NUM_RUNS += 1 };
  }));

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn drop_accept_pending() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  let ex = ioc.get_executor();

  ioc.post(Box::pin(async {
    let mut acceptor = rio::ip::tcp::Acceptor::new(ex.clone());
    acceptor.listen(0x7f000001, 3301).unwrap();
    let mut f = acceptor.async_accept();

    let waker = std::sync::Arc::new(NopWaker {}).into();
    let mut cx = std::task::Context::from_waker(&waker);

    assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());

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
