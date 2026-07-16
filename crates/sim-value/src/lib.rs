//! Ergonomic construction and access for kernel `Expr` data.
//!
//! The kernel `Expr` enum is bare data with no ergonomic surface, so dozens of
//! libs independently re-grew `sym`/`number`/`field`/`set`/`expr_kind` and a
//! `k`/`i` path-addressing scheme. This crate is the one home for that. It
//! depends only on `sim-kernel` and adds data ergonomics, not runtime behavior,
//! so it does not touch the kernel boundary.
//!
//! - [`build`]: constructors (`sym`, `int`, `float`, `text`, `list`, `map`, ...);
//! - [`capability_names_from_expr`]: parses capability-name expressions;
//! - [`access`]: reading and immutable updates (`field`, `field_any`, `set`,
//!   `set_strict`, `remove`, ...);
//! - [`edit`]: side-effect-free exact and line-range text edits;
//! - [`kind`]: the one `Expr` variant classifier (`expr_kind`);
//! - [`path`]: one value-addressing primitive (`Path`, `get`, `set_at`).
//!
//! # Example
//!
//! ```
//! use sim_value::access::{field, set};
//! use sim_value::build::{int, map, sym};
//!
//! let value = map(vec![("a", int(1)), ("b", int(2))]);
//! assert_eq!(field(&value, "a"), Some(&int(1)));
//!
//! let updated = set(&value, "a", int(9));
//! assert_eq!(field(&updated, "a"), Some(&int(9)));
//! assert_eq!(field(&updated, "b"), Some(&int(2))); // siblings preserved
//! let _ = sym("ok");
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod access;
pub mod build;
pub mod capability;
pub mod edit;
pub mod kind;
pub mod path;

pub use capability::capability_names_from_expr;

#[cfg(test)]
mod tests;
