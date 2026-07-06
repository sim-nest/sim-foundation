//! The NetError type for net-core parsing failures.

/// Errors produced by the net-core parsing primitives.
///
/// These describe malformed or oversize wire data only. They carry no I/O or
/// application context; callers translate them into their own error domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetError {
    /// The URL did not contain a `scheme://` separator, or was otherwise
    /// structurally malformed (e.g. missing host).
    MalformedUrl(String),
    /// The URL scheme is not one this crate recognizes for default-port
    /// resolution.
    UnsupportedScheme(String),
    /// The URL scheme did not match the scheme a caller required (via
    /// [`parse_url_for_scheme`](crate::parse_url_for_scheme)).
    UnexpectedScheme {
        /// The scheme the caller required.
        expected: String,
        /// The scheme actually found in the URL.
        found: String,
    },
    /// The URL port component could not be parsed as a `u16`.
    InvalidPort(String),
    /// The HTTP response head (status line + headers) was malformed.
    InvalidHead(String),
    /// A length-prefixed or buffered payload exceeded the caller's size limit.
    OversizeBody(usize),
}

impl core::fmt::Display for NetError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MalformedUrl(url) => write!(formatter, "malformed url {url}"),
            Self::UnsupportedScheme(scheme) => write!(formatter, "unsupported url scheme {scheme}"),
            Self::UnexpectedScheme { expected, found } => {
                write!(formatter, "expected url scheme {expected}, found {found}")
            }
            Self::InvalidPort(url) => write!(formatter, "invalid port in url {url}"),
            Self::InvalidHead(detail) => write!(formatter, "invalid http head: {detail}"),
            Self::OversizeBody(limit) => {
                write!(formatter, "payload exceeded size limit of {limit} bytes")
            }
        }
    }
}

impl core::error::Error for NetError {}
