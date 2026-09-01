//! Complete provider-neutral relational plans with a sealed admission boundary.
//!
//! Providers receive [`CheckedQuery`] or [`CheckedMutation`], never SQL text or
//! unchecked syntax. The public algebra binds every field explicitly, including
//! correlated subqueries and the `excluded` row of conflict updates.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod admission;
mod datum;
mod model;

pub use admission::{admit_mutation, admit_query};
pub use model::*;
