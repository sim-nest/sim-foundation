//! Deterministic cookbook builders for net-core recipes.

use crate::{HttpBodyMode, NdjsonDecoder, NetError, SseDecoder, body_mode, parse_http_head};

/// Report produced by the HTTP response-head cookbook recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseHeadDemo {
    /// Parsed status code.
    pub status: u16,
    /// Parsed reason phrase.
    pub reason: String,
    /// Parsed content type header.
    pub content_type: String,
    /// Parsed body framing mode.
    pub body_mode: HttpBodyMode,
    /// Event name decoded from a modeled SSE frame.
    pub sse_event: Option<String>,
    /// Payload decoded from a modeled SSE frame.
    pub sse_data: String,
    /// First NDJSON record decoded from a modeled stream.
    pub ndjson_record: String,
}

/// Build the modeled response-head parse report used by the cookbook recipe.
pub fn response_head_demo() -> Result<ResponseHeadDemo, NetError> {
    let head = parse_http_head(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n",
    )?;
    let body_mode = body_mode(&head)?;
    let content_type = head
        .header("Content-Type")
        .expect("fixture has content type")
        .to_owned();

    let mut sse = SseDecoder::new();
    let mut events = sse.push(b"event: ready\ndata: {\"ok\":true}\n\n")?;
    let event = events
        .pop()
        .expect("cookbook SSE fixture dispatches one event");

    let mut ndjson = NdjsonDecoder::new();
    let ndjson_record = ndjson
        .push(b"{\"ok\":true}\n")?
        .pop()
        .expect("cookbook NDJSON fixture emits one record");

    Ok(ResponseHeadDemo {
        status: head.status,
        reason: head.reason,
        content_type,
        body_mode,
        sse_event: event.event,
        sse_data: event.data,
        ndjson_record,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_head_demo_parses_head_sse_and_ndjson() {
        let demo = response_head_demo().expect("valid response-head demo");

        assert_eq!(demo.status, 200);
        assert_eq!(demo.reason, "OK");
        assert_eq!(demo.content_type, "application/json");
        assert_eq!(demo.body_mode, HttpBodyMode::ContentLength(11));
        assert_eq!(demo.sse_event.as_deref(), Some("ready"));
        assert_eq!(demo.sse_data, "{\"ok\":true}");
        assert_eq!(demo.ndjson_record, "{\"ok\":true}");
    }
}
