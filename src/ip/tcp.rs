// use crate as rio;

// pub struct Acceptor {
//   fd: i32,
//   ex: rio::Executor,
// }

// pub struct Socket {
//   fd: i32,
//   ex: rio::Executor,
// }

// pub struct AcceptFuture<'a> {
//   initiated: bool,
//   ex: rio::Executor,
//   state: std::rc::Rc<std::cell::UnsafeCell<rio::FdFutureSharedState>>,
//   _m: std::marker::PhantomData<&'a mut Acceptor>,
// }

// pub struct ConnectFuture<'a> {
//   initiated: bool,
//   ex: rio::Executor,
//   state: std::rc::Rc<std::cell::UnsafeCell<rio::FdFutureSharedState>>,
//   s: &'a mut Socket,
// }

// impl<'a> std::future::Future for AcceptFuture<'a> {
//   type Output = Result<i32, rio::libc::Errno>;
//   fn poll(
//     mut self: std::pin::Pin<&mut Self>,
//     _cx: &mut std::task::Context<'_>,
//   ) -> std::task::Poll<Self::Output> {
//     let pfds = self.state.get();

//     if !self.initiated {
//       self.initiated = true;
//       let fd = unsafe { (*pfds).fd };
//       let ioc_state = unsafe { &mut *self.ex.get_state() };
//       unsafe { (*pfds).task = Some(ioc_state.task_ctx.unwrap()) };

//       let ring = unsafe { (*self.ex.get_state()).ring };
//       let sqe = unsafe { rio::liburing::make_sqe(ring) };
//       let user_data = std::rc::Rc::into_raw(self.state.clone()).cast::<rio::libc::c_void>();

//       unsafe { rio::liburing::io_uring_sqe_set_data(sqe, user_data as *mut rio::libc::c_void) };
//       unsafe { rio::liburing::io_uring_prep_accept(sqe, fd) };
//       unsafe { rio::liburing::io_uring_submit(ring) };
//       return std::task::Poll::Pending;
//     }

//     if !unsafe { (*pfds).done } {
//       return std::task::Poll::Pending;
//     }

//     let fd = unsafe { (*pfds).res };
//     if fd < 0 {
//       std::task::Poll::Ready(Err(rio::libc::errno(-fd).unwrap_err()))
//     } else {
//       std::task::Poll::Ready(Ok(fd))
//     }
//   }
// }

// impl<'a> std::future::Future for ConnectFuture<'a> {
//   type Output = Result<(), rio::libc::Errno>;
//   fn poll(
//     self: std::pin::Pin<&mut Self>,
//     _cx: &mut std::task::Context<'_>,
//   ) -> std::task::Poll<Self::Output> {
//     std::task::Poll::Ready(Ok(()))
//   }
// }

// impl Acceptor {
//   #[must_use]
//   pub fn new(ex: rio::Executor) -> Self {
//     Self { fd: -1, ex }
//   }

//   pub fn listen(&mut self, ipv4_addr: u32, port: u16) -> Result<(), rio::libc::Errno> {
//     match rio::liburing::make_ipv4_tcp_server_socket(ipv4_addr, port) {
//       Ok(fd) => {
//         self.fd = fd;
//         Ok(())
//       }
//       Err(e) => Err(e),
//     }
//   }

//   pub fn async_accept(&mut self) -> AcceptFuture {
//     assert!(self.fd > 0);
//     let shared_statep = std::rc::Rc::new(std::cell::UnsafeCell::new(rio::FdFutureSharedState {
//       done: false,
//       fd: self.fd,
//       res: -1,
//       task: None,
//       disarmed: false,
//     }));

//     AcceptFuture {
//       initiated: false,
//       ex: self.ex.clone(),
//       state: shared_statep,
//       _m: std::marker::PhantomData,
//     }
//   }
// }

// impl Socket {
//   pub fn async_connect(&mut self) -> ConnectFuture {
//     assert_eq!(self.fd, -1);

//     let shared_statep = std::rc::Rc::new(std::cell::UnsafeCell::new(rio::FdFutureSharedState {
//       done: false,
//       fd: self.fd,
//       res: -1,
//       task: None,
//       disarmed: false,
//     }));

//     ConnectFuture {
//       initiated: false,
//       ex: self.ex.clone(),
//       state: shared_statep,
//       s: self,
//     }
//   }
// }
