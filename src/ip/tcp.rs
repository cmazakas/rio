extern crate libc;
extern crate nix;

use crate as fiona;

pub mod tls;

pub struct Acceptor {
    fd: i32,
    ex: fiona::Executor,
}

pub struct Socket {
    fd: i32,
    ex: fiona::Executor,
    pub timeout: std::time::Duration,
}

pub struct Client {
    s: Socket,
}

pub struct Server {
    s: Socket,
}

// service => port number or service name
// for list of valid service names, use `cat /etc/services`
// valid examples include "http" and "https"
#[must_use]
pub fn async_resolve_dns(host: &str, service: &str) -> DNSFuture {
    let node = Some(std::ffi::CString::new(host).unwrap());
    let service = Some(std::ffi::CString::new(service).unwrap());

    DNSFuture {
        node,
        service,
        ..Default::default()
    }
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

#[derive(Default)]
pub struct DNSFuture {
    handle: Option<std::thread::JoinHandle<Result<Vec<(std::net::IpAddr, u16)>, fiona::Errno>>>,
    done: bool,
    node: Option<std::ffi::CString>,
    service: Option<std::ffi::CString>,
}

impl std::future::Future for DNSFuture {
    type Output = Result<Vec<(std::net::IpAddr, u16)>, fiona::Errno>;
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        struct DropGuard {
            head: *mut libc::addrinfo,
        }
        impl Drop for DropGuard {
            fn drop(&mut self) {
                unsafe {
                    libc::freeaddrinfo(self.head);
                }
            }
        }

        match &self.handle {
            Some(h) => {
                if !h.is_finished() {
                    return std::task::Poll::Pending;
                }
                let h = self.handle.take().unwrap();
                let results = h.join().unwrap();
                self.done = true;
                std::task::Poll::Ready(results)
            }
            None => {
                assert!(!self.done, "This future already completed");

                let node = self.node.take().unwrap();
                let service = self.service.take().unwrap();
                let waker = cx.waker().clone();
                self.handle = Some(std::thread::spawn(move || {
                    let mut hints = unsafe { std::mem::zeroed::<libc::addrinfo>() };
                    hints.ai_family = libc::AF_UNSPEC;
                    hints.ai_socktype = libc::SOCK_STREAM;
                    hints.ai_protocol = libc::IPPROTO_TCP;
                    hints.ai_flags = 0;

                    let mut res = std::ptr::null_mut::<libc::addrinfo>();

                    let r = unsafe { libc::getaddrinfo(node.as_ptr(), service.as_ptr(), &hints, &mut res) };

                    if r != 0 {
                        return Err(unsafe { std::mem::transmute(r) });
                    }

                    let _guard = DropGuard { head: res };
                    let mut v = Vec::new();

                    while !res.is_null() {
                        let addrinfo = unsafe { &mut *res };
                        res = addrinfo.ai_next;
                        if addrinfo.ai_family == libc::AF_INET {
                            let mut addr_in = unsafe { std::mem::zeroed::<libc::sockaddr_in>() };
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    addrinfo.ai_addr.cast::<libc::sockaddr_in>(),
                                    &mut addr_in,
                                    1,
                                );
                            }

                            let ipv4 = std::net::Ipv4Addr::from(addr_in.sin_addr.s_addr.to_be());

                            let port = addr_in.sin_port.to_be();
                            v.push((std::net::IpAddr::V4(ipv4), port));
                        }

                        if addrinfo.ai_family == libc::AF_INET6 {
                            let mut addr_in6 = unsafe { std::mem::zeroed::<libc::sockaddr_in6>() };

                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    addrinfo.ai_addr.cast::<libc::sockaddr_in6>(),
                                    &mut addr_in6,
                                    1,
                                );
                            }

                            let ipv6 = std::net::Ipv6Addr::from(addr_in6.sin6_addr.s6_addr);

                            let port = addr_in6.sin6_port.to_be();
                            v.push((std::net::IpAddr::V6(ipv6), port));
                        }
                    }

                    waker.wake();
                    Ok(v)
                }));

                std::task::Poll::Pending
            }
        }
    }
}

impl Acceptor {
    #[must_use]
    pub fn new(ex: &fiona::Executor) -> Self {
        Self { fd: -1, ex: ex.clone() }
    }

