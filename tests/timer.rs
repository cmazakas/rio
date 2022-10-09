extern crate rio;

#[test]
fn verify_duration() {
  static mut WAS_RUN: bool = false;
  let mut ioc = rio::IoContext::new();
  ioc.post(Box::new({
    let ioc = ioc.clone();
    async move {
      let mut timer = rio::io::Timer::new(ioc);

      let timeout = 500;
      timer.expires_after(timeout);

      let t1 = std::time::Instant::now();
      timer.async_wait().await.unwrap();
      let t2 = std::time::Instant::now();

      let dur = t2.duration_since(t1);
      assert!(
        (dur.as_millis() >= timeout as u128) && (dur.as_millis() < (timeout as u128 + 50)),
        "{} >= {}",
        dur.as_millis(),
        timeout as u128
      );

      let timeout = 1500;
      timer.expires_after(timeout);

      let t1 = std::time::Instant::now();
      timer.async_wait().await.unwrap();
      let t2 = std::time::Instant::now();

      let dur = t2.duration_since(t1);
      assert!(
        (dur.as_millis() >= timeout as u128) && (dur.as_millis() < (timeout as u128 + 50)),
        "{} >= {}",
        dur.as_millis(),
        timeout as u128
      );

      unsafe {
        WAS_RUN = true;
      }
    }
  }));
  ioc.run();

  assert!(unsafe { WAS_RUN });
}
