use crate::{
    CapOutcome, HeadOutcome, HttpBodyMode, LineDecoder, NdjsonDecoder, NetError, SseDecoder,
    body_mode, decode_chunked, parse_http_head, parse_url, parse_url_for_scheme,
    parse_url_for_scheme_preserving_path, read_capped_line, read_head_until_double_crlf,
};

#[test]
fn parses_url_with_explicit_port() {
    let parts = parse_url("http://example.com:8080/v1/chat").unwrap();
    assert_eq!(parts.scheme, "http");
    assert_eq!(parts.host, "example.com");
    assert_eq!(parts.port, 8080);
    assert_eq!(parts.path, "/v1/chat");
}

#[test]
fn parses_url_with_default_ports_and_path() {
    let http = parse_url("http://example.com").unwrap();
    assert_eq!(http.port, 80);
    assert_eq!(http.path, "/");

    let https = parse_url("https://api.example.com/v1/").unwrap();
    assert_eq!(https.port, 443);
    // Trailing slash trimmed.
    assert_eq!(https.path, "/v1");
}

#[test]
fn rejects_malformed_and_unsupported_urls() {
    assert_eq!(
        parse_url("example.com/x"),
        Err(NetError::MalformedUrl("example.com/x".to_owned()))
    );
    assert!(matches!(
        parse_url("ftp://example.com"),
        Err(NetError::UnsupportedScheme(_))
    ));
    assert!(matches!(
        parse_url("http://example.com:notaport/x"),
        Err(NetError::InvalidPort(_))
    ));
    assert!(matches!(
        parse_url("http:///path"),
        Err(NetError::MalformedUrl(_))
    ));
}

#[test]
fn parses_head_and_classifies_content_length() {
    let head = parse_http_head(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 17\r\n",
    )
    .unwrap();
    assert_eq!(head.status, 200);
    assert_eq!(head.reason, "OK");
    assert_eq!(head.header("content-type"), Some("application/json"));
    assert_eq!(body_mode(&head).unwrap(), HttpBodyMode::ContentLength(17));
}

#[test]
fn classifies_chunked_over_content_length() {
    let head = parse_http_head(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\n",
    )
    .unwrap();
    assert_eq!(body_mode(&head).unwrap(), HttpBodyMode::Chunked);
}

#[test]
fn zero_content_length_reads_until_eof() {
    let head = parse_http_head("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n").unwrap();
    assert_eq!(body_mode(&head).unwrap(), HttpBodyMode::UntilEof);

    let bare = parse_http_head("HTTP/1.1 200 OK\r\n").unwrap();
    assert_eq!(body_mode(&bare).unwrap(), HttpBodyMode::UntilEof);
}

#[test]
fn decode_chunked_reassembles_multiple_chunks() {
    let decoded = decode_chunked(b"4\r\nWiki\r\n5;name=value\r\npedia\r\n0\r\n\r\n", 16).unwrap();
    assert_eq!(decoded, b"Wikipedia");
}

#[test]
fn decode_chunked_accepts_terminal_zero_and_trailers() {
    let decoded = decode_chunked(b"1\r\na\r\n0\r\nX-Trace: yes\r\n\r\n", 8).unwrap();
    assert_eq!(decoded, b"a");
}

#[test]
fn decode_chunked_rejects_oversize_output() {
    assert_eq!(
        decode_chunked(b"5\r\nhello\r\n0\r\n\r\n", 4),
        Err(NetError::OversizeBody(4))
    );
}

#[test]
fn decode_chunked_rejects_bad_or_truncated_frames() {
    assert!(matches!(
        decode_chunked(b"not-hex\r\n", 16),
        Err(NetError::InvalidChunkSize(_))
    ));
    assert_eq!(
        decode_chunked(b"5\r\nabc", 16),
        Err(NetError::TruncatedChunk)
    );
    assert_eq!(
        decode_chunked(b"1\r\na\n0\r\n\r\n", 16),
        Err(NetError::InvalidChunkDelimiter)
    );
    assert_eq!(decode_chunked(b"0\r\n", 16), Err(NetError::TruncatedChunk));
}

#[test]
fn rejects_invalid_head() {
    assert!(matches!(
        parse_http_head("HTTP/1.1 notastatus OK\r\n"),
        Err(NetError::InvalidHead(_))
    ));
    assert!(matches!(
        parse_http_head("HTTP/1.1 200 OK\r\nNoColonHeader\r\n"),
        Err(NetError::InvalidHead(_))
    ));
}

