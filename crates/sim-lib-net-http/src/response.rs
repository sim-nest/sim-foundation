use std::{io::BufRead, time::Instant};

use crate::{Error, Header, Policy, Request, Result, client::checkpoint, io_error};

pub(crate) const HEAD_CAP: usize = 64 * 1024;

pub(crate) fn read_chunked_stream(
    reader: &mut dyn BufRead,
    body: &mut Vec<u8>,
    request: &Request<'_>,
    policy: &Policy,
    started: Instant,
    on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
) -> Result<Vec<Header>> {
    loop {
        checkpoint(request, policy, started)?;
        let mut size_line = String::new();
        if reader.read_line(&mut size_line).map_err(io_error)? == 0 {
            return Err(Error::Protocol("truncated chunk size".into()));
        }
        if size_line.len() > HEAD_CAP || !size_line.ends_with("\r\n") {
            return Err(Error::Protocol("invalid chunk size line".into()));
        }
        let size_text = size_line
            .trim_end_matches("\r\n")
            .split(';')
            .next()
            .unwrap_or("");
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| Error::Protocol("invalid chunk size".into()))?;
        if size == 0 {
            break;
        }
        if body.len().saturating_add(size) > policy.max_response_bytes {
            return Err(Error::ResponseTooLarge {
                cap: policy.max_response_bytes,
            });
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader.read_exact(&mut body[start..]).map_err(io_error)?;
        on_chunk(&body[start..])?;
        let mut terminator = [0u8; 2];
        reader.read_exact(&mut terminator).map_err(io_error)?;
        if terminator != *b"\r\n" {
            return Err(Error::Protocol("invalid chunk terminator".into()));
        }
    }
    read_trailers(reader)
}

pub(crate) fn push_chunk(
    body: &mut Vec<u8>,
    chunk: &[u8],
    policy: &Policy,
    cb: &mut dyn FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    if body.len().saturating_add(chunk.len()) > policy.max_response_bytes {
        return Err(Error::ResponseTooLarge {
            cap: policy.max_response_bytes,
        });
    }
    cb(chunk)?;
    body.extend_from_slice(chunk);
    Ok(())
}

pub(crate) fn reject_ambiguous_response(headers: &[(String, String)]) -> Result<()> {
    let cls = headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .count();
    let tes = headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("transfer-encoding"))
        .count();
    if cls > 1 || (cls > 0 && tes > 0) || tes > 1 {
        return Err(Error::AmbiguousHeader);
    }
    if let Some((_, v)) = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("transfer-encoding"))
        && !v.eq_ignore_ascii_case("chunked")
    {
        return Err(Error::UnsupportedTransferFraming);
    }
    Ok(())
}

fn read_trailers(reader: &mut dyn BufRead) -> Result<Vec<Header>> {
    let mut trailers = Vec::new();
    let mut metadata_bytes = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(io_error)? == 0 {
            return Err(Error::Protocol("truncated trailers".into()));
        }
        metadata_bytes = metadata_bytes.saturating_add(line.len());
        if metadata_bytes > HEAD_CAP {
            return Err(Error::ResponseTooLarge { cap: HEAD_CAP });
        }
        if line == "\r\n" {
            break;
        }
        let line = line
            .strip_suffix("\r\n")
            .ok_or_else(|| Error::Protocol("invalid trailer line".into()))?;
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::Protocol("invalid trailer line".into()))?;
        trailers.push(Header::new(name, value.trim_start())?);
    }
    Ok(trailers)
}
