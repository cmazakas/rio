use crate::{self as rio, libc};

pub struct TimerFuture<'a> {
  ex: rio::Executor,
  fds: rio::op::FdState,
  dur: std::time::Duration,
  _m: std::marker::PhantomData<&'a mut Timer>,
}

#[derive(Clone)]
pub struct TimerCancel {
  ex: rio::Executor,
  fds: rio::op::FdState,
}

impl<'a> TimerFuture<'a> {
  fn new(
    ex: rio::Executor,
    fds: rio::op::FdState,
    dur: std::time::Duration,
    m: std::marker::PhantomData<&'a mut Timer>,
  ) -> Self {
    Self {
      fds,
      ex,
      dur,
      _m: m,
    }
  }

  #[must_use]
  pub fn get_cancel_handle(&self) -> TimerCancel {
    TimerCancel {
      ex: self.ex.clone(),
      fds: self.fds.clone(),
    }
  }
}

impl<'a> Drop for TimerFuture<'a> {
  fn drop(&mut self) {
    let p = self.fds.get();
    if unsafe { (*p).initiated && !(*p).done } {
      unsafe {
        let ring = (*self.ex.get_state()).ring;
        let sqe = rio::liburing::make_sqe(ring);
        rio::liburing::io_uring_prep_cancel(sqe, p.cast::<libc::c_void>(), 0);
        rio::liburing::io_uring_submit((*self.ex.get_state()).ring);
      }
    }
  }
}

impl TimerCancel {
  pub fn cancel(self) {
    unsafe {
      let p = self.fds.get();
      if (*p).disarmed {
        return;
      }

      let ring = (*self.ex.get_state()).ring;
      let sqe = rio::liburing::make_sqe(ring);
      rio::liburing::io_uring_prep_cancel(sqe, p.cast::<libc::c_void>(), 0);
      rio::liburing::io_uring_submit((*self.ex.get_state()).ring);
    }
  }

  pub fn disarm(&mut self) {
    unsafe {
      let p = self.fds.get();
      (*p).disarmed = true;
    };
  }
}

impl<'a> std::future::Future for TimerFuture<'a> {
  type Output = Result<(), libc::Errno>;

  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.fds.get();
    if unsafe { (*p).initiated } {
      let done = unsafe { (*p).done };
      if !done {
        return std::task::Poll::Pending;
      }

      if unsafe { (*p).res < 0 } {
        let e = unsafe { -(*p).res };
        return std::task::Poll::Ready(Err(libc::errno(e)));
      }

      let exp = match unsafe { &(*p).op } {
        rio::op::Op::Timer(ref ts) => ts.buf,
        _ => panic!("The number of expirations for a TimerFuture is strictly one as the buffer is allocated in tandem with the task metadata"),
      };
      assert_eq!(exp, 1);

      return std::task::Poll::Ready(Ok(()));
    }

    assert!(
      unsafe {
        rio::liburing::timerfd_settime(
          (*p).fd,
          self.dur.as_secs() as u64,
          u64::from(self.dur.subsec_nanos()),
        )
      }
      .is_ok(),
      "Failed to set timer on FD"
    );

    let fd = unsafe { (*p).fd };
    let ioc_state = unsafe { &mut *self.ex.get_state() };
    unsafe { (*p).task = Some(ioc_state.task_ctx.unwrap()) };

    let ring = ioc_state.ring;
    let sqe = unsafe { rio::liburing::make_sqe(ring) };
    let buf = match unsafe { &mut (*p).op } {
      rio::op::Op::Timer(ref mut ts) => {
        std::ptr::addr_of_mut!(ts.buf).cast::<rio::libc::c_void>()
      }
      _ => panic!("invalid op type in TimerFuture"),
    };

    let user_data = self.fds.clone().into_raw().cast::<rio::libc::c_void>();

    unsafe {
      rio::liburing::io_uring_sqe_set_data(sqe, user_data);
    }

    unsafe { rio::liburing::io_uring_prep_read(sqe, fd, buf, 8, 0) };
    unsafe { rio::liburing::io_uring_submit(ring) };

    unsafe { (*p).initiated = true };
    std::task::Poll::Pending
  }
}

pub struct Timer {
  fd: i32,
  dur: std::time::Duration,
  ex: rio::Executor,
}

impl Timer {
  #[must_use]
  pub fn new(ex: rio::Executor) -> Self {
    let fd = rio::liburing::timerfd_create();
    assert!(fd != -1, "Can't create a timer");

    Self {
      fd,
      dur: std::time::Duration::default(),
      ex,
    }
  }

  pub fn expires_after(&mut self, dur: std::time::Duration) {
    self.dur = dur;
  }

  pub fn async_wait(&mut self) -> TimerFuture {
    let fd = self.fd;

    let fds = rio::op::FdState::new(
      fd,
      rio::op::Op::Timer(rio::op::TimerState { buf: 0 }),
    );
    TimerFuture::new(self.ex.clone(), fds, self.dur, std::marker::PhantomData)
  }
}

impl Drop for Timer {
  fn drop(&mut self) {
    unsafe { rio::libc::close(self.fd) };
  }
}
