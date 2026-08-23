//! One blocking HTTP policy boundary for SIM.
//!
//! Protocol parsing stays in `sim-lib-net-core`; sockets and DNS arrive through
//! the bound capsule's `sim-transport-ports` services. No ambient network,
//! proxy, cookie, credential, redirect, or logging behavior is hidden here.

use std::{
    fmt,
    io::{BufRead, BufReader, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use sim_lib_net_core::{HeadOutcome, HttpBodyMode, UrlParts};

#[cfg(feature = "tls")]
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, StreamOwned,
    pki_types::{CertificateDer, ServerName},
};

const HEAD_CAP: usize = 64 * 1024;

/// Cookbook descriptors embedded for documentation and runtime discovery.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

/// An HTTP method whose spelling has passed token validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Method(String);

impl Method {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        valid_token(&value)
            .then_some(Self(value))
            .ok_or(Error::InvalidMethod)
    }
    pub fn get() -> Self {
        Self("GET".into())
    }
    pub fn post() -> Self {
        Self("POST".into())
    }
    pub fn head() -> Self {
        Self("HEAD".into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An absolute HTTP URL, validated once at construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    raw: String,
    parts: UrlParts,
}

impl Url {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();
        if raw.bytes().any(|b| b <= 0x20 || b == 0x7f) {
            return Err(Error::InvalidUrl);
        }
        let authority = raw
            .split_once("://")
            .ok_or(Error::InvalidUrl)?
            .1
            .split('/')
            .next()
            .unwrap_or("");
        if authority.contains('@') {
            return Err(Error::UserInfoForbidden);
        }
        let parts = sim_lib_net_core::parse_url(&raw).map_err(|_| Error::InvalidUrl)?;
        if !matches!(parts.scheme.as_str(), "http" | "https") {
            return Err(Error::UnsupportedScheme);
        }
        Ok(Self { raw, parts })
    }
    pub fn as_str(&self) -> &str {
        &self.raw
    }
    pub fn path(&self) -> &str {
        &self.parts.path
    }
    pub fn host(&self) -> &str {
        &self.parts.host
    }
    pub fn port(&self) -> u16 {
        self.parts.port
    }
    pub fn scheme(&self) -> &str {
        &self.parts.scheme
    }
}

/// Ordered header field. Sensitive values are never included in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Header {
    name: String,
    value: String,
    sensitive: bool,
}

