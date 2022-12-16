use crate as rio;

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
  ex: rio::Executor,
}

pub struct Socket {
  fd: i32,
  timer_fd: i32,
  ex: rio::Executor,
  timeout: std::time::Duration,
}

pub struct AcceptFuture<'a> {
  ex: rio::Executor,
  fds: rio::op::FdState,
  _m: std::marker::PhantomData<&'a mut Acceptor>,
}

pub struct ConnectFuture<'a> {
  ex: rio::Executor,
  connect_fds: rio::op::FdState,
  timer_fds: rio::op::FdState,
  timeout: std::time::Duration,
  _m: std::marker::PhantomData<&'a mut Socket>,
}

pub struct ReadFuture<'a> {
  ex: rio::Executor,
  fds: rio::op::FdState,
  _m: std::marker::PhantomData<&'a mut Socket>,
}

pub struct WriteFuture<'a> {
  ex: rio::Executor,
  fds: rio::op::FdState,
  _m: std::marker::PhantomData<&'a mut Socket>,
}

unsafe fn drop_cancel(
  p: *mut rio::op::FdStateImpl,
  ioc: *mut rio::IoContextState,
) {
  if (*p).initiated && !(*p).done {
    let ring = (*ioc).ring;
    unsafe {
      let sqe = rio::liburing::make_sqe(ring);
      rio::liburing::io_uring_prep_cancel(
        sqe,
        p.cast::<rio::libc::c_void>(),
        0,
      );
      rio::liburing::io_uring_submit(ring);
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
    unsafe { drop_cancel(self.timer_fds.get(), self.ex.get_state()) };
  }
}

impl<'a> Drop for ReadFuture<'a> {
  fn drop(&mut self) {
    unsafe { drop_cancel(self.fds.get(), self.ex.get_state()) };
  }
}

impl<'a> Drop for WriteFuture<'a> {
  fn drop(&mut self) {
    unsafe { drop_cancel(self.fds.get(), self.ex.get_state()) };
  }
}

impl<'a> std::future::Future for AcceptFuture<'a> {
  type Output = Result<Socket, rio::libc::Errno>;
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
      let sqe = unsafe { rio::liburing::make_sqe(ring) };
      let user_data = self.fds.clone().into_raw().cast::<rio::libc::c_void>();

      let (addr, addrlen) = match unsafe { &mut (*p).op } {
        rio::op::Op::Accept(ref mut s) => (
          std::ptr::addr_of_mut!(s.addr_in),
          std::ptr::addr_of_mut!(s.addr_len),
        ),
        _ => panic!("incorrect op type specified for the AcceptFuture"),
      };

      unsafe { rio::liburing::io_uring_sqe_set_data(sqe, user_data) };
      unsafe { rio::liburing::io_uring_prep_accept(sqe, fd, addr, addrlen, 0) };
      unsafe { rio::liburing::io_uring_submit(ring) };
      return std::task::Poll::Pending;
    }

    if !unsafe { (*p).done } {
      return std::task::Poll::Pending;
    }

    let fd = unsafe { (*p).res };
    if fd < 0 {
      std::task::Poll::Ready(Err(rio::libc::errno(-fd)))
    } else {
      // let addr = match unsafe { &mut (*p).op } {
      //   rio::op::Op::Accept(ref mut s) => s.addr_in,
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
  type Output = Result<(), rio::libc::Errno>;
  fn poll(
    mut self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.connect_fds.get();
    let connect_fds = unsafe { &mut *p };

    let p = self.timer_fds.get();
    let timer_fds = unsafe { &mut *p };

    println!("in ConnectFuture");
    println!("connect_fds: {:?}", self.connect_fds.get());
    println!("timer_fds: {:?}", self.timer_fds.get());

    if connect_fds.done {
      if connect_fds.res < 0 {
        return std::task::Poll::Ready(Err(rio::libc::errno(-connect_fds.res)));
      }

      return std::task::Poll::Ready(Ok(()));
    }

    if connect_fds.initiated {
      assert!(timer_fds.initiated);
      return std::task::Poll::Pending;
    }

    let ioc_state = unsafe { &mut *self.ex.get_state() };
    connect_fds.task = Some(ioc_state.task_ctx.unwrap());
    timer_fds.task = Some(ioc_state.task_ctx.unwrap());

    let ring = self.ex.get_ring();

    let connect_sqe = unsafe { rio::liburing::make_sqe(ring) };

    let (addr, addrlen) = match connect_fds.op {
      rio::op::Op::Connect(ref s) => (
        std::ptr::addr_of!(s.addr_in),
        std::mem::size_of::<rio::ip::tcp::sockaddr_in>() as u32,
      ),
      _ => panic!(""),
    };

    let user_data = self
      .connect_fds
      .clone()
      .into_raw()
      .cast::<rio::libc::c_void>();

    unsafe { rio::liburing::io_uring_sqe_set_data(connect_sqe, user_data) };
    unsafe {
      rio::liburing::io_uring_prep_connect(
        connect_sqe,
        connect_fds.fd,
        addr.cast::<rio::libc::sockaddr>(),
        addrlen,
      );
    }
    unsafe { rio::liburing::io_uring_sqe_set_flags(connect_sqe, 1_u32 << 2) };

    let cancel_timer_sqe = unsafe { rio::liburing::make_sqe(ring) };
    unsafe {
      rio::liburing::io_uring_prep_cancel(
        cancel_timer_sqe,
        self.timer_fds.get().cast::<rio::libc::c_void>(),
        0,
      );
    }

    self.timeout = std::time::Duration::from_secs(15);
    println!("setting timeout to: {}", self.timeout.as_secs());
    let timer_sqe = unsafe { rio::liburing::make_sqe(ring) };
    assert!(
      unsafe {
        rio::liburing::timerfd_settime(
          timer_fds.fd,
          self.timeout.as_secs() as u64,
          u64::from(self.timeout.subsec_nanos()),
        )
      }
      .is_ok(),
      "Failed to set timer on FD"
    );

    let buf = match timer_fds.op {
      rio::op::Op::Timer(ref mut ts) => {
        std::ptr::addr_of_mut!(ts.buf).cast::<rio::libc::c_void>()
      }
      _ => panic!("invalid op type in TimerFuture"),
    };

    let user_data = self
      .timer_fds
      .clone()
      .into_raw()
      .cast::<rio::libc::c_void>();

    unsafe {
      rio::liburing::io_uring_sqe_set_data(timer_sqe, user_data);
      rio::liburing::io_uring_prep_read(timer_sqe, timer_fds.fd, buf, 8, 0);
      rio::liburing::io_uring_sqe_set_flags(timer_sqe, 1_u32 << 2);
    }

    let cancel_connect_sqe = unsafe { rio::liburing::make_sqe(ring) };
    unsafe {
      rio::liburing::io_uring_prep_cancel(
        cancel_connect_sqe,
        self.connect_fds.get().cast::<rio::libc::c_void>(),
        0,
      );
    }

    unsafe {
      rio::liburing::io_uring_submit(ring);
    }

    connect_fds.initiated = true;
    timer_fds.initiated = true;

    std::task::Poll::Pending

    // if !fds.initiated {
    //   fds.initiated = true;

    //   let sockfd = fds.fd;

    //   let ioc_state = unsafe { &mut *self.ex.get_state() };
    //   fds.task = Some(ioc_state.task_ctx.unwrap());

    //   let ring = ioc_state.ring;
    //   let sqe = unsafe { rio::liburing::make_sqe(ring) };

    //   let (addr, addrlen) = match fds.op {
    //     rio::op::Op::Connect(ref s) => (
    //       std::ptr::addr_of!(s.addr_in),
    //       std::mem::size_of::<rio::ip::tcp::sockaddr_in>() as u32,
    //     ),
    //     _ => panic!(""),
    //   };

    //   let user_data = self.fds.clone().into_raw().cast::<rio::libc::c_void>();
    //   unsafe { rio::liburing::io_uring_sqe_set_data(sqe, user_data) };

    //   unsafe {
    //     rio::liburing::io_uring_prep_connect(
    //       sqe,
    //       sockfd,
    //       addr.cast::<rio::libc::sockaddr>(),
    //       addrlen,
    //     );
    //   }

    //   unsafe { rio::liburing::io_uring_submit(ring) };

    //   assert!(unsafe {
    //     std::pin::Pin::new_unchecked(&mut self.timer_future)
    //       .poll(cx)
    //       .is_pending()
    //   });

    //   return std::task::Poll::Pending;
    // }

    // if !fds.done {
    //   let timer_expired = unsafe {
    //     std::pin::Pin::new_unchecked(&mut self.timer_future)
    //       .poll(cx)
    //       .is_ready()
    //   };

    //   if timer_expired {
    //     self.get_cancel_handle().cancel();
    //   }

    //   return std::task::Poll::Pending;
    // }

    // if fds.res < 0 {
    //   std::task::Poll::Ready(Err(rio::libc::errno(-fds.res)))
    // } else {
    //   std::task::Poll::Ready(Ok(()))
    // }
  }
}

impl<'a> std::future::Future for ReadFuture<'a> {
  type Output = Result<Vec<u8>, rio::libc::Errno>;
  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.fds.get();
    let fds = unsafe { &mut *p };

    if fds.done {
      if fds.res < 0 {
        return std::task::Poll::Ready(Err(rio::libc::errno(-fds.res)));
      }

      let mut buf = match fds.op {
        rio::op::Op::Read(ref mut s) => s.buf.take().unwrap(),
        _ => {
          panic!("Read op was completed but the internal Op is an invalid type")
        }
      };

      #[allow(clippy::cast_sign_loss)]
      unsafe {
        buf.set_len(buf.len() + fds.res as usize);
      }

      return std::task::Poll::Ready(Ok(buf));
    }

    if fds.initiated {
      return std::task::Poll::Pending;
    }

    fds.initiated = true;

    let sockfd = fds.fd;

    let ioc_state = unsafe { &mut *self.ex.get_state() };
    fds.task = Some(ioc_state.task_ctx.unwrap());

    let ring = ioc_state.ring;
    let sqe = unsafe { rio::liburing::make_sqe(ring) };

    let (buf, nbytes, offset) = match fds.op {
      rio::op::Op::Read(ref mut s) => match s.buf {
        Some(ref mut b) => (b.as_mut_ptr(), b.capacity(), b.len()),
        _ => panic!("In ReadFuture, buf was null when it should've been Some"),
      },
      _ => panic!("Incorrect operation type in ReadFuture"),
    };

    let user_data = self.fds.clone().into_raw().cast::<rio::libc::c_void>();
    unsafe { rio::liburing::io_uring_sqe_set_data(sqe, user_data) };

    unsafe {
      rio::liburing::io_uring_prep_read(
        sqe,
        sockfd,
        buf.cast::<rio::libc::c_void>(),
        nbytes as u32,
        offset as u64,
      );
    }

    unsafe { rio::liburing::io_uring_submit(ring) };

    std::task::Poll::Pending
  }
}

