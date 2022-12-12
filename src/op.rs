use crate as rio;

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
  pub task: Option<*mut rio::Task>,
  pub op: Op,
}

pub struct FdStateImplImpl {}

pub enum Op {
  Null,
  Timer(TimerState),
  Accept(AcceptState),
  Connect(ConnectState),
  Read(ReadState),
  Write(WriteState),
}

pub struct TimerState {
  pub buf: u64,
}

#[derive(Clone)]
pub struct AcceptState {
  pub addr_in: rio::ip::tcp::sockaddr_in,
  pub addr_len: u32,
}

pub struct ConnectState {
  pub addr_in: rio::ip::tcp::sockaddr_in,
}

pub struct ReadState {
  pub buf: Option<Vec<u8>>,
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
