use std::future::Future;

extern crate fiona;

#[test]
fn eager_future() {
  static mut WAS_RUN: bool = false;
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin(async {
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
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer = fiona::time::Timer::new(ex);
      timer.expires_after(std::time::Duration::from_millis(500));
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
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      // make sure we can wait twice in a row
      //
      let mut timer = fiona::time::Timer::new(ex);
      timer.expires_after(std::time::Duration::from_millis(500));
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
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      // make sure we can wait twice in a row
      //
      let mut timer1 = fiona::time::Timer::new(ex.clone());
      let mut timer2 = fiona::time::Timer::new(ex.clone());
      timer1.expires_after(std::time::Duration::from_millis(500));
      timer2.expires_after(std::time::Duration::from_millis(750));
      let t = std::time::Instant::now();
      timer2.async_wait().await.unwrap();

      assert!(std::time::Instant::now().duration_since(t).as_millis() >= 750);
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
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer1 = fiona::time::Timer::new(ex.clone());
      let mut timer2 = fiona::time::Timer::new(ex.clone());
      let mut timer3 = fiona::time::Timer::new(ex.clone());

      timer1.expires_after(std::time::Duration::from_millis(1000));
      timer2.expires_after(std::time::Duration::from_millis(2000));
      timer3.expires_after(std::time::Duration::from_millis(4000));

      let mut f1 = timer1.async_wait();
      let mut f2 = timer2.async_wait();
      let mut f3 = timer3.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);
      assert!(
        unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }
          .is_pending()
      );
      assert!(
        unsafe { std::pin::Pin::new_unchecked(&mut f2).poll(&mut cx) }
          .is_pending()
      );
      assert!(
        unsafe { std::pin::Pin::new_unchecked(&mut f3).poll(&mut cx) }
          .is_pending()
      );

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

#[test]
fn timer_multiple_tasks() {
  const TOTAL_RUNS: i32 = 12;
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = fiona::IoContext::new();
  for _idx in 0..TOTAL_RUNS {
    ioc.post(Box::pin({
      let ex = ioc.get_executor();
      async move {
        let mut timer = fiona::time::Timer::new(ex);
        timer.expires_after(std::time::Duration::from_millis(500));
        timer.async_wait().await.unwrap();
        timer.async_wait().await.unwrap();
        unsafe {
          NUM_RUNS += 1;
        }
      }
    }));
  }

  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, TOTAL_RUNS);
}

