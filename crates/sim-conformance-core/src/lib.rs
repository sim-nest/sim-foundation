//! Neutral, effect-free conformance records for SIM.
//!
//! The crate separates immutable declarations from checked invocations and
//! receipts. Every record uses canonical kernel [`sim_kernel::Datum`] identity,
//! every scope is explicit, and support graphs must be acyclic before a result
//! can be admitted.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod binding;
mod checker;
mod digest;
mod fake;
mod graph;
mod identity;

pub use binding::*;
pub use checker::*;
pub use digest::*;
pub use fake::*;
pub use graph::*;
pub use identity::*;
