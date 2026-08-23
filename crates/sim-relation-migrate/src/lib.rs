//! Checked relational schema evolution.
//!
//! Migration programs are admitted before a provider sees them. Admission
//! proves a single revision chain, exact schema transitions, typed backfills,
//! and the declared final target.
//!
//! ```
//! use sim_relation_migrate::{derive_lossless, OperationKind};
//! use sim_relation_schema::{fixtures, AcceptAllValues};
//! let schema = fixtures::document(&AcceptAllValues).unwrap();
//! assert!(derive_lossless(&schema, &schema).unwrap().is_empty());
//! let _: Option<OperationKind> = None;
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod migration;

pub use migration::*;
