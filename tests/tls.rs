#![allow(unused_imports, dead_code)]

extern crate fiona;
extern crate libc;
extern crate rustls;
extern crate rustls_pemfile;
extern crate webpki_roots;

use std::{io::Read, sync::Arc};

use fiona as fio;

const LOCALHOST: u32 = 0x7f000001;

static mut PORT: std::sync::atomic::AtomicU16 =
  std::sync::atomic::AtomicU16::new(4300);

fn get_port() -> u16 {
  unsafe { PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
}

fn read_file(path: &str) -> Vec<u8> {
  let mut f = std::fs::File::open(path).unwrap();
  let len = f.metadata().unwrap().len();

  let mut buf = vec![0_u8; len as usize];
  f.read_exact(&mut buf).unwrap();

  buf
}

fn make_root_cert_store() -> rustls::RootCertStore {
  let mut root_store = rustls::RootCertStore::empty();
  root_store.add_server_trust_anchors(
    webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|root_ca| {
      rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
        root_ca.subject,
        root_ca.spki,
        root_ca.name_constraints,
      )
    }),
  );

  let certs = read_file("tests/ca.crt");
  let mut certs = rustls_pemfile::certs(&mut &certs[..]).unwrap();
  assert_eq!(certs.len(), 1);

  let test_ca = std::mem::take(&mut certs[0]);
  let test_crt = rustls::Certificate(test_ca);

  root_store.add(&test_crt).unwrap();

  root_store
}

fn make_tls_client_cfg() -> rustls::ClientConfig {
  let root_certs = make_root_cert_store();

  rustls::ClientConfig::builder()
    .with_safe_defaults()
    .with_root_certificates(root_certs)
    .with_no_client_auth()
}

fn make_tls_server_cfg() -> rustls::ServerConfig {
  let buf = read_file("tests/server.crt");

  let certs = rustls_pemfile::certs(&mut &buf[..]).unwrap();
  assert_eq!(certs.len(), 1);

  let cert_chain: Vec<rustls::Certificate> =
    certs.into_iter().map(rustls::Certificate).collect();

  let buf = read_file("tests/server.key");
  let mut keys = rustls_pemfile::pkcs8_private_keys(&mut &buf[..]).unwrap();
  assert_eq!(keys.len(), 1);

  let key_der = rustls::PrivateKey(std::mem::take(&mut keys[0]));

  rustls::ServerConfig::builder()
    .with_safe_defaults()
    .with_no_client_auth()
    .with_single_cert(cert_chain, key_der)
    .unwrap()
}

#[test]
fn tls_test() {
  use std::io::Write;

  let client_cfg = std::sync::Arc::new(make_tls_client_cfg());

  let mut client = rustls::ClientConnection::new(
    client_cfg,
    rustls::ServerName::try_from("localhost").unwrap(),
  )
  .unwrap();

  // client
  //   .writer()
  //   .write_all(b"I bestow the heads of virgins and the first-born sons!")
  //   .unwrap();

  assert!(client.wants_write());
  assert!(!client.wants_read());

  let mut net_buf = Vec::<u8>::new();
  let n = client.write_tls(&mut net_buf).unwrap();

  assert!(!client.wants_write());
  assert!(client.wants_read());
  assert!(n > 0);

  let server_cfg = std::sync::Arc::new(make_tls_server_cfg());

  let mut server = rustls::ServerConnection::new(server_cfg).unwrap();
  assert!(server.wants_read());
  assert!(!server.wants_write());

  let mut server_buf = Vec::<u8>::new();
  server.read_tls(&mut &net_buf[..]).unwrap();
  server.process_new_packets().unwrap();
  match server
    .reader()
    .read_to_end(&mut server_buf)
    .unwrap_err()
    .kind()
  {
    std::io::ErrorKind::WouldBlock => {}
    _ => panic!(""),
  }

  assert!(client.is_handshaking());

  assert!(!server.wants_read());
  assert!(server.wants_write());
  assert!(server.is_handshaking());

  server_buf.clear();
  server.write_tls(&mut server_buf).unwrap();

  assert!(!client.wants_write());
  assert!(client.wants_read());

  net_buf = server_buf.clone();
  client.read_tls(&mut &net_buf[..]).unwrap();
  client.process_new_packets().unwrap();

  assert!(client.wants_write());
  assert!(client.wants_read());
  assert!(!client.is_handshaking());

  net_buf.clear();
  client.write_tls(&mut net_buf).unwrap();
  server_buf = net_buf.clone();
  net_buf.clear();

  server.read_tls(&mut &server_buf[..]).unwrap();
  server.process_new_packets().unwrap();

  assert!(!server.is_handshaking());

  for _i in 0..5 {
    client
      .writer()
      .write_all(b"I bestow the heads of virgins and the first-born sons!")
      .unwrap();

    net_buf.clear();
    client.write_tls(&mut net_buf).unwrap();
    // println!("{:?}", &net_buf[0..n]);
    assert!(!client.wants_write());

    server_buf = net_buf.clone();
    net_buf.clear();

    server.read_tls(&mut &server_buf[..]).unwrap();
    server.process_new_packets().unwrap();

    server_buf.clear();
    server.reader().read_to_end(&mut server_buf).unwrap_err();

    assert_eq!(
      "I bestow the heads of virgins and the first-born sons!",
      std::str::from_utf8(&server_buf).unwrap()
    );

    server
      .writer()
      .write_all(b"... within these monuments of stone...")
      .unwrap();

    server_buf.clear();
    server.write_tls(&mut server_buf).unwrap();

    net_buf = server_buf.clone();
    server_buf.clear();
    // println!("{:?}", &net_buf[..]);

    client.read_tls(&mut &net_buf[..]).unwrap();
    let state = client.process_new_packets().unwrap();
    assert!(!state.peer_has_closed());
    assert!(state.plaintext_bytes_to_read() > 0);

    net_buf.clear();
    client.reader().read_to_end(&mut net_buf).unwrap_err();
    assert_eq!(
      "... within these monuments of stone...",
      std::str::from_utf8(&net_buf).unwrap()
    );
  }

  client.send_close_notify();
  net_buf.clear();
  client.write_tls(&mut net_buf).unwrap();

  server_buf = net_buf.clone();
  net_buf.clear();

  server.read_tls(&mut &server_buf[..]).unwrap();
  let state = server.process_new_packets().unwrap();
  assert!(state.peer_has_closed());

  server_buf.clear();
  server.reader().read_to_end(&mut server_buf).unwrap();

  server.send_close_notify();
  assert!(server.wants_write());
  server_buf.clear();
  server.write_tls(&mut server_buf).unwrap();

  net_buf = server_buf.clone();
  server_buf.clear();

  client.read_tls(&mut &net_buf[..]).unwrap();
  client.process_new_packets().unwrap();
  net_buf.clear();
  client.reader().read_to_end(&mut net_buf).unwrap();

  assert!(!client.wants_read());
  assert!(!client.wants_write());

  assert!(!server.wants_read());
  assert!(!server.wants_write());
}

