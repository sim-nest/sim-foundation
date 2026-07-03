//! Server-Sent Events record decoding.

use crate::line::LineDecoder;

/// A decoded Server-Sent Events record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` field value, if any was present in the record.
    pub event: Option<String>,
    /// The `data:` payload. Multiple `data:` lines are joined with `\n`.
    pub data: String,
}

/// Incremental Server-Sent Events decoder.
///
/// Accumulates `event:` and `data:` field lines and emits an [`SseEvent`] when a
/// blank line terminates the record (standard SSE dispatch). Multiple `data:`
/// lines within one record are joined with `\n`. Comment lines (those starting
/// with `:`) and unknown fields are ignored. Field values have a single leading
/// space after the colon stripped, per the SSE specification.
///
/// `push` does the newline framing internally via [`LineDecoder`]; callers that
/// already have whole lines can use [`push_line`](SseDecoder::push_line).
#[derive(Debug, Default)]
pub struct SseDecoder {
    lines: LineDecoder,
    event: Option<String>,
    data: Vec<String>,
    have_record: bool,
}

impl SseDecoder {
    /// Create an empty decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw bytes; return any records completed by blank lines.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        let mut events = Vec::new();
        for line in self.lines.push(bytes) {
            // SSE payloads are UTF-8; lossy decoding keeps the framer robust and
            // pushes any invalid-byte handling onto the caller's data parse.
            let text = String::from_utf8_lossy(&line);
            if let Some(event) = self.push_line(&text) {
                events.push(event);
            }
        }
        events
    }

    /// Feed a single already-framed line (without its terminating newline).
    /// Returns an [`SseEvent`] when this line is the blank line that dispatches
    /// the accumulated record.
    pub fn push_line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            return self.dispatch();
        }
        if line.starts_with(':') {
            // Comment line: ignored, but it still marks an active record so a
            // following blank line dispatches an empty event only if fields set.
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, rest)) => (field, rest.strip_prefix(' ').unwrap_or(rest)),
            None => (line, ""),
        };
        match field {
            "event" => {
                self.event = Some(value.to_owned());
                self.have_record = true;
            }
            "data" => {
                self.data.push(value.to_owned());
                self.have_record = true;
            }
            _ => {}
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        if !self.have_record {
            return None;
        }
        let event = self.event.take();
        let data = std::mem::take(&mut self.data).join("\n");
        self.have_record = false;
        Some(SseEvent { event, data })
    }

    /// Flush any buffered partial line and any accumulated-but-undispatched
    /// record (a stream that ended without a final blank line).
    pub fn flush(&mut self) -> Option<SseEvent> {
        if let Some(line) = self.lines.flush() {
            let text = String::from_utf8_lossy(&line);
            if let Some(event) = self.push_line(&text) {
                return Some(event);
            }
        }
        self.dispatch()
    }
}
