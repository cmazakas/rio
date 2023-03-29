extern crate fiona;

#[test]
#[ignore]
fn timer_stress_test() {
    static mut NUM_RUNS: i32 = 0;
    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    let total_timers = 12_500;

    for _idx in 0..total_timers {
        let ex = ex.clone();
        ioc.post(async move {
            println!("on timer {_idx}");
            let mut timer = fiona::time::Timer::new(&ex);
            timer.expires_after(std::time::Duration::from_millis(2000));
            timer.async_wait().await.unwrap();
            timer.async_wait().await.unwrap();
            timer.async_wait().await.unwrap();
            unsafe {
                NUM_RUNS += 1;
            }
        });
    }

    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, total_timers);
}