#[test]
fn verify_duration() {
  static mut WAS_RUN: bool = false;
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer = fiona::time::Timer::new(ex);

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
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer = fiona::time::Timer::new(ex.clone());
      let timeout = std::time::Duration::from_millis(10);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(
        unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }
          .is_pending()
      );
      std::mem::drop(f);

      let mut timer2 = fiona::time::Timer::new(ex.clone());
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
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer = fiona::time::Timer::new(ex.clone());
      let timeout = std::time::Duration::from_millis(10);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(
        unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }
          .is_pending()
      );

      // by dropping and then creating a new timer object, we guarantee the FD
      // is reused
      //
      std::mem::drop(f);
      std::mem::drop(timer);

      let mut timer2 = fiona::time::Timer::new(ex.clone());
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
    let mut ioc = fiona::IoContext::new();
    ioc.post(Box::pin({
      let ex = ioc.get_executor();
      async move {
        let mut timer = fiona::time::Timer::new(ex.clone());
        let timeout = std::time::Duration::from_millis(100);
        timer.expires_after(timeout);

        let mut f = timer.async_wait();

        let waker = std::sync::Arc::new(NopWaker {}).into();
        let mut cx = std::task::Context::from_waker(&waker);

        assert!(
          unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }
            .is_pending()
        );

        unsafe {
          NUM_RUNS += 1;
        }
      }
    }));

    ioc.post(Box::pin({
      let ex = ioc.get_executor();
      async move {
        let timeout = std::time::Duration::from_millis(250);

        let mut timer = fiona::time::Timer::new(ex.clone());
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
  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin({
    let ex = ioc.get_executor();
    async move {
      let mut timer = fiona::time::Timer::new(ex.clone());
      let timeout = std::time::Duration::from_secs(1);
      timer.expires_after(timeout);

      let mut f = timer.async_wait();

      let waker = std::sync::Arc::new(NopWaker {}).into();
      let mut cx = std::task::Context::from_waker(&waker);

      assert!(
        unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }
          .is_pending()
      );

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

#[test]
fn nested_ioc() {
  static mut WAS_RUN: bool = false;

  let mut ioc = fiona::IoContext::new();
  ioc.post(Box::pin(async {
    let mut ioc2 = fiona::IoContext::new();
    let ex2 = ioc2.get_executor();
    ioc2.post(Box::pin(async move {
      let mut timer = fiona::time::Timer::new(ex2);
      timer.expires_after(std::time::Duration::from_millis(100));
      timer.async_wait().await.unwrap();
    }));
    ioc2.run();

    unsafe {
      WAS_RUN = true;
    }
  }));

  ioc.run();
  assert!(unsafe { WAS_RUN });
}

#[test]
fn nested_future() {
  static mut WAS_RUN: bool = false;

  let mut ioc = fiona::IoContext::new();
  ioc.post({
    let ex = ioc.get_executor();
    Box::pin(async {
      let nested = async {
        let mut timer = fiona::time::Timer::new(ex);
        let dur = std::time::Duration::from_millis(500);
        let t = std::time::Instant::now();
        timer.expires_after(dur);
        timer.async_wait().await.unwrap();
        assert!(std::time::Instant::now().duration_since(t).as_millis() >= 500);
        unsafe { WAS_RUN = true };
      };

      nested.await;
    })
  });

  ioc.run();
  assert!(unsafe { WAS_RUN });
}

#[test]
fn cancellation() {
  static mut WAS_RUN: bool = false;

  let mut ioc = fiona::IoContext::new();
  ioc.post({
    let mut ex = ioc.get_executor();
    Box::pin(async move {
      let mut timer = fiona::time::Timer::new(ex.clone());
      timer.expires_after(std::time::Duration::from_secs(30));
      let f = timer.async_wait();
      let c = f.get_cancel_handle();

      ex.post(Box::pin({
        let ex = ex.clone();
        async move {
          let mut timer2 = fiona::time::Timer::new(ex);
          timer2.expires_after(std::time::Duration::from_millis(250));
          timer2.async_wait().await.unwrap();
          c.cancel();
        }
      }));

      let result = f.await;
      match result.expect_err("Operation didn't report cancellation properly!")
      {
        fiona::libc::Errno::ECANCELED => {}
        _ => panic!("Incorrect error type returned, should be ECANCELED"),
      }

      unsafe { WAS_RUN = true };
    })
  });

  ioc.run();
  assert!(unsafe { WAS_RUN });
}

#[test]
fn cancellation_with_drop() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = fiona::IoContext::new();
  ioc.post({
    let mut ex = ioc.get_executor();
    Box::pin(async move {
      let mut timer = fiona::time::Timer::new(ex.clone());
      timer.expires_after(std::time::Duration::from_secs(30));
      let f = timer.async_wait();
      let c = f.get_cancel_handle();

      ex.post(Box::pin({
        async move {
          c.cancel();
          unsafe { NUM_RUNS += 1 };
        }
      }));

      drop(f);

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.post({
    let ex = ioc.get_executor();
    Box::pin(async move {
      let mut timer = fiona::time::Timer::new(ex);
      timer.expires_after(std::time::Duration::from_millis(250));
      timer.async_wait().await.unwrap();
      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 3);
}

#[test]
fn cancellation_post_expiration() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = fiona::IoContext::new();
  ioc.post({
    let ex = ioc.get_executor();
    Box::pin(async move {
      let mut timer = fiona::time::Timer::new(ex);
      timer.expires_after(std::time::Duration::from_millis(30));
      let f = timer.async_wait();
      let c = f.get_cancel_handle();

      f.await.unwrap();
      c.cancel();

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.post({
    let ex = ioc.get_executor();
    Box::pin(async move {
      let mut timer = fiona::time::Timer::new(ex);
      timer.expires_after(std::time::Duration::from_millis(250));
      timer.async_wait().await.unwrap();

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn cancellation_disarming() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = fiona::IoContext::new();
  ioc.post({
    let mut ex = ioc.get_executor();
    Box::pin(async move {
      let mut timer = fiona::time::Timer::new(ex.clone());
      timer.expires_after(std::time::Duration::from_millis(30));
      let f = timer.async_wait();
      let mut c = f.get_cancel_handle();

      ex.post({
        let ex = ex.clone();
        let c = c.clone();
        Box::pin(async move {
          let mut timer = fiona::time::Timer::new(ex);
          timer.expires_after(std::time::Duration::from_secs(1));
          timer.async_wait().await.unwrap();
          c.cancel();
          assert_eq!(unsafe { NUM_RUNS }, 1);
          unsafe { NUM_RUNS += 1 };
        })
      });

      f.await.unwrap();
      c.disarm();

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 2);
}
