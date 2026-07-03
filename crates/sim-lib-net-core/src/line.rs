//! Incremental newline framing (LineDecoder).

/// Incremental newline framer.
///
/// Accumulates pushed bytes and yields complete lines split on `\n`, stripping
/// a single trailing `\r` from each (so both `\n` and `\r\n` framing work). A
/// partial final line with no terminating `\n` stays buffered across pushes
/// until either a later push completes it or [`flush`](LineDecoder::flush) is
/// called.
///
/// This is the framing half of `sim-lib-agent-runner-http`'s old per-decoder
/// `line_buffer` loop, lifted into one shared, tested type.
#[derive(Debug, Default)]
pub struct LineDecoder {
    buf: Vec<u8>,
}

impl LineDecoder {
    /// Create an empty decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `bytes` and return every line completed by a `\n`. The trailing
    /// `\r` of a `\r\n` pair is stripped. Bytes after the last `\n` remain
    /// buffered.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(bytes);
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

    /// Take any buffered remainder (an unterminated final line). Returns `None`
    /// when nothing is buffered. A single trailing `\r` is stripped to match
    /// [`push`](LineDecoder::push).
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        if self.buf.is_empty() {
            return None;
        }
        let mut line = std::mem::take(&mut self.buf);
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        Some(line)
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
}
