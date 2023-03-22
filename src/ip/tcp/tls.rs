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

macro_rules! tls_stream {
  ($x:ident) => {
    $x.tls_stream.as_mut().unwrap()
  };
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
  ) -> Result<Vec<u8>, fiona::Error> {
    async fn async_client_handshake_impl(
      s: &mut fiona::ip::tcp::Socket,
      tls_stream: &mut rustls::ClientConnection,
      mut buf: Vec<u8>,
    ) -> Result<Vec<u8>, fiona::Error> {
      assert!(tls_stream.is_handshaking());

      while tls_stream.is_handshaking() {
        if tls_stream.wants_write() {
          buf = async_write_tls_helper(s, tls_stream, buf).await?;
        }

        if tls_stream.wants_read() {
          buf.clear();
          buf = s.async_read(buf).await?;
          tls_stream.read_tls(&mut &buf[..])?;
          tls_stream.process_new_packets()?;
        }
      }

      buf = send_full_tls(s, tls_stream, buf).await?;
      assert!(!tls_stream.is_handshaking());

      Ok(buf)
    }

    assert!(!self.connected);
    self.s.async_connect(ipv4_addr, port).await?;

    self.tls_stream = Some(rustls::ClientConnection::new(
      self.client_cfg.take().unwrap(),
      rustls::ServerName::try_from(server_name).unwrap(),
    )?);

    let buf =
      async_client_handshake_impl(&mut self.s, tls_stream!(self), buf).await?;

    self.connected = true;

    Ok(buf)
  }

  pub async fn async_read(
    &mut self,
    buf: Vec<u8>,
  ) -> Result<Vec<u8>, fiona::Error> {
    assert!(self.connected);
    async_read_impl(&mut self.s, tls_stream!(self), buf).await
  }

  pub async fn async_write(
    &mut self,
    buf: Vec<u8>,
  ) -> Result<Vec<u8>, fiona::Error> {
    let tls = tls_stream!(self);
    async_write_impl(&mut self.s, tls, buf).await
  }

  pub async fn async_shutdown(
    &mut self,
    mut buf: Vec<u8>,
  ) -> Result<Vec<u8>, fiona::Error> {
    let tls = tls_stream!(self);

    tls.send_close_notify();

    buf.clear();
    tls.write_tls(&mut buf)?;

    buf = self.s.async_write(buf).await?;

    while tls.wants_read() {
      buf.clear();
      buf = self.s.async_read(buf).await?;

      tls.read_tls(&mut &buf[..])?;
      let info = tls.process_new_packets()?;
      assert!(tls.wants_read() || info.peer_has_closed());
    }

    Ok(buf)
  }
}

impl Server {
  fn tls_stream(&mut self) -> &mut rustls::ServerConnection {
    tls_stream!(self)
  }

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
  ) -> Result<Vec<u8>, fiona::Error> {
    async fn async_server_handshake_impl(
      s: &mut fiona::ip::tcp::Socket,
      tls_stream: &mut rustls::ServerConnection,
      mut buf: Vec<u8>,
    ) -> Result<Vec<u8>, fiona::Error> {
      assert!(tls_stream.is_handshaking());

      while tls_stream.is_handshaking() {
        if tls_stream.wants_read() {
          buf.clear();
          buf = s.async_read(buf).await?;
          assert!(!buf.is_empty());

          tls_stream.read_tls(&mut &buf[..])?;
          let _info = tls_stream.process_new_packets()?;
        }

        if tls_stream.wants_write() {
          buf = async_write_tls_helper(s, tls_stream, buf).await?;
        }
      }

      Ok(buf)
    }

    self.tls_stream = Some(rustls::ServerConnection::new(
      self.server_cfg.take().unwrap(),
    )?);

    let buf =
      async_server_handshake_impl(&mut self.s, tls_stream!(self), buf).await?;

    Ok(buf)
  }

  pub async fn async_read(
    &mut self,
    buf: Vec<u8>,
  ) -> Result<Vec<u8>, fiona::Error> {
    assert!(!self.tls_stream().is_handshaking());

    async_read_impl(&mut self.s, tls_stream!(self), buf).await
  }

  pub async fn async_write(
    &mut self,
    buf: Vec<u8>,
  ) -> Result<Vec<u8>, fiona::Error> {
    let tls = tls_stream!(self);
    async_write_impl(&mut self.s, tls, buf).await
  }
}

