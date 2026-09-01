use std::{
    io::{BufReader, Read, Write},
    time::Instant,
};

use sim_lib_net_core::{HeadOutcome, HttpBodyMode};

use crate::{
    Connector, Error, Header, Policy, ProxyPolicy, RedirectPolicy, Request, RequestBody, Response,
    Result, connect_tls, host_header, io_error,
    response::{HEAD_CAP, push_chunk, read_chunked_stream, reject_ambiguous_response},
};

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
        write_body(request, &self.policy, started, &mut *stream)?;
        stream.flush().map_err(io_error)?;
        checkpoint(request, &self.policy, started)?;
        read_response(&mut *stream, request, &self.policy, started, &mut on_chunk)
    }
}

fn write_body(
    request: &mut Request<'_>,
    policy: &Policy,
    started: Instant,
    stream: &mut dyn Write,
) -> Result<()> {
    let body_cancellation = request.cancellation.clone();
    let body_deadline = request.deadline;
    match &mut request.body {
        RequestBody::Empty => Ok(()),
        RequestBody::Bytes(body) => stream.write_all(body).map_err(io_error),
        RequestBody::Stream(body) => {
            let mut sent = 0usize;
            let mut chunk = [0u8; 8192];
            loop {
                if body_cancellation.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                if body_deadline.is_some_and(|deadline| Instant::now() >= deadline)
                    || started.elapsed() >= policy.total_timeout
                {
                    return Err(Error::DeadlineExceeded);
                }
                let read = body.read(&mut chunk).map_err(io_error)?;
                if read == 0 {
                    break;
                }
                sent = sent.saturating_add(read);
                if sent > policy.max_request_bytes {
                    return Err(Error::RequestTooLarge {
                        cap: policy.max_request_bytes,
                    });
                }
                write!(stream, "{read:x}\r\n").map_err(io_error)?;
                stream.write_all(&chunk[..read]).map_err(io_error)?;
                stream.write_all(b"\r\n").map_err(io_error)?;
            }
            stream.write_all(b"0\r\n\r\n").map_err(io_error)
        }
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

pub(crate) fn checkpoint(request: &Request<'_>, policy: &Policy, started: Instant) -> Result<()> {
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
    reject_denied_response_metadata(policy, parsed.status, &parsed.headers)?;
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
            read_content_length(
                &mut reader,
                n,
                &mut body,
                request,
                policy,
                started,
                on_chunk,
            )?;
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

fn reject_denied_response_metadata(
    policy: &Policy,
    status: u16,
    headers: &[(String, String)],
) -> Result<()> {
    if (300..400).contains(&status)
        && headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("location"))
        && matches!(policy.redirects, RedirectPolicy::Off)
    {
        return Err(Error::RedirectDenied);
    }
    if headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-encoding") && !value.eq_ignore_ascii_case("identity")
    }) {
        return Err(Error::DecompressionLimit {
            cap: policy.max_decompressed_bytes,
        });
    }
    Ok(())
}

fn read_content_length(
    reader: &mut dyn Read,
    n: usize,
    body: &mut Vec<u8>,
    request: &Request<'_>,
    policy: &Policy,
    started: Instant,
    on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    if n > policy.max_response_bytes {
        return Err(Error::ResponseTooLarge {
            cap: policy.max_response_bytes,
        });
    }
    let mut buffer = [0u8; 8192];
    while body.len() < n {
        checkpoint(request, policy, started)?;
        let take = (n - body.len()).min(buffer.len());
        let got = reader.read(&mut buffer[..take]).map_err(io_error)?;
        if got == 0 {
            return Err(Error::Protocol("truncated response body".into()));
        }
        push_chunk(body, &buffer[..got], policy, on_chunk)?;
    }
    Ok(())
}
