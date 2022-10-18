use std::future::Future;

extern crate rio;

#[test]
fn verify_duration() {
  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      let mut timer = rio::io::Timer::new(ioc);

      let timeout = std::time::Duration::from_millis(500);
      timer.expires_after(timeout);

      let t1 = std::time::Instant::now();
      timer.async_wait().await.unwrap();
      let t2 = std::time::Instant::now();

      let dur = t2.duration_since(t1);
      assert!(dur >= timeout);
      assert!(dur < timeout + std::time::Duration::from_millis(25));

      let timeout = std::time::Duration::from_millis(1500);
      timer.expires_after(timeout);

      let t1 = std::time::Instant::now();
      timer.async_wait().await.unwrap();
      let t2 = std::time::Instant::now();

      let dur = t2.duration_since(t1);
      assert!(dur >= timeout);
      assert!(dur < timeout + std::time::Duration::from_millis(25));

      unsafe {
        WAS_RUN = true;
      }
    }
  }));
  ioc.run();

  assert!(unsafe { WAS_RUN });
}

#[test]
fn drop_future_initiated() {
  /**
   * Test that dropping a future and then creating a new I/O object (likely
   * reusing the same FD) is harmless.
   */
  struct NopWaker {}
  impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
  }

  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      let mut timer = rio::io::Timer::new(ioc.clone());
      let timeout = std::time::Duration::from_millis(10);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());
      std::mem::drop(f);

      let mut timer2 = rio::io::Timer::new(ioc.clone());
      let timeout = std::time::Duration::from_millis(20);
      timer2.expires_after(timeout);
      timer2.async_wait().await.unwrap();

      unsafe {
        WAS_RUN = true;
      }
    }
  }));

  ioc.run();

  assert!(unsafe { WAS_RUN });
}

#[test]
fn drop_timer_initiated() {
  /**
   * We want to test the case of scheduling 1 I/O op and then reuse that FD to
   * schedule another I/O op, using the same parent task.
   * Ideally we'd also get this test working in the case of 2 separate tasks so
   * we test when 2 CQEs come in that have the same FD but different associated
   * tasks to resume but this is largely good enough.
   */
  struct NopWaker {}
  impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
  }

  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      let mut timer = rio::io::Timer::new(ioc.clone());
      let timeout = std::time::Duration::from_millis(10);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());

      // by dropping and then creating a new timer object, we guarantee the FD
      // is reused
      //
      std::mem::drop(f);
      std::mem::drop(timer);

      let mut timer2 = rio::io::Timer::new(ioc.clone());
      let timeout = std::time::Duration::from_millis(20);
      timer2.expires_after(timeout);
      timer2.async_wait().await.unwrap();

      unsafe {
        WAS_RUN = true;
      }
    }
  }));

  ioc.run();

  assert!(unsafe { WAS_RUN });
}

#[test]
fn drop_timer_finish_early() {
  /**
   * We want to test the case where a CQE comes in but the task associated it
   * with it has already complete, meaning that it's no longer in the task list
   */
  struct NopWaker {}
  impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
  }

  {
    static mut NUM_RUNS: i32 = 0;
    let mut ioc = rio::IoContext::new();
    ioc.post(Box::new({
      let ioc = ioc.clone();
      async move {
        let mut timer = rio::io::Timer::new(ioc.clone());
        let timeout = std::time::Duration::from_millis(100);
        timer.expires_after(timeout);

        let mut f = timer.async_wait();

        let waker = std::sync::Arc::new(NopWaker {}).into();
        let mut cx = std::task::Context::from_waker(&waker);

        assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());

        unsafe {
          NUM_RUNS += 1;
        }
      }
    }));

    ioc.post(Box::new({
      let ioc = ioc.clone();
      async move {
        let timeout = std::time::Duration::from_millis(250);

        let mut timer = rio::io::Timer::new(ioc.clone());
        timer.expires_after(timeout);
        timer.async_wait().await.unwrap();

        unsafe {
          NUM_RUNS += 1;
        }
      }
    }));

    ioc.run();
    assert_eq!(unsafe { NUM_RUNS }, 2);
  }
}

#[test]
fn double_wait() {
  /**
   * Want to test that our time structure is stable when we poll a future, drop
   * it and then start up another async read on the timer's fd
   */
  struct NopWaker {}
  impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
  }

  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      let mut timer = rio::io::Timer::new(ioc.clone());
      let timeout = std::time::Duration::from_secs(1);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());

      std::mem::drop(f);

      let t1 = std::time::Instant::now();
      timer.async_wait().await.unwrap();
      let t2 = std::time::Instant::now();

      let dur = t2.duration_since(t1);

      assert!(dur >= timeout);
      assert!(dur < timeout + std::time::Duration::from_millis(10));

      unsafe {
        WAS_RUN = true;
      }
    }
  }));

  ioc.run();

  assert!(unsafe { WAS_RUN });
}