impl Header {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        Self::with_sensitivity(name, value, false)
    }
    pub fn sensitive(name: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        Self::with_sensitivity(name, value, true)
    }
    fn with_sensitivity(
        name: impl Into<String>,
        value: impl Into<String>,
        sensitive: bool,
    ) -> Result<Self> {
        let name = name.into();
        let value = value.into();
        if !valid_token(&name) {
            return Err(Error::InvalidHeaderName);
        }
        if !valid_value(&value) {
            return Err(Error::InvalidHeaderValue);
        }
        let intrinsically_sensitive = matches!(
            name.to_ascii_lowercase().as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
        );
        Ok(Self {
            name,
            value,
            sensitive: sensitive || intrinsically_sensitive,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn is_sensitive(&self) -> bool {
        self.sensitive
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Header")
            .field("name", &self.name)
            .field(
                "value",
                &if self.sensitive {
                    "[REDACTED]"
                } else {
                    &self.value
                },
            )
            .field("sensitive", &self.sensitive)
            .finish()
    }
}

/// Cooperative cancellation observed before and throughout I/O.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectPolicy {
    Off,
    SameOrigin {
        limit: usize,
    },
    AnyOrigin {
        limit: usize,
        forward_sensitive_headers: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyPolicy {
    Off,
    Explicit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlsRoots {
    Capsule,
    #[cfg(feature = "tls")]
    Explicit,
}

/// Every authority expansion and resource bound is visible in this value.
#[derive(Clone, Debug)]
pub struct Policy {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub total_timeout: Duration,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_decompressed_bytes: usize,
    pub redirects: RedirectPolicy,
    pub proxy: ProxyPolicy,
    pub tls_roots: TlsRoots,
    pub send_sni: bool,
    pub allow_userinfo: bool,
    pub cookies: bool,
    pub ambient_credentials: bool,
    /// DER-encoded trust anchors supplied by the bound capsule. Native roots
    /// are never consulted when this list is non-empty.
    pub tls_root_certificates: Vec<Vec<u8>>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            total_timeout: Duration::from_secs(60),
            max_request_bytes: 8 * 1024 * 1024,
            max_response_bytes: 8 * 1024 * 1024,
            max_decompressed_bytes: 16 * 1024 * 1024,
            redirects: RedirectPolicy::Off,
            proxy: ProxyPolicy::Off,
            tls_roots: TlsRoots::Capsule,
            send_sni: true,
            allow_userinfo: false,
            cookies: false,
            ambient_credentials: false,
            tls_root_certificates: Vec::new(),
        }
    }
}

pub enum RequestBody<'a> {
    Empty,
    Bytes(&'a [u8]),
    Stream(&'a mut dyn Read),
}
pub struct Request<'a> {
    pub method: Method,
    pub url: Url,
    pub headers: Vec<Header>,
    pub body: RequestBody<'a>,
    pub deadline: Option<Instant>,
    pub cancellation: Cancellation,
}
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub reason: String,
    pub headers: Vec<Header>,
    pub trailers: Vec<Header>,
    body: Vec<u8>,
}
impl Response {
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidUrl,
    UserInfoForbidden,
    UnsupportedScheme,
    InvalidMethod,
    InvalidHeaderName,
    InvalidHeaderValue,
    AmbiguousHeader,
    UnsupportedTransferFraming,
    RequestTooLarge { cap: usize },
    ResponseTooLarge { cap: usize },
    DecompressionLimit { cap: usize },
    Cancelled,
    DeadlineExceeded,
    RedirectDenied,
    ProxyDenied,
    TlsUnavailable,
    Dns(String),
    Connect(String),
    Io(String),
    Protocol(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

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

pub struct Client<C> {
    connector: C,
    policy: Policy,
}
impl<C: Connector> Client<C> {
    pub fn new(connector: C, policy: Policy) -> Self {
        Self { connector, policy }
    }
    pub fn execute(&self, mut request: Request<'_>) -> Result<Response> {
        self.execute_stream(&mut request, |_| Ok(()))
    }
    pub fn execute_stream(
        &self,
        request: &mut Request<'_>,
        mut on_chunk: impl FnMut(&[u8]) -> Result<()>,
    ) -> Result<Response> {
        validate_request(request, &self.policy)?;
        checkpoint(request, &self.policy, Instant::now())?;
        let started = Instant::now();
        let raw = self.connector.connect(&request.url, &self.policy)?;
        raw.set_read_timeout(Some(self.policy.read_timeout))
            .map_err(io_error)?;
        raw.set_write_timeout(Some(self.policy.write_timeout))
            .map_err(io_error)?;
        let mut stream = connect_tls(&request.url, raw, &self.policy)?;
        let body_len = match &request.body {
            RequestBody::Empty => None,
            RequestBody::Bytes(v) => Some(v.len()),
            RequestBody::Stream(_) => None,
        };
        let headers = request
            .headers
            .iter()
            .map(|h| (h.name.clone(), h.value.clone()))
            .collect::<Vec<_>>();
        let mut head = sim_lib_net_core::build_http_request_head(
            request.method.as_str(),
            request.url.path(),
            &host_header(&request.url.parts),
            body_len,
            &headers,
        )
        .map_err(|e| Error::Protocol(e.to_string()))?;
        if matches!(request.body, RequestBody::Stream(_)) {
            head = head.replacen("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n", 1);
        }
        stream.write_all(head.as_bytes()).map_err(io_error)?;
        let body_cancellation = request.cancellation.clone();
        let body_deadline = request.deadline;
        match &mut request.body {
            RequestBody::Empty => {}
            RequestBody::Bytes(body) => stream.write_all(body).map_err(io_error)?,
            RequestBody::Stream(body) => {
                let mut sent = 0usize;
                let mut chunk = [0u8; 8192];
                loop {
                    if body_cancellation.is_cancelled() {
                        return Err(Error::Cancelled);
                    }
                    if body_deadline.is_some_and(|deadline| Instant::now() >= deadline)
                        || started.elapsed() >= self.policy.total_timeout
                    {
                        return Err(Error::DeadlineExceeded);
                    }
                    let read = body.read(&mut chunk).map_err(io_error)?;
                    if read == 0 {
                        break;
                    }
                    sent = sent.saturating_add(read);
                    if sent > self.policy.max_request_bytes {
                        return Err(Error::RequestTooLarge {
                            cap: self.policy.max_request_bytes,
                        });
                    }
                    write!(stream, "{read:x}\r\n").map_err(io_error)?;
                    stream.write_all(&chunk[..read]).map_err(io_error)?;
                    stream.write_all(b"\r\n").map_err(io_error)?;
                }
                stream.write_all(b"0\r\n\r\n").map_err(io_error)?;
            }
        }
        stream.flush().map_err(io_error)?;
        checkpoint(request, &self.policy, started)?;
        read_response(&mut *stream, request, &self.policy, started, &mut on_chunk)
    }
}

fn validate_request(request: &Request<'_>, policy: &Policy) -> Result<()> {
    if !policy.allow_userinfo
        && request
            .url
            .raw
            .split_once("://")
            .unwrap()
            .1
            .split('/')
            .next()
            .unwrap_or("")
            .contains('@')
    {
        return Err(Error::UserInfoForbidden);
    }
    if !matches!(policy.proxy, ProxyPolicy::Off) {
        return Err(Error::ProxyDenied);
    }
    let mut content_lengths = 0;
    let mut transfers = 0;
    for h in &request.headers {
        if h.name.eq_ignore_ascii_case("content-length") {
            content_lengths += 1;
        }
        if h.name.eq_ignore_ascii_case("transfer-encoding") {
            transfers += 1;
        }
    }
    if content_lengths > 0 || transfers > 0 {
        return Err(Error::AmbiguousHeader);
    }
    let length = match &request.body {
        RequestBody::Empty => 0,
        RequestBody::Bytes(v) => v.len(),
        RequestBody::Stream(_) => 0,
    };
    if length > policy.max_request_bytes {
        return Err(Error::RequestTooLarge {
            cap: policy.max_request_bytes,
        });
    }
    Ok(())
}
fn checkpoint(request: &Request<'_>, policy: &Policy, started: Instant) -> Result<()> {
    if request.cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    if request.deadline.is_some_and(|d| Instant::now() >= d)
        || started.elapsed() >= policy.total_timeout
    {
        return Err(Error::DeadlineExceeded);
    }
    Ok(())
}

fn read_response(
    stream: &mut dyn Read,
    request: &Request<'_>,
    policy: &Policy,
    started: Instant,
    on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
) -> Result<Response> {
    let mut reader = BufReader::new(stream);
    let head = match sim_lib_net_core::read_head_until_double_crlf(&mut reader, HEAD_CAP)
        .map_err(io_error)?
    {
        HeadOutcome::Head(v) => v,
        HeadOutcome::TooLarge => return Err(Error::ResponseTooLarge { cap: HEAD_CAP }),
        _ => return Err(Error::Protocol("truncated response head".into())),
    };
    let parsed = sim_lib_net_core::parse_http_head(
        std::str::from_utf8(&head).map_err(|_| Error::Protocol("non-utf8 response head".into()))?,
    )
    .map_err(|e| Error::Protocol(e.to_string()))?;
    reject_ambiguous_response(&parsed.headers)?;
    if (300..400).contains(&parsed.status)
        && parsed
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("location"))
        && matches!(policy.redirects, RedirectPolicy::Off)
    {
        return Err(Error::RedirectDenied);
    }
    if parsed.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-encoding") && !value.eq_ignore_ascii_case("identity")
    }) {
        return Err(Error::DecompressionLimit {
            cap: policy.max_decompressed_bytes,
        });
    }
    let headers = parsed
        .headers
        .iter()
        .map(|(n, v)| Header::new(n.clone(), v.clone()))
        .collect::<Result<Vec<_>>>()?;
    let mut body = Vec::new();
    let mut trailers = Vec::new();
    let mut buffer = [0u8; 8192];
    let mode = if request.method.as_str() == "HEAD" || matches!(parsed.status, 204 | 304) {
        HttpBodyMode::Empty
    } else {
        sim_lib_net_core::body_mode(&parsed).map_err(|e| Error::Protocol(e.to_string()))?
    };
    match mode {
        HttpBodyMode::ContentLength(n) => {
            if n > policy.max_response_bytes {
                return Err(Error::ResponseTooLarge {
                    cap: policy.max_response_bytes,
                });
            }
            while body.len() < n {
                checkpoint(request, policy, started)?;
                let take = (n - body.len()).min(buffer.len());
                let got = reader.read(&mut buffer[..take]).map_err(io_error)?;
                if got == 0 {
                    return Err(Error::Protocol("truncated response body".into()));
                }
                push_chunk(&mut body, &buffer[..got], policy, on_chunk)?;
            }
        }
        HttpBodyMode::UntilEof => loop {
            checkpoint(request, policy, started)?;
            let got = reader.read(&mut buffer).map_err(io_error)?;
            if got == 0 {
                break;
            }
            push_chunk(&mut body, &buffer[..got], policy, on_chunk)?;
        },
        HttpBodyMode::Chunked => {
            trailers =
                read_chunked_stream(&mut reader, &mut body, request, policy, started, on_chunk)?
        }
        HttpBodyMode::Empty => {}
    }
    Ok(Response {
        status: parsed.status,
        reason: parsed.reason,
        headers,
        trailers,
        body,
    })
}

