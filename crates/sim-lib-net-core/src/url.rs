//! URL parsing into structural components.

use crate::NetError;

/// A syntactically normalized absolute URI used as retrieval identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetrievalUri(String);

impl RetrievalUri {
    /// Returns the normalized URI.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Apply only equivalence-preserving RFC 3986 normalization for retrieval.
///
/// Query order, duplicate parameters, percent escapes, path case, and trailing
/// slashes are preserved. Unicode DNS names are refused: callers must first
/// supply a validated IDNA A-label, avoiding locale-dependent folding.
pub fn normalize_retrieval_uri(input: &str) -> Result<RetrievalUri, NetError> {
    let (scheme_raw, rest) = input
        .split_once("://")
        .ok_or_else(|| NetError::MalformedUrl(input.to_owned()))?;
    if scheme_raw.is_empty()
        || !scheme_raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return Err(NetError::MalformedUrl(input.to_owned()));
    }
    let scheme = scheme_raw.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(NetError::UnsupportedScheme(scheme));
    }
    let head = rest.split_once('#').map_or(rest, |(head, _)| head);
    let authority_end = head.find(['/', '?']).unwrap_or(head.len());
    let authority = &head[..authority_end];
    let tail = &head[authority_end..];
    if authority.is_empty() || authority.contains('@') || !authority.is_ascii() {
        return Err(NetError::MalformedUrl(input.to_owned()));
    }
    let (host_raw, port) = split_authority(authority, input)?;
    let host = host_raw.to_ascii_lowercase();
    validate_ascii_host(&host, input)?;
    let port = match (scheme.as_str(), port) {
        ("http", Some(80)) | ("https", Some(443)) | (_, None) => String::new(),
        (_, Some(value)) => format!(":{value}"),
    };
    let (path, query) = tail
        .split_once('?')
        .map_or((tail, None), |(p, q)| (p, Some(q)));
    let path = if path.is_empty() {
        "/".to_owned()
    } else {
        remove_dot_segments(path)
    };
    Ok(RetrievalUri(format!(
        "{scheme}://{host}{port}{path}{}",
        query.map_or(String::new(), |q| format!("?{q}"))
    )))
}

fn split_authority<'a>(
    authority: &'a str,
    input: &str,
) -> Result<(&'a str, Option<u16>), NetError> {
    if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| NetError::MalformedUrl(input.to_owned()))?;
        let suffix = &authority[end + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            Some(
                suffix
                    .strip_prefix(':')
                    .ok_or_else(|| NetError::MalformedUrl(input.to_owned()))?
                    .parse()
                    .map_err(|_| NetError::InvalidPort(input.to_owned()))?,
            )
        };
        return Ok((&authority[..=end], port));
    }
    match authority.rsplit_once(':') {
        Some((host, digits))
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            Ok((
                host,
                Some(
                    digits
                        .parse()
                        .map_err(|_| NetError::InvalidPort(input.to_owned()))?,
                ),
            ))
        }
        _ => Ok((authority, None)),
    }
}

fn validate_ascii_host(host: &str, input: &str) -> Result<(), NetError> {
    if host.starts_with('[') && host.ends_with(']') {
        return Ok(());
    }
    if host.is_empty()
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(NetError::MalformedUrl(input.to_owned()));
    }
    Ok(())
}

fn remove_dot_segments(path: &str) -> String {
    let trailing = path.ends_with('/') || path.ends_with("/.") || path.ends_with("/..");
    let mut out = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    let mut result = format!("/{}", out.join("/"));
    if trailing && result != "/" {
        result.push('/');
    }
    result
}

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