    pub fn listen(&mut self, ip_addr: std::net::IpAddr, port: u16) -> Result<(), fiona::Errno> {
        match ip_addr {
            std::net::IpAddr::V4(ipv4_addr) => {
                match fiona::liburing::make_ipv4_tcp_server_socket(u32::from(ipv4_addr), port) {
                    Ok(fd) => {
                        self.fd = fd;
                        Ok(())
                    }
                    Err(e) => Err(unsafe { std::mem::transmute(e) }),
                }
            }
            std::net::IpAddr::V6(_ipv6_addr) => {
                unimplemented!();
                // let addr = libc::in6_addr {
                //     s6_addr: ipv6_addr.octets(),
                // };

                // match fiona::liburing::make_ipv6_tcp_server_socket(addr,
                // port) {     Ok(fd) => {
                //         self.fd = fd;
                //         Ok(())
                //     }
                //     Err(e) => Err(unsafe { std::mem::transmute(e) }),
                // }
            }
        }
    }

    pub fn async_accept(&mut self) -> AcceptFuture {
        assert!(self.fd > 0);
        let addr_in: libc::sockaddr_storage = unsafe { std::mem::zeroed() };

        let fds = fiona::op::FdState::new(
            self.fd,
            fiona::op::Op::Accept(fiona::op::AcceptState {
                addr_in,
                addr_len: std::mem::size_of::<libc::sockaddr_storage>() as u32,
            }),
        );

        AcceptFuture {
            ex: self.ex.clone(),
            fds,
            _m: std::marker::PhantomData,
        }
    }
}

impl Socket {
    fn fd(&self) -> i32 {
        self.fd
    }

    fn ex(&self) -> fiona::Executor {
        self.ex.clone()
    }

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
        let write_fds =
            fiona::op::FdState::new(self.fd, fiona::op::Op::Write(fiona::op::WriteState { buf: Some(buf) }));

        WriteFuture {
            ex: self.ex.clone(),
            write_fds,
            timeout: self.timeout,
            _m: std::marker::PhantomData,
        }
    }
}

impl Client {
    #[must_use]
    pub fn new(ex: &fiona::Executor) -> Self {
        Self { s: Socket::new(ex) }
    }

    pub fn async_connect(&mut self, ip_addr: std::net::IpAddr, port: u16) -> ConnectFuture {
        let mut addr_storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };

        match ip_addr {
            std::net::IpAddr::V4(ipv4_addr) => {
                addr_storage.ss_family = libc::AF_INET as u16;

                let ipv4_addr = libc::sockaddr_in {
                    sin_family: libc::AF_INET as u16,
                    sin_port: port.to_be(),
                    sin_addr: libc::in_addr {
                        s_addr: u32::from(ipv4_addr).to_be(),
                    },
                    sin_zero: Default::default(),
                };

                unsafe {
                    std::ptr::copy_nonoverlapping(
                        std::ptr::addr_of!(ipv4_addr).cast::<u8>(),
                        std::ptr::addr_of_mut!(addr_storage).cast::<u8>(),
                        std::mem::size_of::<libc::sockaddr_in>(),
                    );
                }
            }
            std::net::IpAddr::V6(_ipv6_addr) => {
                unimplemented!();
                // addr_storage.ss_family = libc::AF_INET6 as u16;

                // let ipv6_addr = libc::sockaddr_in6 {
                //     sin6_family: libc::AF_INET6 as u16,
                //     sin6_port: port,
                //     sin6_flowinfo: 0,
                //     sin6_scope_id: 0,
                //     sin6_addr: libc::in6_addr {
                //         s6_addr: ipv6_addr.octets(),
                //     },
                // };

                // unsafe {
                //     std::ptr::copy_nonoverlapping(
                //         std::ptr::addr_of!(ipv6_addr).cast::<u8>(),
                //         std::ptr::addr_of_mut!(addr_storage).cast::<u8>(),
                //         std::mem::size_of::<libc::sockaddr_in6>(),
                //     );
                // }
            }
        };

        let connect_fds = fiona::op::FdState::new(
            self.fd(),
            fiona::op::Op::Connect(fiona::op::ConnectState {
                addr_storage,
                timer_fds: None,
            }),
        );

        ConnectFuture {
            ex: self.ex(),
            connect_fds,
            timeout: self.timeout,
            _m: std::marker::PhantomData,
        }
    }
}

impl Server {
    #[must_use]
    pub fn new(ex: &fiona::Executor) -> Self {
        Self { s: Socket::new(ex) }
    }
}

impl std::ops::Deref for Client {
    type Target = Socket;
    fn deref(&self) -> &Self::Target {
        &self.s
    }
}

impl std::ops::DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.s
    }
}

impl std::ops::Deref for Server {
    type Target = Socket;
    fn deref(&self) -> &Self::Target {
        &self.s
    }
}

impl std::ops::DerefMut for Server {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.s
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
    type Output = Result<Server, fiona::Errno>;
    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let p = self.fds.get();
        let accept_fds = unsafe { &mut *p };

