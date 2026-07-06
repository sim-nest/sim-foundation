//! Policy-free, cap-as-argument bounded readers over `std::io`.
//!
//! Several runtime libs each hand-rolled the same defensive read loops: a
//! single line bounded to a byte cap, and an HTTP head accumulated until the
//! `\r\n\r\n` terminator. These readers are the one home for that framing. They
//! stay policy-free: the caller passes the cap (64 KiB, 1 MiB, ...) as an
//! argument and maps the returned outcome onto its own error (`HostError`,
//! a `413` response, `ReadOutcome::TooLarge`, ...). No socket, TLS, or timeout
//! policy lives here.

use std::io::{self, BufRead, Read};

/// The outcome of a single capped line read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapOutcome {
    /// A line (including its trailing newline, if any) was read within `cap`;
    /// `buf` holds it.
    Line,
    /// The line reached `cap` bytes before a newline was seen; `buf` holds the
    /// capped prefix. The caller decides how to reject it.
    TooLarge,
    /// End of input reached with no further bytes; `buf` is empty.
    Eof,
}

/// The outcome of reading an HTTP head up to the `\r\n\r\n` terminator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadOutcome {
    /// A complete head terminated by `\r\n\r\n`, returned as raw bytes
    /// (terminator included) for the caller to parse.
    Head(Vec<u8>),
    /// The head reached `cap` bytes before the terminator; the caller rejects it.
    TooLarge,
    /// End of input before any byte was read: the peer opened and closed without
    /// sending a request.
    Eof,
    /// End of input after a partial head: the connection was truncated
    /// mid-headers. Carries the bytes read so far.
    Truncated(Vec<u8>),
}

/// Read one line into `buf`, bounded to `cap` bytes.
///
/// `buf` is cleared first, then filled via a `Read::take(cap + 1)` ceiling so a
/// hostile unterminated line is bounded before it can grow memory. The returned
/// [`CapOutcome`] tells the caller whether a [`Line`](CapOutcome::Line) was read,
/// the line was [`TooLarge`](CapOutcome::TooLarge), or input hit
/// [`Eof`](CapOutcome::Eof); the caller maps that onto its own error type.
pub fn read_capped_line<R: BufRead>(
    reader: &mut R,
    buf: &mut String,
    cap: usize,
) -> io::Result<CapOutcome> {
    buf.clear();
    let read = Read::take(&mut *reader, cap as u64 + 1).read_line(buf)?;
    if buf.len() > cap {
        return Ok(CapOutcome::TooLarge);
    }
    if read == 0 {
        return Ok(CapOutcome::Eof);
    }
    Ok(CapOutcome::Line)
}

/// Read an HTTP head byte-by-byte until the `\r\n\r\n` terminator, bounded to
/// `cap` bytes.
///
/// Retries on [`io::ErrorKind::Interrupted`]; any other I/O error is returned.
/// The [`HeadOutcome`] distinguishes a complete [`Head`](HeadOutcome::Head), an
/// over-cap [`TooLarge`](HeadOutcome::TooLarge), a clean [`Eof`](HeadOutcome::Eof)
/// (nothing sent), and a [`Truncated`](HeadOutcome::Truncated) head (peer closed
/// mid-headers), leaving the reject/None policy to the caller.
pub fn read_head_until_double_crlf<R: Read>(reader: &mut R, cap: usize) -> io::Result<HeadOutcome> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) if head.is_empty() => return Ok(HeadOutcome::Eof),
            Ok(0) => return Ok(HeadOutcome::Truncated(head)),
            Ok(_) => {
                head.push(byte[0]);
                if head.len() > cap {
                    return Ok(HeadOutcome::TooLarge);
                }
                if head.ends_with(b"\r\n\r\n") {
                    return Ok(HeadOutcome::Head(head));
                }
            }
            Err(ref error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}
