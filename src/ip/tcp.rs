use crate as fiona;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct sockaddr_in {
  sin_family: u16,
  sin_port: u16,
  sin_addr: in_addr,
  _sin_zero: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct in_addr {
  s_addr: u32,
}

pub struct Acceptor {
  fd: i32,
  ex: fiona::Executor,
}

pub struct Socket {
  fd: i32,
  ex: fiona::Executor,
  pub timeout: std::time::Duration,
}

pub struct AcceptFuture<'a> {
  ex: fiona::Executor,
  fds: fiona::op::FdState,
  _m: std::marker::PhantomData<&'a mut Acceptor>,
}

pub struct ConnectFuture<'a> {
  ex: fiona::Executor,
  connect_fds: fiona::op::FdState,
  timeout: std::time::Duration,
  _m: std::marker::PhantomData<&'a mut Socket>,
}

pub struct ReadFuture<'a> {
  ex: fiona::Executor,
  read_fds: fiona::op::FdState,
  timeout: std::time::Duration,
  _m: std::marker::PhantomData<&'a mut Socket>,
}

pub struct WriteFuture<'a> {
  ex: fiona::Executor,
  write_fds: fiona::op::FdState,
  timeout: std::time::Duration,
  _m: std::marker::PhantomData<&'a mut Socket>,
}

fn make_kernel_timespec(
  t: std::time::Duration,
) -> fiona::libc::kernel_timespec {
  let (tv_sec, tv_nsec) = (
    i64::try_from(t.as_secs()).unwrap(),
    i64::try_from(t.subsec_nanos()).unwrap(),
  );
  fiona::libc::kernel_timespec { tv_sec, tv_nsec }
}

unsafe fn drop_cancel(
  p: *mut fiona::op::FdStateImpl,
  ioc: *mut fiona::IoContextState,
) {
  if (*p).initiated && !(*p).done {
    let ring = (*ioc).ring;
    unsafe {
      let sqe = fiona::liburing::make_sqe(ring);
      fiona::liburing::io_uring_prep_cancel(
        sqe,
        p.cast::<fiona::libc::c_void>(),
        0,
      );
      fiona::liburing::io_uring_submit(ring);
    }
  }
}

impl<'a> Drop for AcceptFuture<'a> {
  fn drop(&mut self) {
    unsafe { drop_cancel(self.fds.get(), self.ex.get_state()) };
  }
}

impl<'a> Drop for ConnectFuture<'a> {
  fn drop(&mut self) {
    unsafe { drop_cancel(self.connect_fds.get(), self.ex.get_state()) };
  }
}

impl<'a> Drop for ReadFuture<'a> {
  fn drop(&mut self) {
    unsafe { drop_cancel(self.read_fds.get(), self.ex.get_state()) };
  }
}

impl<'a> Drop for WriteFuture<'a> {
  fn drop(&mut self) {
    unsafe { drop_cancel(self.write_fds.get(), self.ex.get_state()) };
  }
}

impl<'a> std::future::Future for AcceptFuture<'a> {
  type Output = Result<Socket, fiona::libc::Errno>;
  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.fds.get();

    if !unsafe { (*p).initiated } {
      unsafe { (*p).initiated = true };
      let fd = unsafe { (*p).fd };
      let ioc_state = unsafe { &mut *self.ex.get_state() };
      unsafe { (*p).task = Some(ioc_state.task_ctx.unwrap()) };

      let ring = unsafe { (*self.ex.get_state()).ring };
      let sqe = unsafe { fiona::liburing::make_sqe(ring) };
      let user_data = self.fds.clone().into_raw().cast::<fiona::libc::c_void>();

      let (addr, addrlen) = match unsafe { &mut (*p).op } {
        fiona::op::Op::Accept(ref mut s) => (
          std::ptr::addr_of_mut!(s.addr_in),
          std::ptr::addr_of_mut!(s.addr_len),
        ),
        _ => panic!("incorrect op type specified for the AcceptFuture"),
      };

      unsafe { fiona::liburing::io_uring_sqe_set_data(sqe, user_data) };
      unsafe {
        fiona::liburing::io_uring_prep_accept(sqe, fd, addr, addrlen, 0);
      }
      unsafe { fiona::liburing::io_uring_submit(ring) };
      return std::task::Poll::Pending;
    }

