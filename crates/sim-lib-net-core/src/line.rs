//! Incremental newline framing (LineDecoder).

use crate::NetError;

/// Default maximum line length for incremental stream decoders.
pub const DEFAULT_MAX_LINE_BYTES: usize = 64 * 1024;

/// Incremental newline framer.
///
/// Accumulates pushed bytes and yields complete lines split on `\n`, stripping
/// a single trailing `\r` from each (so both `\n` and `\r\n` framing work). A
/// partial final line with no terminating `\n` stays buffered across pushes
/// until either a later push completes it or [`flush`](LineDecoder::flush) is
/// called.
///
/// [`new`](LineDecoder::new) uses [`DEFAULT_MAX_LINE_BYTES`]. Untrusted streams
/// should call [`push_checked`](LineDecoder::push_checked) so over-long lines
/// are reported as [`NetError::LineTooLong`]. [`unbounded`](LineDecoder::unbounded)
/// is the explicit opt-out for trusted in-memory inputs.
///
/// This is the framing half of `sim-lib-agent-runner-http`'s old per-decoder
/// `line_buffer` loop, lifted into one shared, tested type.
#[derive(Debug)]
pub struct LineDecoder {
    buf: Vec<u8>,
    max_line_bytes: Option<usize>,
}

impl Default for LineDecoder {
    fn default() -> Self {
        Self::with_max_line_bytes(DEFAULT_MAX_LINE_BYTES)
    }
}

impl LineDecoder {
    /// Create an empty decoder using [`DEFAULT_MAX_LINE_BYTES`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an empty decoder with no line limit.
    ///
    /// Use this only for trusted, already-bounded inputs.
    pub fn unbounded() -> Self {
        Self {
            buf: Vec::new(),
            max_line_bytes: None,
        }
    }

    /// Create an empty decoder that rejects any line longer than
    /// `max_line_bytes`.
    pub fn with_max_line_bytes(max_line_bytes: usize) -> Self {
        Self {
            buf: Vec::new(),
            max_line_bytes: Some(max_line_bytes),
        }
    }

    /// Append `bytes` and return every line completed by a `\n`. The trailing
    /// `\r` of a `\r\n` pair is stripped. Bytes after the last `\n` remain
    /// buffered.
    ///
    /// This compatibility convenience panics if the configured line cap is
    /// exceeded. Use [`push_checked`](LineDecoder::push_checked) for untrusted
    /// input so the caller can map the error into its own domain.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.push_checked(bytes)
            .expect("line decoder cap exceeded; use push_checked for fallible handling")
    }

    /// Append `bytes` and return every completed line.
    ///
    /// The configured cap is checked before bytes are appended, so an over-long
    /// completed line or still-unterminated line leaves the existing buffered
    /// state unchanged.
    pub fn push_checked(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, NetError> {
        self.check_append(bytes)?;
        self.buf.extend_from_slice(bytes);
        Ok(self.drain_complete_lines())
    }

    /// Take any buffered remainder (an unterminated final line). Returns `None`
    /// when nothing is buffered. A single trailing `\r` is stripped to match
    /// [`push`](LineDecoder::push).
    ///
    /// This compatibility convenience panics if the configured line cap is
    /// exceeded. Use [`flush_checked`](LineDecoder::flush_checked) for
    /// untrusted input.
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        self.flush_checked()
            .expect("line decoder cap exceeded; use flush_checked for fallible handling")
    }

    /// Take any buffered remainder, checking the configured cap first.
    pub fn flush_checked(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        self.check_len(self.buf.len())?;
        if self.buf.is_empty() {
            return Ok(None);
        }
        let mut line = std::mem::take(&mut self.buf);
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        Ok(Some(line))
    }

    /// Number of bytes currently buffered as a partial line.
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Borrow the bytes currently buffered as a partial (unterminated) line.
    pub fn buffered(&self) -> &[u8] {
        &self.buf
    }

    /// Whether any partial line is currently buffered.
    pub fn has_buffered(&self) -> bool {
        !self.buf.is_empty()
    }

    fn check_append(&self, bytes: &[u8]) -> Result<(), NetError> {
        let Some(max) = self.max_line_bytes else {
            return Ok(());
        };
        self.check_len(self.buf.len())?;
        let mut line_len = self.buf.len();
        for byte in bytes {
            if *byte == b'\n' {
                line_len = 0;
            } else {
                line_len = line_len.saturating_add(1);
                if line_len > max {
                    return Err(NetError::LineTooLong { max });
                }
            }
        }
        Ok(())
    }

    fn check_len(&self, len: usize) -> Result<(), NetError> {
        if let Some(max) = self.max_line_bytes
            && len > max
        {
            return Err(NetError::LineTooLong { max });
        }
        Ok(())
    }

    fn drain_complete_lines(&mut self) -> Vec<Vec<u8>> {
        let mut lines = Vec::new();
        while let Some(index) = self.buf.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buf.drain(..=index).collect::<Vec<u8>>();
            line.pop(); // drop the '\n'
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            lines.push(line);
        }
        lines
    }
}
