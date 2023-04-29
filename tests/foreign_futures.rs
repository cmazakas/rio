extern crate fiona;

use std::future::Future;

extern "C" {
    fn rand_r(seedp: *mut u32) -> i32;
}

struct TimerFuture {
    dur: u64,
    rx: Option<std::sync::mpsc::Receiver<i32>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl TimerFuture {
    fn with_dur(mut dur: u64) -> Self {
        if dur == 0 {
            dur += 1;
        }

        Self {
            dur,
            rx: None,
            join_handle: None,
        }
    }
}

impl Drop for TimerFuture {
    fn drop(&mut self) {
        match &mut self.join_handle {
            None => {}
            Some(_) => {
                self.join_handle.take().unwrap().join().unwrap();
            }
        }
    }
}

impl std::future::Future for TimerFuture {
    type Output = i32;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        match self.join_handle {
            None => {
                let waker = cx.waker().clone();
                let dur = 1 + self.dur;

                let (tx, rx) = std::sync::mpsc::channel();

                self.join_handle = Some(std::thread::spawn(move || {
                    println!("dur:{dur}");
                    std::thread::sleep(std::time::Duration::from_secs(dur));
                    tx.send(dur as i32).unwrap();
                    waker.wake();
                }));

                self.rx = Some(rx);

                std::task::Poll::Pending
            }
            Some(ref handle) => {
                if handle.is_finished() {
                    let handle = self.join_handle.take().unwrap();
                    handle.join().unwrap();

                    let x = self.rx.take().unwrap().recv().unwrap();
                    std::task::Poll::Ready(x)
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

    let mut ioc = fiona::IoContext::new();
    ioc.post(async move {
        let mut seed = 0_u32;
        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let n = TimerFuture::with_dur(dur).await;
        assert!(n > 0);
        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
#[ignore]
fn foreign_multiple_timer_future() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    ioc.post(async move {
        let mut seed = 0_u32;
        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f1 = TimerFuture::with_dur(dur);

        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f2 = TimerFuture::with_dur(dur);

        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f3 = TimerFuture::with_dur(dur);

        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f4 = TimerFuture::with_dur(dur);

        let waker = fiona::WakerFuture {}.await;
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
    });

    ioc.post(async move {
        let mut seed = 0_u32;
        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f1 = TimerFuture::with_dur(dur);

        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f2 = TimerFuture::with_dur(dur);

        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f3 = TimerFuture::with_dur(dur);

        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f4 = TimerFuture::with_dur(dur);

        let waker = fiona::WakerFuture {}.await;
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
    });

    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
#[ignore]
fn mixed_futures() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();
    ioc.post(async move {
        let mut timer = fiona::time::Timer::new(&ex);
        timer.expires_after(std::time::Duration::from_millis(500));
        timer.async_wait().await.unwrap();

        let mut seed = 0_u32;
        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();

        TimerFuture::with_dur(dur).await;

        timer.async_wait().await.unwrap();
        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, 1);
}

// #[test]
// #[ignore]
// fn forget() {
//   static mut NUM_RUNS: i32 = 0;

//   let mut ioc = fiona::IoContext::new();
//   ioc.post({
//     Box::pin(async move {
//       let mut f1 = TimerFuture { join_handle: None };

//       let waker = fiona::WakerFuture {}.await;
//       let mut cx = std::task::Context::from_waker(&waker);

//       assert!(
//         unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }
//           .is_pending()
//       );

//       std::mem::forget(f1);
//       unsafe { NUM_RUNS += 1 };
//     })
//   });

//   ioc.run();

//   assert_eq!(unsafe { NUM_RUNS }, 1);
// }

#[test]
#[ignore]
fn drop() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    ioc.post(async move {
        let mut seed = 0_u32;
        let dur = u64::try_from(unsafe { rand_r(&mut seed) } % 4).unwrap();
        let mut f1 = TimerFuture::with_dur(dur);

        let waker = fiona::WakerFuture {}.await;
        let mut cx = std::task::Context::from_waker(&waker);

        assert!(unsafe { std::pin::Pin::new_unchecked(&mut f1).poll(&mut cx) }.is_pending());

        std::mem::drop(f1);
        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, 1);
}
