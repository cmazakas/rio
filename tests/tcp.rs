extern crate rio;

#[test]
fn sockaddr_in_ffi_check() {
  let addr = rio::ip::tcp::sockaddr_in::default();
  let _addr2 = unsafe { rio::libc::rio_sockaddr_in_test(addr) };
}

#[test]
fn tcp_acceptor() {
  static mut WAS_RUN: bool = false;

  let mut ioc = rio::IoContext::new();
  ioc.post({
    let ex = ioc.get_executor();
    Box::pin(async {
      let mut acceptor = rio::ip::tcp::Acceptor::new(ex);
      acceptor.listen(0x7f000001, 3300).unwrap();
      let fd = acceptor.async_accept().await.unwrap();
      println!("this is our client fd => {fd}");

      unsafe { WAS_RUN = true };
    })
  });

  // ioc.post({
  //   let ex = ioc.get_executor();
  //   Box::pin(async {
  //     // let mut client = rio::ip::tcp::Socket::new(ex);
  //     // client.async_connect();
  //   })
  // });

  ioc.run();
  assert!(unsafe { WAS_RUN });
}
