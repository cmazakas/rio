extern crate rio;

use std::future::Future;

struct TimerFuture {
  join_handle: Option<std::thread::JoinHandle<()>>,
}

impl std::future::Future for TimerFuture {
  type Output = ();

  fn poll(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    match &self.join_handle {
      None => {
        let waker = cx.waker().clone();
        self.join_handle = Some(std::thread::spawn(move || {
          std::thread::sleep(std::time::Duration::from_secs(1));
          waker.wake();
        }));

        std::task::Poll::Pending
      }
      Some(handle) => {
        if handle.is_finished() {
          let handle = self.join_handle.take().unwrap();
          handle.join().unwrap();
          std::task::Poll::Ready(())
        } else {
          std::task::Poll::Pending
        }
      }
    }
  }
}

#[test]
#[ignore]
fn foreign_timer_future() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  ioc.post({
    Box::pin(async move {
      TimerFuture { join_handle: None }.await;
      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
#[ignore]
fn foreign_multiple_timer_future() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  ioc.post({
    Box::pin(async move {
      let mut f1 = TimerFuture { join_handle: None };
      let mut f2 = TimerFuture { join_handle: None };
      let mut f3 = TimerFuture { join_handle: None };
      let mut f4 = TimerFuture { join_handle: None };

      let waker = rio::WakerFuture {}.await;
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }.is_pending());
      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f2).poll(&mut cx) }.is_pending());
      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f3).poll(&mut cx) }.is_pending());
      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f4).poll(&mut cx) }.is_pending());

      f4.await;
      f2.await;
      f1.await;
      f3.await;

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.post({
    Box::pin(async move {
      let mut f1 = TimerFuture { join_handle: None };
      let mut f2 = TimerFuture { join_handle: None };
      let mut f3 = TimerFuture { join_handle: None };
      let mut f4 = TimerFuture { join_handle: None };

      let waker = rio::WakerFuture {}.await;
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }.is_pending());
      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f2).poll(&mut cx) }.is_pending());
      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f3).poll(&mut cx) }.is_pending());
      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f4).poll(&mut cx) }.is_pending());

      f1.await;
      f2.await;
      f3.await;
      f4.await;

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
#[ignore]
fn mixed_futures() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  ioc.post({
    let ex = ioc.get_executor();
    Box::pin(async move {
      let mut timer = rio::io::Timer::new(ex);
      timer.expires_after(std::time::Duration::from_millis(500));
      timer.async_wait().await.unwrap();

      TimerFuture { join_handle: None }.await;

      timer.async_wait().await.unwrap();
      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
#[ignore]
fn forget() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  ioc.post({
    Box::pin(async move {
      let mut f1 = TimerFuture { join_handle: None };

      let waker = rio::WakerFuture {}.await;
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }.is_pending());

      std::mem::forget(f1);
      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
#[ignore]
fn drop() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();
  ioc.post({
    Box::pin(async move {
      let mut f1 = TimerFuture { join_handle: None };

      let waker = rio::WakerFuture {}.await;
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }.is_pending());

      std::mem::drop(f1);
      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 1);
}
