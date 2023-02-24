extern crate rustls;

use std::io::{Read, Write};

use crate as fiona;

pub struct Client {
  s: fiona::ip::tcp::Client,
  connected: bool,
  tls_stream: Option<rustls::ClientConnection>,
  client_cfg: Option<std::sync::Arc<rustls::ClientConfig>>,
}

pub struct Server {
  s: fiona::ip::tcp::Server,
  tls_stream: Option<rustls::ServerConnection>,
  server_cfg: Option<std::sync::Arc<rustls::ServerConfig>>,
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

  pub async fn async_connect<'a>(
    &'a mut self,
    server_name: &'a str,
    ipv4_addr: u32,
    port: u16,
    buf: Vec<u8>,
  ) -> Result<Vec<u8>, i32> {
    assert!(!self.connected);
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

    self.connected = true;

    Ok(buf)
  }

  fn tls_stream(&mut self) -> &mut rustls::ClientConnection {
    self.tls_stream.as_mut().unwrap()
  }

  pub async fn async_read(&mut self, buf: Vec<u8>) -> Result<Vec<u8>, i32> {
    assert!(self.connected);

    let mut buf = self.s.async_read(buf).await?;

    let tls = self.tls_stream();
    tls.read_tls(&mut &buf[..]).unwrap();
    tls.process_new_packets().unwrap();

    buf.clear();
    tls.reader().read_to_end(&mut buf).unwrap_err();

    Ok(buf)
  }

  pub async fn async_write(
    &mut self,
    mut buf: Vec<u8>,
  ) -> Result<Vec<u8>, i32> {
    let tls = self.tls_stream();

    tls.writer().write_all(&buf).unwrap();

    buf.clear();
    tls.write_tls(&mut buf).unwrap();

    let buf = self.s.async_write(buf).await?;

    Ok(buf)
  }
}

impl Server {
  #[must_use]
  pub fn new(
    s: fiona::ip::tcp::Server,
    server_cfg: std::sync::Arc<rustls::ServerConfig>,
  ) -> Self {
    Self {
      s,
      server_cfg: Some(server_cfg),
      tls_stream: None,
    }
  }

  pub async fn async_handshake(
    &mut self,
    buf: Vec<u8>,
  ) -> Result<Vec<u8>, i32> {
    async fn async_server_handshake_impl(
      s: &mut fiona::ip::tcp::Socket,
      tls_stream: &mut rustls::ServerConnection,
      mut buf: Vec<u8>,
    ) -> Vec<u8> {
      assert!(tls_stream.is_handshaking());

      while tls_stream.is_handshaking() {
        if tls_stream.wants_read() {
          buf.clear();
          buf = s.async_read(buf).await.unwrap();
          assert!(!buf.is_empty());

          tls_stream.read_tls(&mut &buf[..]).unwrap();
          tls_stream.process_new_packets().unwrap();
        }

        if tls_stream.wants_write() {
          buf.clear();
          tls_stream.write_tls(&mut buf).unwrap();
          buf = s.async_write(buf).await.unwrap();
        }
      }

      buf
    }

    self.tls_stream = Some(
      rustls::ServerConnection::new(self.server_cfg.take().unwrap()).unwrap(),
    );

    let buf = async_server_handshake_impl(
      &mut self.s,
      self.tls_stream.as_mut().unwrap(),
      buf,
    )
    .await;

    Ok(buf)
  }

  fn tls_stream(&mut self) -> &mut rustls::ServerConnection {
    self.tls_stream.as_mut().unwrap()
  }

  pub async fn async_read(&mut self, buf: Vec<u8>) -> Result<Vec<u8>, i32> {
    let mut buf = self.s.async_read(buf).await?;

    let tls = self.tls_stream();
    tls.read_tls(&mut &buf[..]).unwrap();
    tls.process_new_packets().unwrap();

    buf.clear();
    tls.reader().read_to_end(&mut buf).unwrap_err();

    Ok(buf)
  }

  pub async fn async_write(
    &mut self,
    mut buf: Vec<u8>,
  ) -> Result<Vec<u8>, i32> {
    let tls = self.tls_stream();

    tls.writer().write_all(&buf).unwrap();

    buf.clear();
    tls.write_tls(&mut buf).unwrap();

    let buf = self.s.async_write(buf).await?;

    Ok(buf)
  }
}