#[test]
fn tls_server_test() {
  use std::io::Write;

  static mut NUM_RUNS: i32 = 0;

  let server_port = get_port();

  fn server(
    ex: &fio::Executor,
    port: u16,
  ) -> impl std::future::Future<Output = ()> {
    let ex = ex.clone();
    let server_cfg = std::sync::Arc::new(make_tls_server_cfg());

    async move {
      let mut acceptor = fio::ip::tcp::Acceptor::new(&ex);
      acceptor.listen(LOCALHOST, port).unwrap();

      let mut peer = acceptor.async_accept().await.unwrap();
      peer.timeout = std::time::Duration::from_secs(1);

      let mut tls_stream = rustls::ServerConnection::new(server_cfg).unwrap();
      assert!(tls_stream.is_handshaking());

      let mut buf = vec![0_u8; 1024 * 1024];
      unsafe {
        buf.set_len(0);
      }
      let mut buf = peer.async_read(buf).await.unwrap();

      // beginning of the TLS handshake
      assert!(!buf.is_empty());

      assert!(tls_stream.wants_read());
      assert!(!tls_stream.wants_write());

      // read Client Hello
      tls_stream.read_tls(&mut &buf[..]).unwrap();
      let info = tls_stream.process_new_packets().unwrap();

      assert!(tls_stream.is_handshaking());
      assert_eq!(info.plaintext_bytes_to_read(), 0);

      // write Server Hello
      buf.clear();
      tls_stream.write_tls(&mut buf).unwrap();
      assert!(tls_stream.is_handshaking());

      let mut buf = peer.async_write(buf).await.unwrap();

      // at this stage, TLS should be complete once we read in the remaining
      // portion of the handshake
      // application data should _not_ be mixed in here
      buf.clear();
      let mut buf = peer.async_read(buf).await.unwrap();

      tls_stream.read_tls(&mut &buf[..]).unwrap();
      let info = tls_stream.process_new_packets().unwrap();

      assert!(!tls_stream.is_handshaking());
      assert_eq!(info.plaintext_bytes_to_read(), 0);

      // now we're ready for application data
      buf.clear();
      let mut buf = peer.async_read(buf).await.unwrap();

      tls_stream.read_tls(&mut &buf[..]).unwrap();
      let info = tls_stream.process_new_packets().unwrap();

      let n = info.plaintext_bytes_to_read();
      assert!(n > 0);
      assert_eq!(n, 11);

      buf.clear();
      match tls_stream
        .reader()
        .read_to_end(&mut buf)
        .unwrap_err()
        .kind()
      {
        std::io::ErrorKind::WouldBlock => {}
        _ => panic!(),
      }
      assert_eq!("I bestow...", std::str::from_utf8(&buf).unwrap());

      buf.clear();
      let mut buf = peer.async_read(buf).await.unwrap();
      tls_stream.read_tls(&mut &buf[..]).unwrap();
      let info = tls_stream.process_new_packets().unwrap();

      info.tls_bytes_to_write();
      assert!(info.peer_has_closed());
      assert!(info.tls_bytes_to_write() > 0);

      buf.clear();
      tls_stream.send_close_notify();
      tls_stream.write_tls(&mut buf).unwrap();
      peer.async_write(buf).await.unwrap();

      assert!(!tls_stream.wants_write());
      assert!(!tls_stream.wants_read());

      unsafe { NUM_RUNS += 1 };
    }
  }

  fn client(
    ex: &fio::Executor,
    port: u16,
  ) -> impl std::future::Future<Output = ()> {
    let ex = ex.clone();
    let client_cfg = std::sync::Arc::new(make_tls_client_cfg());

    async move {
      let mut client = fio::ip::tcp::Client::new(&ex);
      client.timeout = std::time::Duration::from_secs(1);
      client.async_connect(LOCALHOST, port).await.unwrap();

      let mut buf = vec![0_u8; 1024 * 1024];
      unsafe {
        buf.set_len(0);
      }

      let mut tls_stream = rustls::ClientConnection::new(
        client_cfg,
        rustls::ServerName::try_from("localhost").unwrap(),
      )
      .unwrap();

      tls_stream.write_tls(&mut buf).unwrap();
      let mut buf = client.async_write(buf).await.unwrap();

      assert!(tls_stream.is_handshaking());

      buf.clear();
      let mut buf = client.async_read(buf).await.unwrap();

      tls_stream.read_tls(&mut &buf[..]).unwrap();
      let info = tls_stream.process_new_packets().unwrap();

      assert!(!tls_stream.is_handshaking());
      assert_eq!(info.plaintext_bytes_to_read(), 0);

      buf.clear();
      tls_stream.write_tls(&mut buf).unwrap();

      let mut buf = client.async_write(buf).await.unwrap();

      assert!(!tls_stream.is_handshaking());

      tls_stream.writer().write_all(b"I bestow...").unwrap();

      buf.clear();
      tls_stream.write_tls(&mut buf).unwrap();

      let mut buf = client.async_write(buf).await.unwrap();
      buf.clear();

      tls_stream.send_close_notify();
      tls_stream.write_tls(&mut buf).unwrap();

      let mut buf = client.async_write(buf).await.unwrap();

      buf.clear();
      let buf = client.async_read(buf).await.unwrap();

      tls_stream.read_tls(&mut &buf[..]).unwrap();
      let info = tls_stream.process_new_packets().unwrap();

      assert!(info.peer_has_closed());
      assert_eq!(info.tls_bytes_to_write(), 0);

      assert!(!tls_stream.wants_read());
      assert!(!tls_stream.wants_write());

      unsafe { NUM_RUNS += 1 };
    }
  }

  let mut ioc = fio::IoContext::new();
  let ex = ioc.get_executor();
  ioc.post(server(&ex, server_port));
  ioc.post(client(&ex, server_port));
  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 2);
}