impl<'a> std::future::Future for WriteFuture<'a> {
  type Output = Result<Vec<u8>, rio::libc::Errno>;
  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let p = self.fds.get();
    let fds = unsafe { &mut *p };

    if fds.done {
      if fds.res < 0 {
        return std::task::Poll::Ready(Err(rio::libc::errno(-fds.res)));
      }

      let buf = match fds.op {
        rio::op::Op::Write(ref mut s) => s.buf.take().unwrap(),
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

    fds.initiated = true;

    let sockfd = fds.fd;

    let ioc_state = unsafe { &mut *self.ex.get_state() };
    fds.task = Some(ioc_state.task_ctx.unwrap());

    let ring = ioc_state.ring;
    let sqe = unsafe { rio::liburing::make_sqe(ring) };

    let (buf, nbytes, offset) = match fds.op {
      rio::op::Op::Write(ref mut s) => match s.buf {
        Some(ref mut b) => (b.as_ptr(), b.len(), 0_u64),
        _ => panic!("In WriteFuture, buf was null when it should've been Some"),
      },
      _ => panic!("Incorrect operation type in WriteFuture"),
    };

    let user_data = self.fds.clone().into_raw().cast::<rio::libc::c_void>();
    unsafe { rio::liburing::io_uring_sqe_set_data(sqe, user_data) };

    unsafe {
      rio::liburing::io_uring_prep_write(
        sqe,
        sockfd,
        buf.cast::<rio::libc::c_void>(),
        nbytes as u32,
        offset as u64,
      );
    }

    unsafe { rio::liburing::io_uring_submit(ring) };

    std::task::Poll::Pending
  }
}

impl Acceptor {
  #[must_use]
  pub fn new(ex: rio::Executor) -> Self {
    Self { fd: -1, ex }
  }

