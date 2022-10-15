use crate::{self as rio};

pub struct TimerFuture {
  initiated: bool,
  ioc: rio::IoContext,
  state: std::rc::Rc<std::cell::UnsafeCell<rio::FdFutureSharedState>>,
  buf: std::pin::Pin<Box<u64>>,
  millis: i32,
}

impl TimerFuture {
  fn new(
    ioc: rio::IoContext,
    state: std::rc::Rc<std::cell::UnsafeCell<rio::FdFutureSharedState>>,
    millis: i32,
  ) -> Self {
    Self {
      initiated: false,
      state,
      ioc,
      buf: Box::pin(0_u64),
      millis,
    }
  }
}

pub enum Err {
  ReadFailed,
}

impl std::fmt::Debug for Err {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ReadFailed => f.write_str("Read did not complete successfully"),
    }
  }
}

impl std::future::Future for TimerFuture {
  type Output = Result<(), Err>;

  fn poll(
    mut self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = unsafe { (*std::rc::Rc::as_ptr(&(*self).state)).get() };
    if self.initiated {
      let done = unsafe { (*p).done };
      if !done {
        return std::task::Poll::Pending;
      }

      if unsafe { (*p).res < 0 } {
        return std::task::Poll::Ready(Err(Err::ReadFailed));
      }

      return std::task::Poll::Ready(Ok(()));
    }

    assert!(
      unsafe { rio::liburing::timerfd_settime((*p).fd, self.millis) }.is_ok(),
      "Failed to set timer on FD"
    );

    let fd = unsafe { (*p).fd };
    let ioc_state = unsafe { &mut *self.ioc.get_state() };
    unsafe { (*p).task = Some(ioc_state.task_ctx.unwrap()) };

    let ring = ioc_state.ring;
    let sqe = unsafe { rio::liburing::make_sqe(ring) };
    let buf = std::ptr::addr_of_mut!(*self.buf).cast::<rio::libc::c_void>();

    unsafe {
      rio::liburing::io_uring_sqe_set_data(
        sqe,
        std::rc::Rc::into_raw(self.state.clone()).cast::<rio::libc::c_void>()
          as *mut rio::libc::c_void,
      );
    }

    unsafe { rio::liburing::io_uring_prep_read(sqe, fd, buf, 8, 0) };
    unsafe { rio::liburing::io_uring_submit(ring) };

    self.initiated = true;

    std::task::Poll::Pending
  }
}

pub struct Timer {
  fd: i32,
  millis: i32,
  ioc: rio::IoContext,
}

impl Timer {
  #[must_use]
  pub fn new(ioc: rio::IoContext) -> Self {
    let fd = rio::liburing::timerfd_create();
    assert!(fd != -1, "Can't create a timer");

    Self { fd, millis: 0, ioc }
  }

  pub fn expires_after(&mut self, millis: i32) {
    self.millis = millis;
  }

  pub fn async_wait(&mut self) -> impl std::future::Future<Output = Result<(), Err>> + '_ {
    let fd = self.fd;

    let shared_statep = std::rc::Rc::new(std::cell::UnsafeCell::new(rio::FdFutureSharedState {
      done: false,
      fd,
      res: -1,
      task: None,
    }));

    TimerFuture::new(self.ioc.clone(), shared_statep, self.millis)
  }
}

impl Drop for Timer {
  fn drop(&mut self) {
    unsafe { rio::libc::close(self.fd) };
  }
}
