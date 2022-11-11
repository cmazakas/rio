extern crate rio;

#[test]
fn executor_post_ioc_running() {
  static mut WAS_RUN: bool = false;

  let mut ioc = rio::IoContext::new();
  ioc.post({
    let mut ex = ioc.get_executor();
    Box::pin(async move {
      ex.post({
        let ex = ex.clone();
        Box::pin(async move {
          let mut timer = rio::time::Timer::new(ex);
          let dur = std::time::Duration::from_millis(500);
          let t = std::time::Instant::now();
          timer.expires_after(dur);

          timer.async_wait().await.unwrap();
          timer.async_wait().await.unwrap();

          assert!(std::time::Instant::now().duration_since(t).as_millis() >= 2 * 500);

          unsafe { WAS_RUN = true };
        })
      });

      let mut timer = rio::time::Timer::new(ex);
      let dur = std::time::Duration::from_millis(500);
      let t = std::time::Instant::now();
      timer.expires_after(dur);
      timer.async_wait().await.unwrap();

      assert!(std::time::Instant::now().duration_since(t).as_millis() >= 500);
    })
  });

  ioc.run();
  assert!(unsafe { WAS_RUN });
}

#[test]
fn executor_post_ioc_not_running() {
  static mut NUM_RUNS: i32 = 0;

  let mut ioc = rio::IoContext::new();

  let mut ex = ioc.get_executor();
  ex.post(Box::pin(async {
    unsafe { NUM_RUNS += 1 };
  }));

  ex.post({
    let ex = ex.clone();
    Box::pin(async move {
      let mut timer = rio::time::Timer::new(ex);
      let dur = std::time::Duration::from_millis(500);
      let t = std::time::Instant::now();
      timer.expires_after(dur);

      timer.async_wait().await.unwrap();
      timer.async_wait().await.unwrap();

      assert!(std::time::Instant::now().duration_since(t).as_millis() >= 2 * 500);

      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.post({
    Box::pin(async move {
      let mut timer = rio::time::Timer::new(ex);
      let dur = std::time::Duration::from_millis(500);
      let t = std::time::Instant::now();
      timer.expires_after(dur);
      timer.async_wait().await.unwrap();

      assert!(std::time::Instant::now().duration_since(t).as_millis() >= 500);
      unsafe { NUM_RUNS += 1 };
    })
  });

  ioc.run();
  assert_eq!(unsafe { NUM_RUNS }, 3);
}
