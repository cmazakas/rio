/// The `tcp` module contains classes for working with both client and server-oriented TCP/IP streams. It also provides
/// TLS and async DNS.
///
/// Example:
/// ```rust
/// const LOCALHOST: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
///
/// async fn server(ex: fiona::Executor, port: u16) {
///     let mut acceptor = fiona::ip::tcp::Acceptor::new(&ex);
///     acceptor.listen(LOCALHOST, port).unwrap();
///     let mut stream = acceptor.async_accept().await.unwrap();
///
///     let mut buf = Vec::<u8>::with_capacity(4096);
///
///     let buf = stream.async_read(buf).await.unwrap();
///
///     assert_eq!(buf.len(), 13);
///     let str = unsafe { std::str::from_utf8_unchecked(&buf[0..buf.len()]) };
///     assert_eq!(str, "Hello, world!");
/// }
///
/// async fn client(ex: fiona::Executor, port: u16) {
///     let mut client = fiona::ip::tcp::Client::new(&ex);
///     client.async_connect(LOCALHOST, port).await.unwrap();
///
///     let str = String::from("Hello, world!").into_bytes();
///     client.async_write(str).await.unwrap();
/// }
///
/// let mut ioc = fiona::IoContext::new();
/// let ex = ioc.get_executor();
///
/// ioc.post(server(ex.clone(), 3030));
/// ioc.post(client(ex, 3030));
/// ioc.run();
/// ```
pub mod tcp;
