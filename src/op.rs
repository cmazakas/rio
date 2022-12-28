use crate as fiona;

#[derive(Clone)]
pub struct FdState {
  p: std::rc::Rc<std::cell::UnsafeCell<FdStateImpl>>,
}

pub struct FdStateImpl {
  pub initiated: bool,
  pub done: bool,
  pub disarmed: bool,
  pub fd: i32,
  pub res: i32,
  pub task: Option<*mut fiona::Task>,
  pub op: Op,
}

pub enum Op {
  Null,
  Timer(TimerState),
  Timeout(TimeoutState),
  Accept(AcceptState),
  Connect(ConnectState),
  Read(ReadState),
  Write(WriteState),
}

pub struct TimerState {
  pub buf: u64,
}

pub struct TimeoutState {
  pub tspec: fiona::libc::kernel_timespec,
  pub op_fds: Option<FdState>,
}

#[derive(Clone)]
pub struct AcceptState {
  pub addr_in: fiona::ip::tcp::sockaddr_in,
  pub addr_len: u32,
}

pub struct ConnectState {
  pub addr_in: fiona::ip::tcp::sockaddr_in,
  pub timer_fds: Option<FdState>,
}

pub struct ReadState {
  pub buf: Option<Vec<u8>>,
  pub timer_fds: Option<FdState>,
}

pub struct WriteState {
  pub buf: Option<Vec<u8>>,
}

impl FdState {
  #[must_use]
  pub fn new(fd: i32, op: Op) -> Self {
    Self {
      p: std::rc::Rc::new(std::cell::UnsafeCell::new(FdStateImpl {
        initiated: false,
        done: false,
        disarmed: false,
        fd,
        res: -1,
        task: None,
        op,
      })),
    }
  }

  #[must_use]
  pub fn get(&self) -> *mut FdStateImpl {
    self.p.get()
  }

  #[must_use]
  pub fn into_raw(self) -> *mut FdStateImpl {
    std::rc::Rc::into_raw(self.p) as *mut _
  }

  pub unsafe fn from_raw(p: *mut FdStateImpl) -> Self {
    Self {
      p: std::rc::Rc::from_raw(p.cast()),
    }
  }
}

#[derive(Clone)]
pub struct CancelHandle {
  fds: FdState,
  ex: fiona::Executor,
}

impl CancelHandle {
  #[must_use]
  pub fn new(fds: FdState, ex: fiona::Executor) -> Self {
    Self { fds, ex }
  }

  pub fn cancel(self) {
    let p = self.fds.get();
    if unsafe { (*p).disarmed } {
      return;
    }

    let ring = self.ex.get_ring();
    let sqe = unsafe { fiona::liburing::make_sqe(ring) };
    unsafe {
      fiona::liburing::io_uring_prep_cancel(
        sqe,
        p.cast::<fiona::libc::c_void>(),
        0,
      );
    }

    unsafe {
      fiona::liburing::io_uring_submit(ring);
    }
  }

  pub fn disarm(&mut self) {
    let p = self.fds.get();
    unsafe {
      (*p).disarmed = true;
    }
  }
}
