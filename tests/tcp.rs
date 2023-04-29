extern crate fiona;

use std::future::Future;

static mut PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(3300);

fn get_port() -> u16 {
    unsafe { PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
}

const LOCALHOST: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

const LOCALHOST_IPV6: std::net::IpAddr = std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST);

struct NopWaker {}
impl std::task::Wake for NopWaker {
    fn wake(self: std::sync::Arc<Self>) {}
}

#[test]
fn kernel_timespec_ffi_check() {
    let timespec = fiona::libc::kernel_timespec {
        tv_nsec: 1337,
        tv_sec: 7331,
    };

    let t2 = unsafe { fiona::libc::rio_timespec_test(timespec) };

    assert_eq!(t2.tv_sec, timespec.tv_sec);
    assert_eq!(t2.tv_nsec, timespec.tv_nsec);
}

#[test]
fn tcp_acceptor() {
    static mut NUM_RUNS: i32 = 0;

    let port = get_port();

    async fn server(ex: fiona::Executor, port: u16) {
        let mut acceptor = fiona::ip::tcp::Acceptor::new(&ex);
        acceptor.listen(LOCALHOST, port).unwrap();
        let mut stream = acceptor.async_accept().await.unwrap();

        let mut buf = vec![0_u8; 4096];
        unsafe {
            buf.set_len(0);
        }

        let buf = stream.async_read(buf).await.unwrap();

        assert_eq!(buf.len(), 13);
        let str = unsafe { std::str::from_utf8_unchecked(&buf[0..buf.len()]) };
        assert_eq!(str, "Hello, world!");

        unsafe { NUM_RUNS += 1 };
    }

    async fn client(ex: fiona::Executor, port: u16) {
        let mut client = fiona::ip::tcp::Client::new(&ex);
        client.async_connect(LOCALHOST, port).await.unwrap();

        let str = String::from("Hello, world!").into_bytes();
        client.async_write(str).await.unwrap();

        unsafe { NUM_RUNS += 1 };
    }

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    ioc.post(server(ex.clone(), port));
    ioc.post(client(ex, port));
    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn multi_accept() {
    static mut NUM_RUNS: i32 = 0;
    const MAX_CONNS: i32 = 100;

    let port = get_port();

    async fn server(mut ex: fiona::Executor, port: u16) {
        let mut acceptor = fiona::ip::tcp::Acceptor::new(&ex);
        acceptor.listen(LOCALHOST, port).unwrap();

        let mut num_conns = 0_i32;
        while num_conns < MAX_CONNS {
            let mut stream = acceptor.async_accept().await.unwrap();

            ex.post(async move {
                let mut buf = vec![0_u8; 4096];
                unsafe {
                    buf.set_len(0);
                }

                let buf = stream.async_read(buf).await.unwrap();

                assert_eq!(buf.len(), 13);
                let str = unsafe { std::str::from_utf8_unchecked(&buf[0..buf.len()]) };
                assert_eq!(str, "Hello, world!");
            });

            num_conns += 1;
        }

        unsafe { NUM_RUNS += 1 };
    }

    async fn client(mut ex: fiona::Executor, port: u16) {
        for _i in 0..MAX_CONNS {
            ex.post({
                let ex = ex.clone();
                async move {
                    let mut client = fiona::ip::tcp::Client::new(&ex);

                    client.async_connect(LOCALHOST, port).await.unwrap();

                    let str = String::from("Hello, world!").into_bytes();

                    client.async_write(str).await.unwrap();
                }
            });
        }

        unsafe { NUM_RUNS += 1 };
    }

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    ioc.post(server(ex.clone(), port));
    ioc.post(client(ex, port));
    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn econnrefused_connect_future() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    let mut timer = fiona::time::Timer::new(&ex);
    ioc.post(async move {
        timer.expires_after(std::time::Duration::from_millis(1500));
        timer.async_wait().await.unwrap();

        unsafe { NUM_RUNS += 1 };
    });

    let mut client = fiona::ip::tcp::Client::new(&ex);
    ioc.post(async move {
        let r = client.async_connect(LOCALHOST, get_port()).await;

        match r {
            Err(e) => match e {
                fiona::Errno::ECONNREFUSED => {}
                _ => panic!("incorrect errno value, should be ECONNREFUSED"),
            },
            _ => panic!("expected an error when connecting"),
        }

        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();
    assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn connect_timeout() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    let mut client = fiona::ip::tcp::Client::new(&ex);
    client.timeout = std::time::Duration::from_secs(2);
    ioc.post(async move {
        // use one of the IP addresses from the test networks:
        // 192.0.2.0/24
        // https://en.wikipedia.org/wiki/Internet_Protocol_version_4#Special-use_addresses
        let r = client
            .async_connect(std::net::IpAddr::V4(std::net::Ipv4Addr::from([192, 0, 2, 0])), 3301)
            .await;

        match r {
            Err(e) => match e {
                fiona::Errno::ECANCELED => {}
                _ => panic!("incorrect errno value, should be ECANCELED"),
            },
            _ => panic!("expected an error when connecting"),
        }

        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();
    assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
fn read_timeout() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    let port = get_port();

    let mut acceptor = fiona::ip::tcp::Acceptor::new(&ex);
    acceptor.listen(LOCALHOST, port).unwrap();

    let mut client = fiona::ip::tcp::Client::new(&ex);

    ioc.post(async move {
        let _s = acceptor.async_accept().await.unwrap();
        let mut timer = fiona::time::Timer::new(&ex);
        timer.expires_after(std::time::Duration::from_secs(2));
        timer.async_wait().await.unwrap();
    });

    ioc.post(async move {
        client.timeout = std::time::Duration::from_secs(1);
        client.async_connect(LOCALHOST, port).await.unwrap();

        let mut buf = vec![0_u8; 128];
        unsafe {
            buf.set_len(0);
        }

        let r = client.async_read(buf).await.unwrap_err();
        match r {
            fiona::Errno::ECANCELED => {}
            _ => panic!(""),
        };

        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();
    assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
fn drop_accept_pending() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    ioc.post(async move {
        let mut acceptor = fiona::ip::tcp::Acceptor::new(&ex);
        acceptor.listen(LOCALHOST, get_port()).unwrap();
        let mut f = acceptor.async_accept();

        let waker = std::sync::Arc::new(NopWaker {}).into();
        let mut cx = std::task::Context::from_waker(&waker);

        assert!(unsafe { std::pin::Pin::new_unchecked(&mut f).poll(&mut cx) }.is_pending());

        let mut timer = fiona::time::Timer::new(&ex);
        timer.expires_after(std::time::Duration::from_millis(500));
        timer.async_wait().await.unwrap();

        std::mem::drop(f);

        timer.async_wait().await.unwrap();

        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();
    assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
fn cancel_accept() {
    static mut NUM_RUNS: i32 = 0;

    let mut ioc = fiona::IoContext::new();
    let mut ex = ioc.get_executor();

    ioc.post(async move {
        let mut acceptor = fiona::ip::tcp::Acceptor::new(&ex);
        acceptor.listen(LOCALHOST, get_port()).unwrap();
        let f = acceptor.async_accept();
        let c = f.get_cancel_handle();

        ex.post({
            let ex = ex.clone();
            async move {
                let mut timer = fiona::time::Timer::new(&ex);
                timer.expires_after(std::time::Duration::from_millis(500));
                timer.async_wait().await.unwrap();

                c.cancel();
            }
        });

        let result = f.await;
        match result {
            Ok(_) => panic!("Ok is not valid for cancellation"),
            Err(err) => match err {
                fiona::Errno::ECANCELED => {}
                _ => panic!("incorrect errno value"),
            },
        }

        unsafe { NUM_RUNS += 1 };
    });

    ioc.run();
    assert_eq!(unsafe { NUM_RUNS }, 1);
}

#[test]
#[ignore]
#[should_panic]
fn tcp_acceptor_ipv6() {
    static mut NUM_RUNS: i32 = 0;

    let port = get_port();

    async fn server(ex: fiona::Executor, port: u16) {
        let mut acceptor = fiona::ip::tcp::Acceptor::new(&ex);
        acceptor.listen(LOCALHOST_IPV6, port).unwrap();
        let mut stream = acceptor.async_accept().await.unwrap();

        let mut buf = vec![0_u8; 4096];
        unsafe {
            buf.set_len(0);
        }

        let buf = stream.async_read(buf).await.unwrap();

        assert_eq!(buf.len(), 13);
        let str = unsafe { std::str::from_utf8_unchecked(&buf[0..buf.len()]) };
        assert_eq!(str, "Hello, world!");

        unsafe { NUM_RUNS += 1 };
    }

    async fn client(ex: fiona::Executor, port: u16) {
        let mut client = fiona::ip::tcp::Client::new(&ex);
        client.async_connect(LOCALHOST_IPV6, port).await.unwrap();

        let str = String::from("Hello, world!").into_bytes();
        client.async_write(str).await.unwrap();

        unsafe { NUM_RUNS += 1 };
    }

    let mut ioc = fiona::IoContext::new();
    let ex = ioc.get_executor();

    ioc.post(server(ex.clone(), port));
    ioc.post(client(ex, port));
    ioc.run();

    assert_eq!(unsafe { NUM_RUNS }, 2);
}
