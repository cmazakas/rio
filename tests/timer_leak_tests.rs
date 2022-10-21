extern crate rio;

use std::future::Future;

#[test]
fn forget_future_initiated() {
  /**
   * Test that forget()'ing an initiated future for the timer is harmless.
   */
  struct NopWaker {}
  impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
  }

  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer = rio::io::Timer::new(ex.clone());
      let timeout = std::time::Duration::from_millis(10);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());
      std::mem::forget(f);

      let mut timer = rio::io::Timer::new(ex.clone());
      let timeout = std::time::Duration::from_millis(20);
      timer.expires_after(timeout);
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
fn forget_timer_initiated() {
  /**
   * Test that forget() is harmless when the associated future is initiated and
   * then forgotten.
   */
  struct NopWaker {}
  impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
  }

  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer = rio::io::Timer::new(ex.clone());
      let timeout = std::time::Duration::from_millis(10);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());
      std::mem::forget(f);
      std::mem::forget(timer);

      let mut timer = rio::io::Timer::new(ex.clone());
      let timeout = std::time::Duration::from_millis(20);
      timer.expires_after(timeout);
      timer.async_wait().await.unwrap();

      unsafe {
        WAS_RUN = true;
      }
    }
  }));

  ioc.run();

  assert!(unsafe { WAS_RUN });
}
