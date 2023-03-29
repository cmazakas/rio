extern crate libc;

use crate::{self as fiona, op::CancelHandle};

pub struct TimerFuture<'a> {
    ex: fiona::Executor,
    fds: fiona::op::FdState,
    _m: std::marker::PhantomData<&'a mut Timer>,
}

impl<'a> TimerFuture<'a> {
    fn new(
        ex: fiona::Executor,
        fds: fiona::op::FdState,
        m: std::marker::PhantomData<&'a mut Timer>,
    ) -> Self {
        Self { fds, ex, _m: m }
    }

    #[must_use]
    pub fn get_cancel_handle(&self) -> CancelHandle {
        CancelHandle::new(self.fds.clone(), self.ex.clone())
    }
}

impl<'a> Drop for TimerFuture<'a> {
    fn drop(&mut self) {
        let p = self.fds.get();

        if unsafe { (*p).initiated && !(*p).done } {
            let ring = self.ex.get_ring();
            unsafe {
                let sqe = fiona::liburing::io_uring_get_sqe(ring);
                fiona::liburing::io_uring_prep_cancel(
                    sqe,
                    p.cast::<libc::c_void>(),
                    0,
                );
                fiona::liburing::io_uring_submit(ring);
            }
        }
    }
}

impl<'a> std::future::Future for TimerFuture<'a> {
    type Output = Result<(), i32>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let p = self.fds.get();
        let fds = unsafe { &mut *p };
        if fds.initiated {
            if !fds.done {
                return std::task::Poll::Pending;
            }

            if fds.res < 0 {
                match -fds.res {
                    libc::ETIME => {}
                    err => return std::task::Poll::Ready(Err(err)),
                }
            }

            return std::task::Poll::Ready(Ok(()));
        }

        let ioc_state = unsafe { &mut *self.ex.get_state() };
        fds.task = Some(ioc_state.task_ctx.unwrap());

        let ts = match fds.op {
            fiona::op::Op::Timeout(ref mut ts) => {
                std::ptr::addr_of_mut!(ts.tspec)
            }
            _ => panic!("invalid op type in TimerFuture"),
        };

        let ring = ioc_state.ring;
        let sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };

        let user_data = self.fds.clone().into_raw().cast::<libc::c_void>();

        unsafe {
            fiona::liburing::io_uring_sqe_set_data(sqe, user_data);
        }

        unsafe { fiona::liburing::io_uring_prep_timeout(sqe, ts, 0, 0) };
        unsafe { fiona::liburing::io_uring_submit(ring) };

        fds.initiated = true;
        std::task::Poll::Pending
    }
}

pub struct Timer {
    dur: std::time::Duration,
    ex: fiona::Executor,
}

impl Timer {
    #[must_use]
    pub fn new(ex: &fiona::Executor) -> Self {
        Self {
            dur: std::time::Duration::default(),
            ex: ex.clone(),
        }
    }

    pub fn expires_after(&mut self, dur: std::time::Duration) {
        self.dur = dur;
    }

    pub fn async_wait(&mut self) -> TimerFuture {
        let fds = fiona::op::FdState::new(
            -1,
            fiona::op::Op::Timeout(fiona::op::TimeoutState {
                op_fds: None,
                tspec: fiona::libc::kernel_timespec {
                    tv_sec: i64::try_from(self.dur.as_secs()).unwrap(),
                    tv_nsec: i64::try_from(self.dur.subsec_nanos()).unwrap(),
                },
            }),
        );

        TimerFuture::new(self.ex.clone(), fds, std::marker::PhantomData)
    }
}
