#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Reusable, side-effect-free HTTP/streaming parsing and wire-format helpers.
//!
//! This is a leaf parsing crate: it contains URL parsing, HTTP response-head
//! parsing, body-mode classification, chunked-transfer decoding, line framing,
//! and SSE/NDJSON record decoders, plus small text encoders for shared wire
//! identifiers. It deliberately contains **no** socket/TLS I/O and **no**
//! application policy -- callers own transport and event mapping.
//!
//! Multiple runtime libs share these parsers for wire framing, line
//! accumulation, and head parsing while keeping transport and event mapping in
//! their own layers.
//!
//! # Example
//!
//! `LineDecoder` frames newline-delimited input, buffering a partial final line
//! across pushes:
//!
//! ```
//! use sim_lib_net_core::LineDecoder;
//!
//! let mut decoder = LineDecoder::new();
//! assert_eq!(decoder.push(b"a\nb"), vec![b"a".to_vec()]);
//! assert_eq!(decoder.push(b"c\n"), vec![b"bc".to_vec()]);
//! ```

mod body;
mod cookbook;
mod error;
mod hex;
mod http;
mod line;
mod ndjson;
mod read;
mod sse;
mod url;

pub use body::decode_chunked;
pub use cookbook::{ResponseHeadDemo, response_head_demo};
pub use error::NetError;
pub use hex::hex_encode;
pub use http::{HttpBodyMode, HttpHead, body_mode, build_http_request_head, parse_http_head};
pub use line::LineDecoder;
pub use ndjson::NdjsonDecoder;
pub use read::{CapOutcome, HeadOutcome, read_capped_line, read_head_until_double_crlf};
pub use sse::{SseDecoder, SseEvent};
pub use url::{UrlParts, parse_url, parse_url_for_scheme, parse_url_for_scheme_preserving_path};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