    if !unsafe { (*p).done } {
      return std::task::Poll::Pending;
    }

    let fd = unsafe { (*p).res };
    if fd < 0 {
      std::task::Poll::Ready(Err(fiona::libc::errno(-fd)))
    } else {
      // let addr = match unsafe { &mut (*p).op } {
      //   fiona::op::Op::Accept(ref mut s) => s.addr_in,
      //   _ => panic!("incorrect op type specified for the AcceptFuture"),
      // };
      // println!(
      //   "accepted tcp connection on this addr: {:?}:{}",
      //   addr.sin_addr.s_addr.to_le_bytes(),
      //   addr.sin_port.to_be()
      // );
      std::task::Poll::Ready(Ok(unsafe {
        Socket::from_raw(self.ex.clone(), fd)
      }))
    }
  }
}

impl<'a> std::future::Future for ConnectFuture<'a> {
  type Output = Result<(), fiona::libc::Errno>;
  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.connect_fds.get();
    let connect_fds = unsafe { &mut *p };

    if connect_fds.done {
      match connect_fds.op {
        fiona::op::Op::Connect(ref mut s) => {
          s.timer_fds = None;
        }
        _ => panic!(""),
      }

      if connect_fds.res < 0 {
        return std::task::Poll::Ready(Err(fiona::libc::errno(
          -connect_fds.res,
        )));
      }

      return std::task::Poll::Ready(Ok(()));
    }

    if connect_fds.initiated {
      return std::task::Poll::Pending;
    }

    let ioc_state = unsafe { &mut *self.ex.get_state() };
    connect_fds.task = Some(ioc_state.task_ctx.unwrap());

    let ring = self.ex.get_ring();

    let connect_sqe = unsafe { fiona::liburing::make_sqe(ring) };

    let (addr, addrlen) = match connect_fds.op {
      fiona::op::Op::Connect(ref s) => (
        std::ptr::addr_of!(s.addr_in),
        std::mem::size_of::<fiona::ip::tcp::sockaddr_in>() as u32,
      ),
      _ => panic!(""),
    };

    let user_data = self
      .connect_fds
      .clone()
      .into_raw()
      .cast::<fiona::libc::c_void>();

    unsafe { fiona::liburing::io_uring_sqe_set_data(connect_sqe, user_data) };
    unsafe {
      fiona::liburing::io_uring_prep_connect(
        connect_sqe,
        connect_fds.fd,
        addr.cast::<fiona::libc::sockaddr>(),
        addrlen,
      );
    }

    unsafe { fiona::liburing::io_uring_sqe_set_flags(connect_sqe, 1_u32 << 2) };

    let mut ts = make_kernel_timespec(self.timeout);
    let timeout_sqe = unsafe { fiona::liburing::make_sqe(ring) };
    unsafe {
      fiona::liburing::io_uring_prep_link_timeout(timeout_sqe, &mut ts, 0);
    }

    unsafe {
      fiona::liburing::io_uring_submit(ring);
    }

    connect_fds.initiated = true;

    std::task::Poll::Pending
  }
}

impl<'a> std::future::Future for ReadFuture<'a> {
  type Output = Result<Vec<u8>, fiona::libc::Errno>;

  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.read_fds.get();
    let read_fds = unsafe { &mut *p };

    if read_fds.done {
      if read_fds.res < 0 {
        return std::task::Poll::Ready(Err(fiona::libc::errno(-read_fds.res)));
      }

      let mut buf = match read_fds.op {
        fiona::op::Op::Read(ref mut s) => s.buf.take().unwrap(),
        _ => {
          panic!("Read op was completed but the internal Op is an invalid type")
        }
      };

      unsafe { buf.set_len(buf.len() + read_fds.res as usize) };
      return std::task::Poll::Ready(Ok(buf));
    }