#[test]
fn test_async_handshake() {
  use std::io::Write;

  static mut NUM_RUNS: i32 = 0;

  let server_port = get_port();

  fn server(
    ex: &fio::Executor,
    port: u16,
  ) -> impl std::future::Future<Output = ()> {
    let ex = ex.clone();
    let server_cfg = std::sync::Arc::new(make_tls_server_cfg());

    async move {
      let mut acceptor = fio::ip::tcp::Acceptor::new(&ex);
      acceptor.listen(LOCALHOST, port).unwrap();

      let mut peer = acceptor.async_accept().await.unwrap();
      peer.timeout = std::time::Duration::from_secs(1);

      let mut buf = vec![0_u8; 16];
      unsafe {
        buf.set_len(0);
      }

      let mut tls_server = fio::ip::tcp::tls::Server::new(peer, server_cfg);
      let _buf = tls_server.async_handshake(buf).await.unwrap();

      unsafe { NUM_RUNS += 1 };
    }
  }

  fn client(
    ex: &fio::Executor,
    port: u16,
  ) -> impl std::future::Future<Output = ()> {
    let ex = ex.clone();
    let client_cfg = std::sync::Arc::new(make_tls_client_cfg());

    async move {
      let mut buf = vec![0_u8; 16];
      unsafe {
        buf.set_len(0);
      }

      let mut tls_client = fio::ip::tcp::tls::Client::new(&ex, client_cfg);

      let server_name = "localhost";
      let ipv4_addr = LOCALHOST;

      let _buf = tls_client
        .async_connect(server_name, ipv4_addr, port, buf)
        .await
        .unwrap();

      unsafe { NUM_RUNS += 1 };
    }
  }

  let mut ioc = fio::IoContext::new();
  let ex = ioc.get_executor();
  ioc.post(server(&ex, server_port));
  ioc.post(client(&ex, server_port));
  ioc.run();

  assert_eq!(unsafe { NUM_RUNS }, 2);
}
