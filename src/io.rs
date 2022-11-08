use crate::{self as rio, libc};

pub struct TimerFuture<'a> {
  initiated: bool,
  ex: rio::Executor,
  state: std::rc::Rc<std::cell::UnsafeCell<rio::FdFutureSharedState>>,
  buf: std::pin::Pin<Box<u64>>,
  dur: std::time::Duration,
  _m: std::marker::PhantomData<&'a mut Timer>,
}

#[derive(Clone)]
pub struct TimerCancel {
  ex: rio::Executor,
  state: std::rc::Rc<std::cell::UnsafeCell<rio::FdFutureSharedState>>,
}

impl<'a> TimerFuture<'a> {
  fn new(
    ex: rio::Executor,
    state: std::rc::Rc<std::cell::UnsafeCell<rio::FdFutureSharedState>>,
    dur: std::time::Duration,
    m: std::marker::PhantomData<&'a mut Timer>,
  ) -> Self {
    Self {
      initiated: false,
      state,
      ex,
      buf: Box::pin(0_u64),
      dur,
      _m: m,
    }
  }

  #[must_use]
  pub fn get_cancel_handle(&self) -> TimerCancel {
    TimerCancel {
      ex: self.ex.clone(),
      state: self.state.clone(),
    }
  }
}

impl<'a> Drop for TimerFuture<'a> {
  fn drop(&mut self) {
    let p = unsafe { (*std::rc::Rc::as_ptr(&self.state)).get() };
    if self.initiated && unsafe { !(*p).done } {
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
      let p = (*std::rc::Rc::as_ptr(&self.state)).get();
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
      let p = (*std::rc::Rc::as_ptr(&self.state)).get();
      (*p).disarmed = true;
    };
  }
}

impl<'a> std::future::Future for TimerFuture<'a> {
  type Output = Result<(), libc::Errno>;

  fn poll(
    mut self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = unsafe { (*std::rc::Rc::as_ptr(&(self.state))).get() };
    if self.initiated {
      let done = unsafe { (*p).done };
      if !done {
        return std::task::Poll::Pending;
      }

      if unsafe { (*p).res < 0 } {
        let e = unsafe { -(*p).res };
        return std::task::Poll::Ready(Err(libc::errno(e)));
      }

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
    let buf = std::ptr::addr_of_mut!(*self.buf).cast::<rio::libc::c_void>();

    let user_data = std::rc::Rc::into_raw(self.state.clone()).cast::<rio::libc::c_void>();

    unsafe {
      rio::liburing::io_uring_sqe_set_data(sqe, user_data as *mut rio::libc::c_void);
    }

    unsafe { rio::liburing::io_uring_prep_read(sqe, fd, buf, 8, 0) };
    unsafe { rio::liburing::io_uring_submit(ring) };

    self.initiated = true;
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

    let shared_statep = std::rc::Rc::new(std::cell::UnsafeCell::new(rio::FdFutureSharedState {
      done: false,
      fd,
      res: -1,
      task: None,
      disarmed: false,
    }));

    TimerFuture::new(
      self.ex.clone(),
      shared_statep,
      self.dur,
      std::marker::PhantomData,
    )
  }
}

impl Drop for Timer {
  fn drop(&mut self) {
    unsafe { rio::libc::close(self.fd) };
  }
}