fn parse_authority(
    authority: &str,
    url: &str,
    default_port: impl FnOnce() -> Result<u16, NetError>,
) -> Result<(String, u16), NetError> {
    if let Some(bracketed) = authority.strip_prefix('[') {
        let end = bracketed
            .find(']')
            .ok_or_else(|| NetError::MalformedUrl(url.to_owned()))?;
        let host = &bracketed[..end];
        host.parse::<std::net::Ipv6Addr>()
            .map_err(|_| NetError::MalformedUrl(url.to_owned()))?;
        let suffix = &bracketed[end + 1..];
        let port = if suffix.is_empty() {
            default_port()?
        } else {
            suffix
                .strip_prefix(':')
                .filter(|value| !value.is_empty())
                .ok_or_else(|| NetError::MalformedUrl(url.to_owned()))?
                .parse::<u16>()
                .map_err(|_| NetError::InvalidPort(url.to_owned()))?
        };
        return Ok((host.to_owned(), port));
    }
    if authority.contains(['[', ']']) {
        return Err(NetError::MalformedUrl(url.to_owned()));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') && !host.is_empty() => Ok((
            host.to_owned(),
            port.parse::<u16>()
                .map_err(|_| NetError::InvalidPort(url.to_owned()))?,
        )),
        Some(_) => Err(NetError::MalformedUrl(url.to_owned())),
        None if authority.is_empty() => Err(NetError::MalformedUrl(url.to_owned())),
        None => Ok((authority.to_owned(), default_port()?)),
    }
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

    let (host, port) = parse_authority(host_port, url, || default_port_for_scheme(scheme, url))?;

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

/// Parse a URL for a known `scheme`, resolving `ws`/`wss` default ports and
/// preserving a caller-supplied trailing slash in the path.
///
/// A superset variant of [`parse_url_for_scheme`] for transports that also
/// speak WebSocket URLs and treat a trailing slash as significant. The
/// differences from [`parse_url`]/[`parse_url_for_scheme`], all driven by data
/// rather than transport policy:
///
/// * Default ports resolve for `ws` (80) and `wss` (443) in addition to `http`
///   (80) and `https` (443); any other scheme without an explicit port is
///   rejected with [`NetError::UnsupportedScheme`].
/// * The path is taken verbatim after the first `/`, so a caller's trailing
///   slash is preserved (`scheme://host/a/` keeps `/a/`), where [`parse_url`]
///   trims it. A request target and its trailing-slash variant address
///   different resources, so a transport that reconstructs the URL must not lose
///   the distinction.
/// * `default_path` is substituted only when the URL carries no path component
///   at all (no `/` after the authority); `scheme://host/` yields `/`, not
///   `default_path`.
///
/// The scheme must equal `scheme` exactly, else [`NetError::UnexpectedScheme`]
/// is returned. The scheme list and ports are data; the function opens no
/// sockets and applies no transport policy.
pub fn parse_url_for_scheme_preserving_path(
    url: &str,
    scheme: &str,
    default_path: &str,
) -> Result<UrlParts, NetError> {
    let (url_scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| NetError::MalformedUrl(url.to_owned()))?;
    if url_scheme != scheme {
        return Err(NetError::UnexpectedScheme {
            expected: scheme.to_owned(),
            found: url_scheme.to_owned(),
        });
    }

    let (host_port, path) = match rest.split_once('/') {
        Some((host_port, suffix)) => (host_port, format!("/{suffix}")),
        None => (rest, default_path.to_owned()),
    };

    let (host, port) = parse_authority(host_port, url, || {
        web_default_port_for_scheme(url_scheme)
            .ok_or_else(|| NetError::UnsupportedScheme(format!("{scheme} in {url}")))
    })?;

    Ok(UrlParts {
        scheme: url_scheme.to_owned(),
        host,
        port,
        path,
    })
}

fn default_port_for_scheme(scheme: &str, url: &str) -> Result<u16, NetError> {
    match scheme {
        "http" => Ok(80),
        "https" => Ok(443),
        _ => Err(NetError::UnsupportedScheme(format!("{scheme} in {url}"))),
    }
}

/// The registered default port for a web-transport scheme, or `None` when the
/// scheme has no default in this table.
///
/// The table is data, not policy: `http`/`ws` -> 80, `https`/`wss` -> 443.
fn web_default_port_for_scheme(scheme: &str) -> Option<u16> {
    match scheme {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        _ => None,
    }
}
