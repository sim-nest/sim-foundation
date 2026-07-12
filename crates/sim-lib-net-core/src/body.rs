//! HTTP response body framing helpers.

use crate::NetError;

/// Decode a complete `Transfer-Encoding: chunked` body.
///
/// The decoded body is capped at `cap` bytes. Chunk extensions and trailer
/// lines are accepted, but the body must contain the terminal empty trailer
/// line (`0\r\n\r\n` for a response without trailers).
///
/// # Examples
///
/// ```
/// use sim_lib_net_core::decode_chunked;
///
/// let decoded = decode_chunked(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n", 32).unwrap();
/// assert_eq!(decoded, b"Wikipedia");
/// ```
pub fn decode_chunked(mut input: &[u8], cap: usize) -> Result<Vec<u8>, NetError> {
    let mut decoded = Vec::new();
    loop {
        let line_end = find_crlf(input).ok_or(NetError::TruncatedChunk)?;
        let size = parse_chunk_size(&input[..line_end])?;
        input = &input[line_end + 2..];

        if size == 0 {
            validate_trailers(input)?;
            return Ok(decoded);
        }

        let next_len = decoded
            .len()
            .checked_add(size)
            .ok_or(NetError::OversizeBody(cap))?;
        if next_len > cap {
            return Err(NetError::OversizeBody(cap));
        }

        let needed = size.checked_add(2).ok_or(NetError::InvalidChunkSize(
            "size overflows frame length".to_owned(),
        ))?;
        if input.len() < needed {
            return Err(NetError::TruncatedChunk);
        }
        if &input[size..needed] != b"\r\n" {
            return Err(NetError::InvalidChunkDelimiter);
        }

        decoded.extend_from_slice(&input[..size]);
        input = &input[needed..];
    }
}

fn find_crlf(input: &[u8]) -> Option<usize> {
    input.windows(2).position(|window| window == b"\r\n")
}

fn parse_chunk_size(line: &[u8]) -> Result<usize, NetError> {
    let text = std::str::from_utf8(line)
        .map_err(|err| NetError::InvalidChunkSize(format!("size line is not UTF-8: {err}")))?;
    let size_text = text.split(';').next().unwrap_or_default().trim();
    if size_text.is_empty() {
        return Err(NetError::InvalidChunkSize("size line is empty".to_owned()));
    }
    usize::from_str_radix(size_text, 16)
        .map_err(|err| NetError::InvalidChunkSize(format!("invalid size: {err}")))
}

fn validate_trailers(mut input: &[u8]) -> Result<(), NetError> {
    loop {
        let line_end = find_crlf(input).ok_or(NetError::TruncatedChunk)?;
        input = &input[line_end + 2..];
        if line_end == 0 {
            if input.is_empty() {
                return Ok(());
            }
            return Err(NetError::InvalidChunkDelimiter);
        }
    }
}
