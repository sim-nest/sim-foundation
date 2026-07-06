//! URL parsing into structural components.

use crate::NetError;

/// The structural components of an absolute URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlParts {
    /// Lowercase-preserving scheme exactly as written (e.g. `http`, `https`).
    pub scheme: String,
    /// Host without port.
    pub host: String,
    /// Port: the explicit port if present, else the scheme default
    /// (`http` -> 80, `https` -> 443).
    pub port: u16,
    /// Request path, defaulting to `/` when the URL has no path component.
    /// A non-empty path has any trailing slash trimmed (except the lone `/`).
    pub path: String,
}

/// Parse a `scheme://host[:port][/path]` URL.
///
/// Extracted from `sim-lib-agent-runner-http`'s `parse_url`. Differences from
/// that internal helper, which are intentional for a shared primitive:
///
/// * Default ports are resolved here for `http` (80) and `https` (443); any
///   other scheme is rejected with [`NetError::UnsupportedScheme`] when no
///   explicit port is given. (The client only used `http`/`https`.)
/// * The path defaults to `/` instead of the empty string when absent, so the
///   result is a usable request target on its own.
pub fn parse_url(url: &str) -> Result<UrlParts, NetError> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| NetError::MalformedUrl(url.to_owned()))?;

    let (host_port, path) = match rest.split_once('/') {
        Some((host_port, suffix)) => {
            let trimmed = suffix.trim_end_matches('/');
            (host_port, format!("/{trimmed}"))
        }
        None => (rest, "/".to_owned()),
    };

    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port)) => (
            host.to_owned(),
            port.parse::<u16>()
                .map_err(|_| NetError::InvalidPort(url.to_owned()))?,
        ),
        None => (host_port.to_owned(), default_port_for_scheme(scheme, url)?),
    };

    if host.is_empty() {
        return Err(NetError::MalformedUrl(url.to_owned()));
    }

    Ok(UrlParts {
        scheme: scheme.to_owned(),
        host,
        port,
        path,
    })
}

/// Parse a URL and require a specific `scheme`, applying `default_path` when the
/// URL carries no path component.
///
/// A policy-free convenience over [`parse_url`] for transport callers that know
/// which scheme they speak and want a usable request target:
///
/// * The scheme must equal `scheme` exactly, else
///   [`NetError::UnexpectedScheme`] is returned. (Callers still get default-port
///   resolution for `http`/`https` from [`parse_url`].)
/// * When the URL had no path, [`parse_url`] yields `/`; `parse_url_for_scheme`
///   substitutes `default_path` in that case, so a bare `scheme://host` becomes
///   a request target at the caller's default endpoint.
pub fn parse_url_for_scheme(
    url: &str,
    scheme: &str,
    default_path: &str,
) -> Result<UrlParts, NetError> {
    let mut parts = parse_url(url)?;
    if parts.scheme != scheme {
        return Err(NetError::UnexpectedScheme {
            expected: scheme.to_owned(),
            found: parts.scheme,
        });
    }
    if parts.path == "/" {
        parts.path = default_path.to_owned();
    }
    Ok(parts)
}

fn default_port_for_scheme(scheme: &str, url: &str) -> Result<u16, NetError> {
    match scheme {
        "http" => Ok(80),
        "https" => Ok(443),
        _ => Err(NetError::UnsupportedScheme(format!("{scheme} in {url}"))),
    }
}