async fn async_write_impl<Data>(
  s: &mut fiona::ip::tcp::Socket,
  tls: &mut rustls::ConnectionCommon<Data>,
  mut buf: Vec<u8>,
) -> Result<Vec<u8>, fiona::Error> {
  assert!(!tls.is_handshaking());

  tls.writer().write_all(&buf)?;

  while tls.wants_write() {
    buf.clear();
    buf.resize(buf.capacity(), 0);
    let mut b = buf.as_mut_slice();

    let n = tls.write_tls(&mut b)?;
    unsafe {
      buf.set_len(n);
    }

    buf = s.async_write(buf).await?;
  }

  Ok(buf)
}

async fn async_write_tls_helper<Data>(
  s: &mut fiona::ip::tcp::Socket,
  tls: &mut rustls::ConnectionCommon<Data>,
  mut buf: Vec<u8>,
) -> Result<Vec<u8>, fiona::Error> {
  buf.clear();
  buf.resize(buf.capacity(), 0);

  let mut b = buf.as_mut_slice();
  let n = tls.write_tls(&mut b)?;
  unsafe {
    buf.set_len(n);
  }

  buf = s.async_write(buf).await?;
  assert!(!buf.is_empty());

  Ok(buf)
}

async fn send_full_tls<Data>(
  s: &mut fiona::ip::tcp::Socket,
  tls: &mut rustls::ConnectionCommon<Data>,
  mut buf: Vec<u8>,
) -> Result<Vec<u8>, fiona::Error> {
  while tls.wants_write() {
    buf = async_write_tls_helper(s, tls, buf).await?;
  }

  Ok(buf)
}

#[allow(clippy::unnecessary_wraps)]
fn read_plaintext<Data>(
  tls: &mut rustls::ConnectionCommon<Data>,
  mut buf: Vec<u8>,
) -> Result<Vec<u8>, fiona::Error> {
  buf.clear();
  buf.resize(buf.capacity(), 0);
  let r = tls.reader().read(&mut buf)?;
  unsafe {
    buf.set_len(r);
  }
  Ok(buf)
}

async fn async_read_impl<Data>(
  s: &mut fiona::ip::tcp::Socket,
  tls: &mut rustls::ConnectionCommon<Data>,
  mut buf: Vec<u8>,
) -> Result<Vec<u8>, fiona::Error> {
  assert!(!tls.is_handshaking());

  let mut info = tls.process_new_packets()?;

  let mut n = info.plaintext_bytes_to_read();
  if n > 0 {
    buf = read_plaintext(tls, buf)?;
    return Ok(buf);
  }

  if !tls.wants_read() && info.peer_has_closed() {
    tls.send_close_notify();
    buf = send_full_tls(s, tls, buf).await?;
    return Ok(buf);
  }

  buf.clear();
  buf = s.async_read(buf).await?;
  tls.read_tls(&mut &buf[..])?;
  info = tls.process_new_packets()?;
  if info.plaintext_bytes_to_read() == 0 {
    while info.plaintext_bytes_to_read() == 0 && !info.peer_has_closed() {
      buf.clear();
      buf = s.async_read(buf).await?;
      tls.read_tls(&mut &buf[..])?;
      info = tls.process_new_packets()?;
    }
  }

  n = info.plaintext_bytes_to_read();
  if n > 0 {
    buf = read_plaintext(tls, buf)?;
    return Ok(buf);
  }

  if info.peer_has_closed() {
    tls.send_close_notify();
    buf = send_full_tls(s, tls, buf).await?;
    return Ok(buf);
  }

  Ok(buf)
}
