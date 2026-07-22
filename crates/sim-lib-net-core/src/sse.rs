//! Server-Sent Events record decoding.

use crate::{
    NetError,
    line::{DEFAULT_MAX_LINE_BYTES, LineDecoder},
};

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
/// already have whole lines can use
/// [`push_line_checked`](SseDecoder::push_line_checked).
///
/// [`new`](SseDecoder::new) uses [`DEFAULT_MAX_LINE_BYTES`] for both a single
/// line and the accumulated `data:` payload in one record.
/// [`unbounded`](SseDecoder::unbounded) is the explicit opt-out for trusted
/// in-memory inputs.
#[derive(Debug)]
pub struct SseDecoder {
    lines: LineDecoder,
    event: Option<String>,
    data: Vec<String>,
    data_bytes: usize,
    max_line_bytes: Option<usize>,
    have_record: bool,
}

impl Default for SseDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SseDecoder {
    /// Create an empty decoder using [`DEFAULT_MAX_LINE_BYTES`].
    pub fn new() -> Self {
        Self::with_max_line_bytes(DEFAULT_MAX_LINE_BYTES)
    }

    /// Create an empty decoder with no line or event-data limit.
    ///
    /// Use this only for trusted, already-bounded inputs.
    pub fn unbounded() -> Self {
        Self {
            lines: LineDecoder::unbounded(),
            event: None,
            data: Vec::new(),
            data_bytes: 0,
            max_line_bytes: None,
            have_record: false,
        }
    }

    /// Create an empty decoder that rejects any line, buffered partial line, or
    /// accumulated record data longer than `max_line_bytes`.
    pub fn with_max_line_bytes(max_line_bytes: usize) -> Self {
        Self {
            lines: LineDecoder::with_max_line_bytes(max_line_bytes),
            event: None,
            data: Vec::new(),
            data_bytes: 0,
            max_line_bytes: Some(max_line_bytes),
            have_record: false,
        }
    }

    /// Feed raw bytes; return any records completed by blank lines.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, NetError> {
        self.push_checked(bytes)
    }

    /// Feed raw bytes; return any records completed by blank lines.
    pub fn push_checked(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, NetError> {
        let mut events = Vec::new();
        for line in self.lines.push_checked(bytes)? {
            // SSE payloads are UTF-8; lossy decoding keeps the framer robust and
            // pushes any invalid-byte handling onto the caller's data parse.
            let text = String::from_utf8_lossy(&line);
            if let Some(event) = self.push_line_checked(&text)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// Feed a single already-framed line (without its terminating newline).
    /// Returns an [`SseEvent`] when this line is the blank line that dispatches
    /// the accumulated record.
    pub fn push_line(&mut self, line: &str) -> Option<SseEvent> {
        self.push_line_checked(line)
            .expect("sse decoder cap exceeded; use push_line_checked for fallible handling")
    }

    /// Feed a single already-framed line, checking line and accumulated data
    /// bounds.
    pub fn push_line_checked(&mut self, line: &str) -> Result<Option<SseEvent>, NetError> {
        self.check_line(line.len())?;
        if line.is_empty() {
            return Ok(self.dispatch());
        }
        if line.starts_with(':') {
            // Comment line: ignored. A following blank line dispatches only if
            // fields were set.
            return Ok(None);
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
                self.add_data(value)?;
                self.data.push(value.to_owned());
                self.have_record = true;
            }
            _ => {}
        }
        Ok(None)
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        if !self.have_record {
            return None;
        }
        let event = self.event.take();
        let data = std::mem::take(&mut self.data).join("\n");
        self.data_bytes = 0;
        self.have_record = false;
        Some(SseEvent { event, data })
    }

    /// Flush any buffered partial line and any accumulated-but-undispatched
    /// record (a stream that ended without a final blank line).
    pub fn flush(&mut self) -> Option<SseEvent> {
        self.flush_checked()
            .expect("sse decoder cap exceeded; use flush_checked for fallible handling")
    }

    /// Flush any buffered partial line and any accumulated-but-undispatched
    /// record, checking configured bounds first.
    pub fn flush_checked(&mut self) -> Result<Option<SseEvent>, NetError> {
        if let Some(line) = self.lines.flush_checked()? {
            let text = String::from_utf8_lossy(&line);
            if let Some(event) = self.push_line_checked(&text)? {
                return Ok(Some(event));
            }
        }
        Ok(self.dispatch())
    }

    fn add_data(&mut self, value: &str) -> Result<(), NetError> {
        let separator = usize::from(!self.data.is_empty());
        let next_len = self
            .data_bytes
            .saturating_add(separator)
            .saturating_add(value.len());
        self.check_line(next_len)?;
        self.data_bytes = next_len;
        Ok(())
    }

    fn check_line(&self, len: usize) -> Result<(), NetError> {
        if let Some(max) = self.max_line_bytes
            && len > max
        {
            return Err(NetError::LineTooLong { max });
        }
        Ok(())
    }
}