fn read_chunked_stream(
    reader: &mut dyn BufRead,
    body: &mut Vec<u8>,
    request: &Request<'_>,
    policy: &Policy,
    started: Instant,
    on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
) -> Result<Vec<Header>> {
    loop {
        checkpoint(request, policy, started)?;
        let mut size_line = String::new();
        if reader.read_line(&mut size_line).map_err(io_error)? == 0 {
            return Err(Error::Protocol("truncated chunk size".into()));
        }
        if size_line.len() > HEAD_CAP || !size_line.ends_with("\r\n") {
            return Err(Error::Protocol("invalid chunk size line".into()));
        }
        let size_text = size_line
            .trim_end_matches("\r\n")
            .split(';')
            .next()
            .unwrap_or("");
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| Error::Protocol("invalid chunk size".into()))?;
        if size == 0 {
            break;
        }
        if body.len().saturating_add(size) > policy.max_response_bytes {
            return Err(Error::ResponseTooLarge {
                cap: policy.max_response_bytes,
            });
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader.read_exact(&mut body[start..]).map_err(io_error)?;
        on_chunk(&body[start..])?;
        let mut terminator = [0u8; 2];
        reader.read_exact(&mut terminator).map_err(io_error)?;
        if terminator != *b"\r\n" {
            return Err(Error::Protocol("invalid chunk terminator".into()));
        }
    }
    let mut trailers = Vec::new();
    let mut metadata_bytes = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(io_error)? == 0 {
            return Err(Error::Protocol("truncated trailers".into()));
        }
        metadata_bytes = metadata_bytes.saturating_add(line.len());
        if metadata_bytes > HEAD_CAP {
            return Err(Error::ResponseTooLarge { cap: HEAD_CAP });
        }
        if line == "\r\n" {
            break;
        }
        let line = line
            .strip_suffix("\r\n")
            .ok_or_else(|| Error::Protocol("invalid trailer line".into()))?;
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::Protocol("invalid trailer line".into()))?;
        trailers.push(Header::new(name, value.trim_start())?);
    }
    Ok(trailers)
}
fn push_chunk(
    body: &mut Vec<u8>,
    chunk: &[u8],
    policy: &Policy,
    cb: &mut dyn FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    if body.len().saturating_add(chunk.len()) > policy.max_response_bytes {
        return Err(Error::ResponseTooLarge {
            cap: policy.max_response_bytes,
        });
    }
    cb(chunk)?;
    body.extend_from_slice(chunk);
    Ok(())
}
fn reject_ambiguous_response(headers: &[(String, String)]) -> Result<()> {
    let cls = headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .count();
    let tes = headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("transfer-encoding"))
        .count();
    if cls > 1 || (cls > 0 && tes > 0) || tes > 1 {
        return Err(Error::AmbiguousHeader);
    }
    if let Some((_, v)) = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("transfer-encoding"))
        && !v.eq_ignore_ascii_case("chunked")
    {
        return Err(Error::UnsupportedTransferFraming);
    }
    Ok(())
}
fn valid_token(v: &str) -> bool {
    !v.is_empty() && v.bytes().all(|b| b > 0x20 && b < 0x7f && b != b':')
}
fn valid_value(v: &str) -> bool {
    v.bytes()
        .all(|b| b == b'\t' || (0x20..=0x7e).contains(&b) || (0x80..=0xff).contains(&b))
}
fn host_header(p: &UrlParts) -> String {
    if (p.scheme == "http" && p.port == 80) || (p.scheme == "https" && p.port == 443) {
        p.host.clone()
    } else {
        format!("{}:{}", p.host, p.port)
    }
}
fn io_error(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}

#[cfg(feature = "tls")]
fn connect_tls(
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
fn connect_tls(
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

#[cfg(test)]
mod tests;