        if !accept_fds.initiated {
            accept_fds.initiated = true;
            let fd = accept_fds.fd;
            let ioc_state = unsafe { &mut *self.ex.get_state() };
            accept_fds.task = Some(ioc_state.task_ctx.unwrap());

            let ring = unsafe { (*self.ex.get_state()).ring };
            let sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };
            let user_data = self.fds.clone().into_raw().cast::<libc::c_void>();

            let (addr, addrlen) = match &mut accept_fds.op {
                fiona::op::Op::Accept(ref mut s) => {
                    (std::ptr::addr_of_mut!(s.addr_in), std::ptr::addr_of_mut!(s.addr_len))
                }
                _ => internal_error("incorrect op type specified for the AcceptFuture"),
            };

            unsafe { fiona::liburing::io_uring_sqe_set_data(sqe, user_data) };
            unsafe {
                fiona::liburing::io_uring_prep_accept(sqe, fd, addr.cast::<libc::sockaddr>(), addrlen, 0);
            }
            unsafe { fiona::liburing::io_uring_submit(ring) };
            return std::task::Poll::Pending;
        }

        if !accept_fds.done {
            return std::task::Poll::Pending;
        }

        let fd = accept_fds.res;
        if fd < 0 {
            std::task::Poll::Ready(Err(unsafe { std::mem::transmute(-fd) }))
        } else {
            // let addr = match &mut accept_fds.op {
            //   fiona::op::Op::Accept(ref mut s) => s.addr_in,
            //   _ => internal_error("incorrect op type specified for the AcceptFuture"),
            // };
            // println!(
            //   "accepted tcp connection on this addr: {:?}:{}",
            //   addr.sin_addr.s_addr.to_le_bytes(),
            //   addr.sin_port.to_be()
            // );
            std::task::Poll::Ready(Ok(unsafe {
                Server {
                    s: Socket::from_raw(self.ex.clone(), fd),
                }
            }))
        }
    }
}

impl<'a> std::future::Future for ConnectFuture<'a> {
    type Output = Result<(), fiona::Errno>;
    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let p = self.connect_fds.get();
        let connect_fds = unsafe { &mut *p };

        if connect_fds.done {
            match connect_fds.op {
                fiona::op::Op::Connect(ref mut s) => {
                    s.timer_fds = None;
                }
                _ => internal_error(""),
            }

            if connect_fds.res < 0 {
                return std::task::Poll::Ready(Err(unsafe { std::mem::transmute(-connect_fds.res) }));
            }

            return std::task::Poll::Ready(Ok(()));
        }

        if connect_fds.initiated {
            return std::task::Poll::Pending;
        }

        let ioc_state = unsafe { &mut *self.ex.get_state() };
        connect_fds.task = Some(ioc_state.task_ctx.unwrap());

        let ring = self.ex.get_ring();

        let connect_sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };

        let (addr, addrlen) = match connect_fds.op {
            fiona::op::Op::Connect(ref s) => (
                std::ptr::addr_of!(s.addr_storage),
                std::mem::size_of::<libc::sockaddr_in>() as u32,
            ),
            _ => internal_error(""),
        };

        let user_data = self.connect_fds.clone().into_raw().cast::<libc::c_void>();

        unsafe {
            fiona::liburing::io_uring_sqe_set_data(connect_sqe, user_data);
        }
        unsafe {
            fiona::liburing::io_uring_prep_connect(connect_sqe, connect_fds.fd, addr.cast::<libc::sockaddr>(), addrlen);
        }

        unsafe {
            fiona::liburing::io_uring_sqe_set_flags(connect_sqe, 1_u32 << 2);
        }

        let mut ts = make_kernel_timespec(self.timeout);
        let timeout_sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };
        unsafe {
            fiona::liburing::io_uring_prep_link_timeout(timeout_sqe, &mut ts, 0);
            fiona::liburing::io_uring_sqe_set_data(timeout_sqe, std::ptr::null_mut());
        }

        unsafe {
            fiona::liburing::io_uring_submit(ring);
        }

        connect_fds.initiated = true;

        std::task::Poll::Pending
    }
}

impl<'a> std::future::Future for ReadFuture<'a> {
    type Output = Result<Vec<u8>, fiona::Errno>;

    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let p = self.read_fds.get();
        let read_fds = unsafe { &mut *p };