#[test]
fn line_decoder_splits_partial_line_across_pushes() {
    let mut decoder = LineDecoder::new();
    let first = decoder.push(b"hello\r\nwor");
    assert_eq!(first, vec![b"hello".to_vec()]);
    assert!(decoder.has_buffered());

    let second = decoder.push(b"ld\ntrailing");
    assert_eq!(second, vec![b"world".to_vec()]);

    assert_eq!(decoder.flush(), Some(b"trailing".to_vec()));
    assert_eq!(decoder.flush(), None);
}

#[test]
fn sse_joins_multiple_data_lines() {
    let mut decoder = SseDecoder::new();
    let events = decoder.push(b"event: message\ndata: line one\ndata: line two\n\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("message"));
    assert_eq!(events[0].data, "line one\nline two");
}

#[test]
fn sse_ignores_comments_and_dispatches_on_blank_line() {
    let mut decoder = SseDecoder::new();
    // Comment line then a record split across two pushes.
    assert!(decoder.push(b": keep-alive\ndata: hi").is_empty());
    let events = decoder.push(b"\n\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event, None);
    assert_eq!(events[0].data, "hi");
}

#[test]
fn ndjson_buffers_partial_final_line_until_flush() {
    let mut decoder = NdjsonDecoder::new();
    let first = decoder
        .push(
            br#"{"a":1}
{"b":2}
{"c":"#,
        )
        .unwrap();
    assert_eq!(
        first,
        vec![r#"{"a":1}"#.to_owned(), r#"{"b":2}"#.to_owned()]
    );

    let second = decoder.push(b"3}").unwrap();
    assert!(second.is_empty());

    assert_eq!(decoder.flush(), Some(r#"{"c":3}"#.to_owned()));
    assert_eq!(decoder.flush(), None);
}

#[test]
fn ndjson_rejects_oversize_line() {
    let mut decoder = NdjsonDecoder::with_max_line_bytes(4);
    // Completed line over the limit fails closed.
    assert_eq!(decoder.push(b"toolong\n"), Err(NetError::OversizeBody(4)));

    // A buffered partial that grows past the limit also fails closed.
    let mut buffered = NdjsonDecoder::with_max_line_bytes(4);
    assert_eq!(buffered.push(b"abc").map(|r| r.len()), Ok(0));
    assert_eq!(buffered.push(b"de"), Err(NetError::OversizeBody(4)));
}

#[test]
fn parse_url_for_scheme_requires_the_scheme() {
    let parts = parse_url_for_scheme("https://api.example.com/v1/chat", "https", "/").unwrap();
    assert_eq!(parts.scheme, "https");
    assert_eq!(parts.host, "api.example.com");
    assert_eq!(parts.port, 443);
    assert_eq!(parts.path, "/v1/chat");

    assert_eq!(
        parse_url_for_scheme("http://api.example.com/x", "https", "/"),
        Err(NetError::UnexpectedScheme {
            expected: "https".to_owned(),
            found: "http".to_owned(),
        })
    );
}

#[test]
fn parse_url_for_scheme_applies_default_path_when_absent() {
    // No path -> the caller's default is substituted.
    let parts = parse_url_for_scheme("http://host:9000", "http", "/v1/models").unwrap();
    assert_eq!(parts.port, 9000);
    assert_eq!(parts.path, "/v1/models");

    // An explicit path is kept as-is.
    let explicit = parse_url_for_scheme("http://host/actual", "http", "/v1/models").unwrap();
    assert_eq!(explicit.path, "/actual");
}

#[test]
fn parse_url_for_scheme_resolves_ws_default_port() {
    let u = parse_url_for_scheme_preserving_path("ws://host/path", "ws", "/").unwrap();
    assert_eq!(u.port, 80);
    assert_eq!(u.host, "host");
    assert_eq!(u.path, "/path");
}

#[test]
fn parse_url_for_scheme_preserving_resolves_wss_default_port() {
    let u = parse_url_for_scheme_preserving_path("wss://host/socket", "wss", "/").unwrap();
    assert_eq!(u.port, 443);
    assert_eq!(u.host, "host");
    assert_eq!(u.path, "/socket");
}

#[test]
fn parse_url_for_scheme_preserving_keeps_http_https_behavior() {
    // http/https default ports and explicit ports are unchanged.
    let http = parse_url_for_scheme_preserving_path("http://host", "http", "/base").unwrap();
    assert_eq!(http.port, 80);
    assert_eq!(http.path, "/base"); // no path component -> default substituted

    let https =
        parse_url_for_scheme_preserving_path("https://host:8443/v1", "https", "/base").unwrap();
    assert_eq!(https.port, 8443);
    assert_eq!(https.path, "/v1");
}

#[test]
fn parse_url_for_scheme_preserving_rejects_scheme_mismatch() {
    assert_eq!(
        parse_url_for_scheme_preserving_path("http://host/x", "ws", "/"),
        Err(NetError::UnexpectedScheme {
            expected: "ws".to_owned(),
            found: "http".to_owned(),
        })
    );
}

#[test]
fn parse_url_for_scheme_preserving_keeps_trailing_slash() {
    // A caller's trailing slash is significant and preserved verbatim, unlike
    // `parse_url`/`parse_url_for_scheme` which trim it.
    let with_slash = parse_url_for_scheme_preserving_path("ws://host/a/", "ws", "/def").unwrap();
    assert_eq!(with_slash.path, "/a/");

    // A bare authority with a lone `/` yields `/`, NOT the default path: the
    // default is only substituted when there is no path component at all.
    let root = parse_url_for_scheme_preserving_path("ws://host/", "ws", "/def").unwrap();
    assert_eq!(root.path, "/");

    // No path component at all -> the caller's default path is substituted.
    let none = parse_url_for_scheme_preserving_path("ws://host", "ws", "/def").unwrap();
    assert_eq!(none.path, "/def");

    // Contrast: `parse_url_for_scheme` trims the trailing slash and substitutes
    // the default for a lone `/`.
    assert_eq!(
        parse_url_for_scheme("http://host/a/", "http", "/def")
            .unwrap()
            .path,
        "/a"
    );
}

#[test]
fn parse_url_for_scheme_preserving_rejects_unsupported_and_bad_port() {
    // Unknown scheme without an explicit port has no default.
    assert!(matches!(
        parse_url_for_scheme_preserving_path("ftp://host/x", "ftp", "/"),
        Err(NetError::UnsupportedScheme(_))
    ));
    // A non-numeric port is rejected.
    assert!(matches!(
        parse_url_for_scheme_preserving_path("ws://host:nope/x", "ws", "/"),
        Err(NetError::InvalidPort(_))
    ));
    // An explicit port on an otherwise-unknown scheme is accepted (ports are
    // data; only default resolution is table-bound).
    let u = parse_url_for_scheme_preserving_path("ftp://host:21/x", "ftp", "/").unwrap();
    assert_eq!(u.port, 21);
}

#[test]
fn read_capped_line_reads_line_then_reports_eof() {
    let mut reader = std::io::BufReader::new(&b"hello\nworld"[..]);
    let mut buf = String::new();
    assert_eq!(
        read_capped_line(&mut reader, &mut buf, 64).unwrap(),
        CapOutcome::Line
    );
    assert_eq!(buf, "hello\n");
    assert_eq!(
        read_capped_line(&mut reader, &mut buf, 64).unwrap(),
        CapOutcome::Line
    );
    assert_eq!(buf, "world");
    assert_eq!(
        read_capped_line(&mut reader, &mut buf, 64).unwrap(),
        CapOutcome::Eof
    );
    assert!(buf.is_empty());
}

#[test]
fn read_capped_line_reports_too_large_over_cap() {
    let mut reader = std::io::BufReader::new(&b"0123456789\n"[..]);
    let mut buf = String::new();
    assert_eq!(
        read_capped_line(&mut reader, &mut buf, 4).unwrap(),
        CapOutcome::TooLarge
    );
}

#[test]
fn read_head_until_double_crlf_returns_full_head() {
    let raw = b"GET / HTTP/1.1\r\nHost: x\r\n\r\nBODY";
    let mut reader = std::io::Cursor::new(&raw[..]);
    match read_head_until_double_crlf(&mut reader, 64 * 1024).unwrap() {
        HeadOutcome::Head(head) => assert_eq!(head, b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"),
        other => panic!("expected Head, got {other:?}"),
    }
}

#[test]
fn read_head_until_double_crlf_classifies_eof_truncated_and_oversize() {
    // Clean EOF: nothing sent.
    let mut empty = std::io::Cursor::new(&b""[..]);
    assert_eq!(
        read_head_until_double_crlf(&mut empty, 1024).unwrap(),
        HeadOutcome::Eof
    );

    // Truncated: bytes but no terminator.
    let mut partial = std::io::Cursor::new(&b"GET / HTTP/1.1\r\n"[..]);
    match read_head_until_double_crlf(&mut partial, 1024).unwrap() {
        HeadOutcome::Truncated(bytes) => assert_eq!(bytes, b"GET / HTTP/1.1\r\n"),
        other => panic!("expected Truncated, got {other:?}"),
    }

    // Over cap before the terminator.
    let mut big = std::io::Cursor::new(&b"AAAAAAAAAAAAAAAA"[..]);
    assert_eq!(
        read_head_until_double_crlf(&mut big, 4).unwrap(),
        HeadOutcome::TooLarge
    );
}
