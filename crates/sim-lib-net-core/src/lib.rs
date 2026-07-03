#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Reusable, side-effect-free HTTP/streaming parsing primitives.
//!
//! This is a leaf parsing crate: it contains URL parsing, HTTP response-head
//! parsing, body-mode classification, line framing, and SSE/NDJSON record
//! decoders. It deliberately contains **no** socket/TLS I/O and **no**
//! application policy -- callers own transport and event mapping.
//!
//! The behavior here was extracted from `sim-lib-agent-runner-http` so that
//! multiple runtime libs can share one tested implementation of the wire
//! framing rather than each hand-rolling line accumulation and head parsing.
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

mod error;
mod http;
mod line;
mod ndjson;
mod sse;
mod url;

pub use error::NetError;
pub use http::{HttpBodyMode, HttpHead, body_mode, parse_http_head};
pub use line::LineDecoder;
pub use ndjson::NdjsonDecoder;
pub use sse::{SseDecoder, SseEvent};
pub use url::{UrlParts, parse_url};

#[cfg(test)]
mod tests;
