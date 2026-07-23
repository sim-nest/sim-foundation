//! Shared table substrate: path validation, path-reference resolution, and the
//! table operation protocol.
//!
//! Table backends (`sim-table-db`, `sim-table-remote`, ...) share one
//! path-segment predicate, one absolute/relative reference syntax, and one
//! `table/<op>` call protocol on the wire. This crate is the home for all three:
//!
//! - [`capabilities`]: canonical fs/find/edit/exec/net capability names and
//!   compatibility alias checks for host-effect call sites;
//! - [`path`]: the legal-segment predicate ([`is_legal_table_segment`]) and a
//!   small [`TablePath`] accumulator that validates as it grows, plus
//!   [`TablePathRef`] for escaped absolute and relative references;
//! - [`op`]: the [`TableOp`] model plus [`encode_table_op`]/[`decode_table_op`],
//!   which round-trip through the kernel `Expr` graph using the exact wire
//!   spellings that `sim-table-remote` already speaks.
//!
//! It depends only on `sim-kernel` and `sim-value`, adding data ergonomics and
//! protocol shape rather than runtime behavior, so it does not touch the kernel
//! boundary.
//!
//! # Example
//!
//! ```
//! use sim_table_core::is_legal_table_segment;
//!
//! assert!(is_legal_table_segment("nodes"));
//! assert!(!is_legal_table_segment("")); // empty
//! assert!(!is_legal_table_segment("..")); // parent escape
//! assert!(!is_legal_table_segment("a/b")); // path separator
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod capabilities;
pub mod citizen_fields;
pub mod manifest;
pub mod op;
pub mod path;

pub use manifest::backend_manifest;
pub use op::{TableOp, TableOpError, decode_table_op, encode_table_op};
pub use path::{
    MAX_TABLE_PATH_SEGMENTS, MAX_TABLE_PATH_TEXT_BYTES, TablePath, TablePathError, TablePathRef,
    TablePathRefError, TablePathRefPart, is_legal_table_segment,
};

#[cfg(test)]
mod tests;
