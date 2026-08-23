use std::{
    fmt,
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use sim_lib_net_core::UrlParts;

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
    pub(crate) raw: String,
    pub(crate) parts: UrlParts,
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
    pub(crate) name: String,
    pub(crate) value: String,
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
    pub(crate) body: Vec<u8>,
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

pub(crate) fn valid_token(v: &str) -> bool {
    !v.is_empty() && v.bytes().all(|b| b > 0x20 && b < 0x7f && b != b':')
}

fn valid_value(v: &str) -> bool {
    v.bytes()
        .all(|b| b == b'\t' || (0x20..=0x7e).contains(&b) || (0x80..=0xff).contains(&b))
}

pub(crate) fn host_header(p: &UrlParts) -> String {
    if (p.scheme == "http" && p.port == 80) || (p.scheme == "https" && p.port == 443) {
        p.host.clone()
    } else {
        format!("{}:{}", p.host, p.port)
    }
}

pub(crate) fn io_error(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}