    if read_fds.initiated {
      return std::task::Poll::Pending;
    }

    let ioc_state = unsafe { &mut *self.ex.get_state() };
    read_fds.task = Some(ioc_state.task_ctx.unwrap());

    let ring = ioc_state.ring;
    let sockfd = read_fds.fd;
    let read_sqe = unsafe { fiona::liburing::make_sqe(ring) };

    let (buf, nbytes, offset) = match read_fds.op {
      fiona::op::Op::Read(ref mut s) => match s.buf {
        Some(ref mut b) => (b.as_mut_ptr(), b.capacity(), b.len()),
        _ => panic!(""),
      },
      _ => panic!(""),
    };

    let user_data = self
      .read_fds
      .clone()
      .into_raw()
      .cast::<fiona::libc::c_void>();

    unsafe { fiona::liburing::io_uring_sqe_set_data(read_sqe, user_data) };

    unsafe {
      fiona::liburing::io_uring_prep_read(
        read_sqe,
        sockfd,
        buf.cast::<fiona::libc::c_void>(),
        nbytes as u32,
        offset as u64,
      );
    }

    unsafe { fiona::liburing::io_uring_sqe_set_flags(read_sqe, 1_u32 << 2) };

    let mut ts = make_kernel_timespec(self.timeout);
    let timeout_sqe = unsafe { fiona::liburing::make_sqe(ring) };
    unsafe {
      fiona::liburing::io_uring_prep_link_timeout(timeout_sqe, &mut ts, 0);
    }

    unsafe { fiona::liburing::io_uring_submit(ring) };

    read_fds.initiated = true;

    std::task::Poll::Pending
  }
}

impl<'a> std::future::Future for WriteFuture<'a> {
  type Output = Result<Vec<u8>, fiona::libc::Errno>;
  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.write_fds.get();
    let fds = unsafe { &mut *p };

    if fds.done {
      if fds.res < 0 {
        return std::task::Poll::Ready(Err(fiona::libc::errno(-fds.res)));
      }

      let buf = match fds.op {
        fiona::op::Op::Write(ref mut s) => s.buf.take().unwrap(),
        _ => {
          panic!(
            "Write op was completed but the internal Op is an invalid type"
          )
        }
      };

      return std::task::Poll::Ready(Ok(buf));
    }

    if fds.initiated {
      return std::task::Poll::Pending;
    }

    let sockfd = fds.fd;

    let ioc_state = unsafe { &mut *self.ex.get_state() };
    fds.task = Some(ioc_state.task_ctx.unwrap());

    let ring = ioc_state.ring;
    let write_sqe = unsafe { fiona::liburing::make_sqe(ring) };

    let (buf, nbytes, offset) = match fds.op {
      fiona::op::Op::Write(ref mut s) => match s.buf {
        Some(ref mut b) => (b.as_ptr(), b.len(), 0_u64),
        _ => panic!("In WriteFuture, buf was null when it should've been Some"),
      },
      _ => panic!("Incorrect operation type in WriteFuture"),
    };

    let user_data = self
      .write_fds
      .clone()
      .into_raw()
      .cast::<fiona::libc::c_void>();

    unsafe { fiona::liburing::io_uring_sqe_set_data(write_sqe, user_data) };

    unsafe {
      fiona::liburing::io_uring_prep_write(
        write_sqe,
        sockfd,
        buf.cast::<fiona::libc::c_void>(),
        nbytes as u32,
        offset as u64,
      );
    }

    unsafe { fiona::liburing::io_uring_sqe_set_flags(write_sqe, 1_u32 << 2) };

    let mut ts = make_kernel_timespec(self.timeout);
    let timeout_sqe = unsafe { fiona::liburing::make_sqe(ring) };
    unsafe {
      fiona::liburing::io_uring_prep_link_timeout(timeout_sqe, &mut ts, 0);
    }

