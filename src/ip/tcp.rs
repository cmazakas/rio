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
  ex: rio::Executor,
}

pub struct AcceptFuture<'a> {
  ex: rio::Executor,
  fds: rio::op::FdState,
  _m: std::marker::PhantomData<&'a mut Acceptor>,
}

pub struct ConnectFuture<'a> {
  ex: rio::Executor,
  fds: rio::op::FdState,
  s: &'a mut Socket,
}

impl<'a> std::future::Future for AcceptFuture<'a> {
  type Output = Result<i32, rio::libc::Errno>;
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
      unsafe { rio::liburing::io_uring_prep_accept(sqe, fd, addr, addrlen) };
      unsafe { rio::liburing::io_uring_submit(ring) };
      return std::task::Poll::Pending;
    }

    if !unsafe { (*p).done } {
      return std::task::Poll::Pending;
    }

    let fd = unsafe { (*p).res };
    if fd < 0 {
      std::task::Poll::Ready(Err(rio::libc::errno(-fd).unwrap_err()))
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
      std::task::Poll::Ready(Ok(fd))
    }
  }
}

impl<'a> std::future::Future for ConnectFuture<'a> {
  type Output = Result<(), rio::libc::Errno>;
  fn poll(
    self: std::pin::Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    std::task::Poll::Ready(Ok(()))
  }
}

impl Acceptor {
  #[must_use]
  pub fn new(ex: rio::Executor) -> Self {
    Self { fd: -1, ex }
  }

  pub fn listen(&mut self, ipv4_addr: u32, port: u16) -> Result<(), rio::libc::Errno> {
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

impl Socket {
  pub fn async_connect(&mut self, ipv4_addr: u32, port: u16) -> ConnectFuture {
    assert_eq!(self.fd, -1);

    let fds = rio::op::FdState::new(
      self.fd,
      rio::op::Op::Connect(rio::op::ConnectState {
        addr_in: unsafe { rio::libc::rio_make_sockaddr_in(ipv4_addr, port) },
      }),
    );

    ConnectFuture {
      ex: self.ex.clone(),
      fds,
      s: self,
    }
  }
}
