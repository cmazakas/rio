#[test]
#[ignore]
#[should_panic]
fn cross_io_contexts() {
  /***
   * This test thankfully panics because the task_ctx for ex2 isn't set when ioc1.run() begins.
   *
   * In in the ideal case, we keep it this way so as to protect the user from themselves.
   *
   * TODO: find out why we leak 2 FDs from `io_uring_queue_init` here
   */
  let mut ioc1 = fiona::IoContext::new();
  let mut ioc2 = fiona::IoContext::new();

  let ex1 = ioc1.get_executor();
  let ex2 = ioc2.get_executor();

  ioc1.post(async move {
    let mut timer = fiona::time::Timer::new(&ex2);
    timer.expires_after(std::time::Duration::from_millis(250));
    timer.async_wait().await.unwrap();
  });

  ioc2.post(async move {
    let mut timer = fiona::time::Timer::new(&ex1);
    timer.expires_after(std::time::Duration::from_millis(250));
    timer.async_wait().await.unwrap();
  });

  ioc1.post({
    let ex = ioc1.get_executor();
    async move {
      let mut timer = fiona::time::Timer::new(&ex);
      timer.expires_after(std::time::Duration::from_millis(250));
      timer.async_wait().await.unwrap();
    }
  });

  ioc2.post({
    let ex = ioc2.get_executor();
    async move {
      let mut timer = fiona::time::Timer::new(&ex);
      timer.expires_after(std::time::Duration::from_millis(250));
      timer.async_wait().await.unwrap();
    }
  });

  ioc1.run();
  ioc2.run();
}