    unsafe { fiona::liburing::io_uring_submit(ring) };
    fds.initiated = true;
    std::task::Poll::Pending
  }
}

impl Acceptor {
  #[must_use]
  pub fn new(ex: &fiona::Executor) -> Self {
    Self {
      fd: -1,
      ex: ex.clone(),
    }
  }

  pub fn listen(
    &mut self,
    ipv4_addr: u32,
    port: u16,
  ) -> Result<(), fiona::libc::Errno> {
    match fiona::liburing::make_ipv4_tcp_server_socket(ipv4_addr, port) {
      Ok(fd) => {
        self.fd = fd;
        Ok(())
      }
      Err(e) => Err(e),
    }
  }

  pub fn async_accept(&mut self) -> AcceptFuture {
    assert!(self.fd > 0);
    let fds = fiona::op::FdState::new(
      self.fd,
      fiona::op::Op::Accept(fiona::op::AcceptState {
        addr_in: fiona::ip::tcp::sockaddr_in::default(),
        addr_len: std::mem::size_of::<fiona::ip::tcp::sockaddr_in>() as u32,
      }),
    );

    AcceptFuture {
      ex: self.ex.clone(),
      fds,
      _m: std::marker::PhantomData,
    }
  }
}

impl Drop for Acceptor {
  fn drop(&mut self) {
    if self.fd >= 0 {
      unsafe { fiona::libc::close(self.fd) };
    }
  }
}

impl Drop for Socket {
  fn drop(&mut self) {
    if self.fd != -1 {
      unsafe { fiona::libc::close(self.fd) };
    }
  }
}

impl Socket {
  #[must_use]
  pub fn new(ex: &fiona::Executor) -> Self {
    Self {
      fd: fiona::liburing::make_ipv4_tcp_socket().unwrap(),
      ex: ex.clone(),
      timeout: std::time::Duration::from_secs(30),
    }
  }

  #[must_use]
  pub unsafe fn from_raw(ex: fiona::Executor, fd: i32) -> Self {
    Self {
      fd,
      ex,
      timeout: std::time::Duration::from_secs(30),
    }
  }

  pub fn async_connect(&mut self, ipv4_addr: u32, port: u16) -> ConnectFuture {
    let connect_fds = fiona::op::FdState::new(
      self.fd,
      fiona::op::Op::Connect(fiona::op::ConnectState {
        addr_in: unsafe { fiona::libc::rio_make_sockaddr_in(ipv4_addr, port) },
        timer_fds: None,
      }),
    );

    ConnectFuture {
      ex: self.ex.clone(),
      connect_fds,
      timeout: self.timeout,
      _m: std::marker::PhantomData,
    }
  }

  pub fn async_read(&mut self, buf: Vec<u8>) -> ReadFuture {
    let read_fds = fiona::op::FdState::new(
      self.fd,
      fiona::op::Op::Read(fiona::op::ReadState {
        buf: Some(buf),
        timer_fds: None,
      }),
    );

    ReadFuture {
      ex: self.ex.clone(),
      read_fds,
      timeout: self.timeout,
      _m: std::marker::PhantomData,
    }
  }

  pub fn async_write(&mut self, buf: Vec<u8>) -> WriteFuture {
    let write_fds = fiona::op::FdState::new(
      self.fd,
      fiona::op::Op::Write(fiona::op::WriteState { buf: Some(buf) }),
    );

    WriteFuture {
      ex: self.ex.clone(),
      write_fds,
      timeout: self.timeout,
      _m: std::marker::PhantomData,
    }
  }
}

impl<'a> AcceptFuture<'a> {
  #[must_use]
  pub fn get_cancel_handle(&self) -> fiona::op::CancelHandle {
    fiona::op::CancelHandle::new(self.fds.clone(), self.ex.clone())
  }
}

impl<'a> ConnectFuture<'a> {
  #[must_use]
  pub fn get_cancel_handle(&self) -> fiona::op::CancelHandle {
    fiona::op::CancelHandle::new(self.connect_fds.clone(), self.ex.clone())
  }
}
