extern crate rustls;

use crate as fiona;

pub struct Client {
  s: fiona::ip::tcp::Client,
  connected: bool,
  tls_stream: Option<rustls::ClientConnection>,
  client_cfg: Option<std::sync::Arc<rustls::ClientConfig>>,
}

async fn async_client_handshake_impl(
  s: &mut fiona::ip::tcp::Socket,
  tls_stream: &mut rustls::ClientConnection,
  mut buf: Vec<u8>,
) -> Vec<u8> {
  assert!(tls_stream.is_handshaking());

  while tls_stream.is_handshaking() {
    if tls_stream.wants_write() {
      buf.clear();
      tls_stream.write_tls(&mut buf).unwrap();
      buf = s.async_write(buf).await.unwrap();
      assert!(!buf.is_empty());
    }

    if tls_stream.wants_read() {
      buf.clear();
      buf = s.async_read(buf).await.unwrap();
      tls_stream.read_tls(&mut &buf[..]).unwrap();
      tls_stream.process_new_packets().unwrap();
    }
  }

  while tls_stream.wants_write() {
    buf.clear();
    tls_stream.write_tls(&mut buf).unwrap();
    buf = s.async_write(buf).await.unwrap();
    assert!(!buf.is_empty());
  }

  assert!(!tls_stream.is_handshaking());

  buf
}

impl Client {
  #[must_use]
  pub fn new(
    ex: &fiona::Executor,
    client_cfg: std::sync::Arc<rustls::ClientConfig>,
  ) -> Self {
    Self {
      s: fiona::ip::tcp::Client::new(ex),
      client_cfg: Some(client_cfg),
      connected: false,
      tls_stream: None,
    }
  }

  pub fn async_connect<'a>(
    &'a mut self,
    server_name: &'a str,
    ipv4_addr: u32,
    port: u16,
    buf: Vec<u8>,
  ) -> impl std::future::Future<Output = Result<Vec<u8>, i32>> + 'a {
    assert!(!self.connected);

    async move {
      self.s.async_connect(ipv4_addr, port).await?;

      self.tls_stream = Some(
        rustls::ClientConnection::new(
          self.client_cfg.take().unwrap(),
          rustls::ServerName::try_from(server_name).unwrap(),
        )
        .unwrap(),
      );

      let buf = async_client_handshake_impl(
        &mut self.s,
        self.tls_stream.as_mut().unwrap(),
        buf,
      )
      .await;

      Ok(buf)
    }
  }
}
