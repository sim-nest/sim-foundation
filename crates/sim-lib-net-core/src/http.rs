//! Incremental parsing of HTTP response heads.

use crate::NetError;

/// A parsed HTTP response head: status line plus headers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpHead {
    /// Numeric status code from the status line.
    pub status: u16,
    /// Reason phrase (everything after the status code on the status line);
    /// may be empty.
    pub reason: String,
    /// Header field name/value pairs in the order received. Values are trimmed.
    pub headers: Vec<(String, String)>,
}

impl HttpHead {
    /// Case-insensitive lookup of the first header with the given name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// How the body following a response head should be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpBodyMode {
    /// Exactly `n` bytes follow.
    ContentLength(usize),
    /// `Transfer-Encoding: chunked` framing follows.
    Chunked,
    /// Read until the connection is closed (no length, no chunking).
    UntilEof,
    /// No body is expected. Not returned by [`body_mode`]; provided for callers
    /// that classify bodies by status (e.g. 204/304) before reading.
    Empty,
}

/// Build a simple HTTP/1.1 request head.
///
/// This is a policy-free formatter: callers choose the method, target,
/// headers, content length, and transport. The helper rejects CR/LF injection
/// and malformed method/header names, then emits a `Connection: close` head so
/// socket callers can use response EOF as a body boundary when needed.
pub fn build_http_request_head(
    method: &str,
    target: &str,
    host: &str,
    content_length: Option<usize>,
    headers: &[(String, String)],
) -> Result<String, NetError> {
    validate_token("method", method)?;
    validate_field_value("target", target)?;
    validate_field_value("host", host)?;

    let mut head = format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    if let Some(length) = content_length {
        head.push_str(&format!("Content-Length: {length}\r\n"));
    }
    for (name, value) in headers {
        validate_token("header name", name)?;
        validate_field_value("header value", value)?;
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    Ok(head)
}

/// Parse a CRLF-delimited HTTP response head.
///
/// Extracted from `sim-lib-agent-runner-http`'s `read_response` head parsing.
/// `head_text` is the head with or without the trailing `\r\n\r\n`; the status
/// line is the first CRLF-delimited line and the remainder are `key: value`
/// headers. The status code is the second whitespace-delimited token of the
/// status line, matching the client's `version status` split.
pub fn parse_http_head(head_text: &str) -> Result<HttpHead, NetError> {
    let body = head_text.trim_end_matches("\r\n\r\n");
    let mut lines = body.split("\r\n");

    let status_line = lines
        .next()
        .ok_or_else(|| NetError::InvalidHead("missing status line".to_owned()))?;
    let mut parts = status_line.split_whitespace();
    let _version = parts
        .next()
        .ok_or_else(|| NetError::InvalidHead("missing version".to_owned()))?;
    let status = parts
        .next()
        .ok_or_else(|| NetError::InvalidHead("missing status".to_owned()))?
        .parse::<u16>()
        .map_err(|_| NetError::InvalidHead("invalid status code".to_owned()))?;
    let reason = parts.collect::<Vec<_>>().join(" ");

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| NetError::InvalidHead("invalid header line".to_owned()))?;
        headers.push((key.to_owned(), value.trim().to_owned()));
    }

    Ok(HttpHead {
        status,
        reason,
        headers,
    })
}

fn validate_token(label: &str, value: &str) -> Result<(), NetError> {
    if value.is_empty()
        || value
            .bytes()
            .any(|byte| byte <= b' ' || byte == b':' || byte >= 0x7f)
    {
        return Err(NetError::InvalidHead(format!("invalid {label}")));
    }
    Ok(())
}

fn validate_field_value(label: &str, value: &str) -> Result<(), NetError> {
    if value.contains('\r') || value.contains('\n') {
        return Err(NetError::InvalidHead(format!("invalid {label}")));
    }
    Ok(())
}

/// Classify how the body should be read, matching the client's precedence.
///
/// Extracted from `sim-lib-agent-runner-http`'s `read_response`:
///
/// 1. `Transfer-Encoding: chunked` -> [`HttpBodyMode::Chunked`].
/// 2. otherwise a present, non-zero `Content-Length` -> [`HttpBodyMode::ContentLength`].
/// 3. otherwise (no/zero length) -> [`HttpBodyMode::UntilEof`].
///
/// The client treated a missing or `0` `Content-Length` identically (read to
/// EOF), so a literal `Content-Length: 0` maps to `UntilEof` here, not
/// [`HttpBodyMode::Empty`]. An invalid `Content-Length` is rejected.
pub fn body_mode(head: &HttpHead) -> Result<HttpBodyMode, NetError> {
    if let Some(encoding) = head.header("Transfer-Encoding")
        && encoding.eq_ignore_ascii_case("chunked")
    {
        return Ok(HttpBodyMode::Chunked);
    }
    match head.header("Content-Length") {
        Some(raw) => {
            let length = raw
                .parse::<usize>()
                .map_err(|_| NetError::InvalidHead("invalid content-length".to_owned()))?;
            if length == 0 {
                Ok(HttpBodyMode::UntilEof)
            } else {
                Ok(HttpBodyMode::ContentLength(length))
            }
        }
        None => Ok(HttpBodyMode::UntilEof),
    }
}