        if read_fds.done {
            if read_fds.res < 0 {
                return std::task::Poll::Ready(Err(unsafe { std::mem::transmute(-read_fds.res) }));
            }

            let mut buf = match read_fds.op {
                fiona::op::Op::Read(ref mut s) => s.buf.take().unwrap(),
                _ => internal_error("Read op was completed but the internal Op is an invalid type"),
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
        let read_sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };

        let (buf, nbytes, offset) = match read_fds.op {
            fiona::op::Op::Read(ref mut s) => match s.buf {
                Some(ref mut b) => (b.as_mut_ptr(), b.capacity(), b.len()),
                _ => internal_error(""),
            },
            _ => internal_error(""),
        };

        let user_data = self.read_fds.clone().into_raw().cast::<libc::c_void>();

        unsafe { fiona::liburing::io_uring_sqe_set_data(read_sqe, user_data) };

        unsafe {
            fiona::liburing::io_uring_prep_read(
                read_sqe,
                sockfd,
                buf.cast::<libc::c_void>(),
                nbytes as u32,
                offset as u64,
            );
        }

        unsafe {
            fiona::liburing::io_uring_sqe_set_flags(read_sqe, 1_u32 << 2);
        }

        let mut ts = make_kernel_timespec(self.timeout);
        let timeout_sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };
        unsafe {
            fiona::liburing::io_uring_prep_link_timeout(timeout_sqe, &mut ts, 0);
            fiona::liburing::io_uring_sqe_set_data(timeout_sqe, std::ptr::null_mut());
        }

        unsafe { fiona::liburing::io_uring_submit(ring) };

        read_fds.initiated = true;

        std::task::Poll::Pending
    }
}

impl<'a> std::future::Future for WriteFuture<'a> {
    type Output = Result<Vec<u8>, fiona::Errno>;
    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let p = self.write_fds.get();
        let fds = unsafe { &mut *p };

        if fds.done {
            if fds.res < 0 {
                return std::task::Poll::Ready(Err(unsafe { std::mem::transmute(-fds.res) }));
            }

            let buf = match fds.op {
                fiona::op::Op::Write(ref mut s) => s.buf.take().unwrap(),
                _ => internal_error("Write op was completed but the internal Op is an invalid type"),
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
        let write_sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };

        let (buf, nbytes, offset) = match fds.op {
            fiona::op::Op::Write(ref mut s) => match s.buf {
                Some(ref mut b) => (b.as_ptr(), b.len(), 0_u64),
                _ => internal_error("In WriteFuture, buf was null when it should've been Some"),
            },
            _ => internal_error("Incorrect operation type in WriteFuture"),
        };

        let user_data = self.write_fds.clone().into_raw().cast::<libc::c_void>();

        unsafe { fiona::liburing::io_uring_sqe_set_data(write_sqe, user_data) };

        unsafe {
            fiona::liburing::io_uring_prep_write(write_sqe, sockfd, buf.cast::<libc::c_void>(), nbytes as u32, offset);
        }

        unsafe {
            fiona::liburing::io_uring_sqe_set_flags(write_sqe, 1_u32 << 2);
        }

        let mut ts = make_kernel_timespec(self.timeout);
        let timeout_sqe = unsafe { fiona::liburing::io_uring_get_sqe(ring) };
        unsafe {
            fiona::liburing::io_uring_prep_link_timeout(timeout_sqe, &mut ts, 0);
            fiona::liburing::io_uring_sqe_set_data(timeout_sqe, std::ptr::null_mut());
        }

        unsafe { fiona::liburing::io_uring_submit(ring) };
        fds.initiated = true;
        std::task::Poll::Pending
    }
}

impl Drop for Acceptor {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe { libc::close(self.fd) };
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if self.fd != -1 {
            unsafe { libc::close(self.fd) };
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

fn make_kernel_timespec(t: std::time::Duration) -> fiona::libc::kernel_timespec {
    let (tv_sec, tv_nsec) = (
        i64::try_from(t.as_secs()).unwrap(),
        i64::try_from(t.subsec_nanos()).unwrap(),
    );
    fiona::libc::kernel_timespec { tv_sec, tv_nsec }
}

unsafe fn drop_cancel(p: *mut fiona::op::FdStateImpl, ioc: *mut fiona::IoContextState) {
    debug_assert!(!p.is_null());
    debug_assert!(!ioc.is_null());

    if (*p).initiated && !(*p).done {
        let ring = (*ioc).ring;
        unsafe {
            let sqe = fiona::liburing::io_uring_get_sqe(ring);
            fiona::liburing::io_uring_prep_cancel(sqe, p.cast::<libc::c_void>(), 0);
            fiona::liburing::io_uring_submit(ring);
        }
    }
}

fn internal_error(msg: &'static str) -> ! {
    panic!("{msg}");
}
