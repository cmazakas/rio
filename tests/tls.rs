#![allow(unused_imports, dead_code)]

extern crate fiona;
extern crate libc;
extern crate rustls;
extern crate rustls_pemfile;
extern crate webpki_roots;

use std::io::Read;

use fiona as fio;

const LOCALHOST: u32 = 0x7f000001;

static mut PORT: std::sync::atomic::AtomicU16 =
  std::sync::atomic::AtomicU16::new(3300);

fn get_port() -> u16 {
  unsafe { PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
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

  let mut certs = vec![0_u8; 4096];
  std::fs::File::open("tests/ca.crt")
    .unwrap()
    .read(&mut certs)
    .unwrap();

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
  let mut f = std::fs::File::open("tests/server.crt").unwrap();
  let len = f.metadata().unwrap().len();

  let mut buf = vec![0_u8; len as usize];
  f.read(&mut buf).unwrap();

  let certs = rustls_pemfile::certs(&mut &buf[..]).unwrap();
  assert_eq!(certs.len(), 1);

  let cert_chain: Vec<rustls::Certificate> =
    certs.into_iter().map(|b| rustls::Certificate(b)).collect();

  let mut f = std::fs::File::open("tests/server.key").unwrap();
  let len = f.metadata().unwrap().len();

  let mut buf = vec![0_u8; len as usize];
  f.read(&mut buf).unwrap();

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

  client
    .writer()
    .write(b"I bestow the heads of virgins and the first-born sons!")
    .unwrap();

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
  /* let n =  */
  server.reader().read(&mut server_buf[..]).unwrap();
  // println!("{:?}", &server_buf[0..n]);

  assert!(!server.wants_read());
  assert!(server.wants_write());

  server_buf.clear();
  server.write_tls(&mut server_buf).unwrap();

  assert!(!client.wants_write());
  assert!(client.wants_read());

  net_buf = server_buf.clone();
  while client.wants_read() {
    let n = client.read_tls(&mut &net_buf[..]).unwrap();
    // println!("{n}");
    if n == 0 {
      break;
    }
    client.process_new_packets().unwrap();
    net_buf.drain(0..n);
  }

  net_buf.clear();
  client.write_tls(&mut net_buf).unwrap();
  // println!("{:?}", &net_buf[0..n]);
  assert!(!client.wants_write());

  server.read_tls(&mut &net_buf[..]).unwrap();
  server.process_new_packets().unwrap();
  let n = server.reader().read(&mut server_buf[..]).unwrap();

  assert_eq!(
    "I bestow the heads of virgins and the first-born sons!",
    std::str::from_utf8(&server_buf[0..n]).unwrap()
  );
}

// #[test]
// fn tls_server_test() {
//   use std::io::Write;

//   static mut NUM_RUNS: i32 = 0;

//   let server_port = get_port();

//   fn server(
//     ex: &fio::Executor,
//     port: u16,
//   ) -> impl std::future::Future<Output = ()> {
//     let ex = ex.clone();
//     async move {
//       let mut acceptor = fio::ip::tcp::Acceptor::new(&ex);
//       acceptor.listen(LOCALHOST, port).unwrap();
//       let peer = acceptor.async_accept();

//       unsafe { NUM_RUNS += 1 };
//     }
//   }

//   let client_cfg = std::sync::Arc::new(make_tls_client_cfg());

//   fn client(
//     ex: &fio::Executor,
//     port: u16,
//     client_cfg: &std::sync::Arc<rustls::ClientConfig>,
//   ) -> impl std::future::Future<Output = ()> {
//     let ex = ex.clone();
//     let client_cfg = client_cfg.clone();

//     async move {
//       let mut client = rustls::ClientConnection::new(
//         client_cfg,
//         rustls::ServerName::try_from("localhost").unwrap(),
//       )
//       .unwrap();

//       client
//         .writer()
//         .write(b"I bestow the heads of virgins and first-born sons")
//         .unwrap();

//       unsafe { NUM_RUNS += 1 };
//     }
//   }

//   let mut ioc = fio::IoContext::new();
//   let ex = ioc.get_executor();
//   ioc.post(server(&ex, server_port));
//   ioc.post(client(&ex, server_port, &client_cfg));
//   ioc.run();

//   assert_eq!(unsafe { NUM_RUNS }, 2);
// }
