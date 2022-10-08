use std::future::Future;

extern crate rio;

#[test]
fn eager_future() {
  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new(async {
    unsafe {
      WAS_RUN = true;
    }
  }));
  ioc.run();

  assert!(unsafe { WAS_RUN });
}

#[test]
fn timer() {
  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      let mut timer = rio::io::Timer::new(ioc);
      timer.expires_after(500);
      timer.async_wait().await.unwrap();
      unsafe {
        WAS_RUN = true;
      }
    }
  }));
  ioc.run();

  assert!(unsafe { WAS_RUN });
}

#[test]
fn timer_consecutive() {
  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      // make sure we can wait twice in a row
      //
      let mut timer = rio::io::Timer::new(ioc);
      timer.expires_after(500);
      timer.async_wait().await.unwrap();
      timer.async_wait().await.unwrap();
      unsafe {
        WAS_RUN = true;
      }
    }
  }));
  ioc.run();

  assert!(unsafe { WAS_RUN });
}

#[test]
fn timer_multiple_concurrent() {
  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      // make sure we can wait twice in a row
      //
      let mut timer1 = rio::io::Timer::new(ioc.clone());
      let mut timer2 = rio::io::Timer::new(ioc.clone());
      timer1.expires_after(500);
      timer2.expires_after(750);
      timer2.async_wait().await.unwrap();
      timer1.async_wait().await.unwrap();
      unsafe {
        WAS_RUN = true;
      }
    }
  }));
  ioc.run();

  assert!(unsafe { WAS_RUN });
}

#[test]
fn timer_multiple_concurrent_manually_polled() {
  struct NopWaker {}
  impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
  }

  // abuse manual polling of Futures
  //
  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      let mut timer1 = rio::io::Timer::new(ioc.clone());
      let mut timer2 = rio::io::Timer::new(ioc.clone());
      let mut timer3 = rio::io::Timer::new(ioc.clone());

      timer1.expires_after(1000);
      timer2.expires_after(2000);
      timer3.expires_after(4000);

      let mut f1 = timer1.async_wait();
      let mut f2 = timer2.async_wait();
      let mut f3 = timer3.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      {
        let mut cx = std::task::Context::from_waker(&waker);
        assert!(unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }.is_pending());
      }

      {
        let mut cx = std::task::Context::from_waker(&waker);
        assert!(unsafe { std::pin::Pin::new_unchecked(&mut f2).poll(&mut cx) }.is_pending());
      }

      {
        let mut cx = std::task::Context::from_waker(&waker);
        assert!(unsafe { std::pin::Pin::new_unchecked(&mut f3).poll(&mut cx) }.is_pending());
      }

      f2.await.unwrap();
      f3.await.unwrap();
      f1.await.unwrap();

      unsafe {
        WAS_RUN = true;
      }
    }
  }));
  ioc.run();

  assert!(unsafe { WAS_RUN });
}
