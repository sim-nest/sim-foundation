use std::{
    io::{Read, Write},
    time::Duration,
};

#[cfg(feature = "tls")]
use std::sync::Arc;

#[cfg(feature = "tls")]
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, StreamOwned,
    pki_types::{CertificateDer, ServerName},
};

use crate::{Error, Policy, Result, Url};

/// Connector injection point. Production binds capsule ports; tests script it.
pub trait Connection: Read + Write + Send {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()>;
    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()>;
}

pub trait Connector: Send + Sync {
    fn connect(&self, url: &Url, policy: &Policy) -> Result<Box<dyn Connection>>;
}

/// Native capsule connector. Capsules may instead inject their own connector;
/// this realization is kept here so applications never own socket policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct TcpConnector;

impl Connector for TcpConnector {
    fn connect(&self, url: &Url, policy: &Policy) -> Result<Box<dyn Connection>> {
        use std::net::{TcpStream, ToSocketAddrs};
        let address = (url.parts.host.as_str(), url.parts.port)
            .to_socket_addrs()
            .map_err(|e| Error::Dns(e.to_string()))?
            .next()
            .ok_or_else(|| Error::Dns("no addresses".into()))?;
        let stream = TcpStream::connect_timeout(&address, policy.connect_timeout)
            .map_err(|e| Error::Connect(e.to_string()))?;
        Ok(Box::new(stream))
    }
}

impl Connection for std::net::TcpStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        std::net::TcpStream::set_read_timeout(self, timeout)
    }
    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        std::net::TcpStream::set_write_timeout(self, timeout)
    }
}

#[cfg(feature = "tls")]
pub(crate) fn connect_tls(
    url: &Url,
    stream: Box<dyn Connection>,
    policy: &Policy,
) -> Result<Box<dyn Connection>> {
    if url.parts.scheme == "http" {
        return Ok(stream);
    }
    let mut roots = RootCertStore::empty();
    if policy.tls_root_certificates.is_empty() {
        for cert in rustls_native_certs::load_native_certs().certs {
            roots.add(cert).map_err(|e| Error::Io(e.to_string()))?;
        }
    } else {
        for cert in &policy.tls_root_certificates {
            roots
                .add(CertificateDer::from(cert.clone()))
                .map_err(|e| Error::Io(e.to_string()))?;
        }
    }
    if roots.is_empty() {
        return Err(Error::TlsUnavailable);
    }
    let mut config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.enable_sni = policy.send_sni;
    let config = Arc::new(config);
    let name = ServerName::try_from(url.parts.host.clone()).map_err(|_| Error::InvalidUrl)?;
    let connection = ClientConnection::new(config, name).map_err(|e| Error::Io(e.to_string()))?;
    Ok(Box::new(TlsConnection(StreamOwned::new(
        connection, stream,
    ))))
}

#[cfg(not(feature = "tls"))]
pub(crate) fn connect_tls(
    url: &Url,
    stream: Box<dyn Connection>,
    _policy: &Policy,
) -> Result<Box<dyn Connection>> {
    if url.parts.scheme == "https" {
        Err(Error::TlsUnavailable)
    } else {
        Ok(stream)
    }
}

#[cfg(feature = "tls")]
struct TlsConnection(StreamOwned<ClientConnection, Box<dyn Connection>>);

#[cfg(feature = "tls")]
impl Read for TlsConnection {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(b)
    }
}

#[cfg(feature = "tls")]
impl Write for TlsConnection {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.write(b)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

#[cfg(feature = "tls")]
impl Connection for TlsConnection {
    fn set_read_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.0.sock.set_read_timeout(t)
    }
    fn set_write_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.0.sock.set_write_timeout(t)
    }
}