  pub fn listen(
    &mut self,
    ipv4_addr: u32,
    port: u16,
  ) -> Result<(), rio::libc::Errno> {
    match rio::liburing::make_ipv4_tcp_server_socket(ipv4_addr, port) {
      Ok(fd) => {
        self.fd = fd;
        Ok(())
      }
      Err(e) => Err(e),
    }
  }

  pub fn async_accept(&mut self) -> AcceptFuture {
    assert!(self.fd > 0);
    let fds = rio::op::FdState::new(
      self.fd,
      rio::op::Op::Accept(rio::op::AcceptState {
        addr_in: rio::ip::tcp::sockaddr_in::default(),
        addr_len: std::mem::size_of::<rio::ip::tcp::sockaddr_in>() as u32,
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
      unsafe { rio::libc::close(self.fd) };
    }
  }
}

impl Drop for Socket {
  fn drop(&mut self) {
    if self.fd != -1 {
      unsafe { rio::libc::close(self.fd) };
      unsafe { rio::libc::close(self.timer_fd) };
    }
  }
}

impl Socket {
  #[must_use]
  pub fn new(ex: rio::Executor) -> Self {
    let timer_fd = rio::liburing::timerfd_create();
    assert!(timer_fd != -1, "Can't create a timer");

    Self {
      fd: rio::liburing::make_ipv4_tcp_socket().unwrap(),
      ex,
      timer_fd,
      timeout: std::time::Duration::from_secs(30),
    }
  }

  #[must_use]
  pub unsafe fn from_raw(ex: rio::Executor, fd: i32) -> Self {
    let timer_fd = rio::liburing::timerfd_create();
    assert!(timer_fd != -1, "Can't create a timer");

    Self {
      fd,
      ex: ex.clone(),
      timer_fd,
      timeout: std::time::Duration::from_secs(30),
    }
  }

  pub fn async_connect(&mut self, ipv4_addr: u32, port: u16) -> ConnectFuture {
    let connect_fds = rio::op::FdState::new(
      self.fd,
      rio::op::Op::Connect(rio::op::ConnectState {
        addr_in: unsafe { rio::libc::rio_make_sockaddr_in(ipv4_addr, port) },
      }),
    );

    let timer_fds = rio::op::FdState::new(
      self.timer_fd,
      rio::op::Op::Timer(rio::op::TimerState { buf: 0 }),
    );

    let timeout = self.timeout;

    ConnectFuture {
      ex: self.ex.clone(),
      connect_fds,
      timer_fds,
      timeout,
      _m: std::marker::PhantomData,
    }
  }

  pub fn async_read(&mut self, buf: Vec<u8>) -> ReadFuture {
    let fds = rio::op::FdState::new(
      self.fd,
      rio::op::Op::Read(rio::op::ReadState { buf: Some(buf) }),
    );

    ReadFuture {
      ex: self.ex.clone(),
      fds,
      _m: std::marker::PhantomData,
    }
  }

  pub fn async_write(&mut self, buf: Vec<u8>) -> WriteFuture {
    let fds = rio::op::FdState::new(
      self.fd,
      rio::op::Op::Write(rio::op::WriteState { buf: Some(buf) }),
    );

    WriteFuture {
      ex: self.ex.clone(),
      fds,
      _m: std::marker::PhantomData,
    }
  }
}

impl<'a> AcceptFuture<'a> {
  #[must_use]
  pub fn get_cancel_handle(&self) -> rio::op::CancelHandle {
    rio::op::CancelHandle::new(self.fds.clone(), self.ex.clone())
  }
}

impl<'a> ConnectFuture<'a> {
  #[must_use]
  pub fn get_cancel_handle(&self) -> rio::op::CancelHandle {
    rio::op::CancelHandle::new(self.timer_fds.clone(), self.ex.clone())
  }
}
