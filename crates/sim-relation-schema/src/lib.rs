//! Validated logical schema intent and normalized physical catalog evidence.
//!
//! ```
//! use sim_relation_schema::{fixtures, AcceptAllValues};
//! let schema = fixtures::document(&AcceptAllValues).unwrap();
//! assert_eq!(schema.tables().len(), 2);
//! assert_ne!(schema.id().unwrap(), fixtures::ledger(&AcceptAllValues).unwrap().id().unwrap());
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod builders;
/// Exact logical fixtures for the supported store families.
pub mod fixtures;
mod model;
mod physical;
mod validation;

pub use builders::{ColumnBuilder, SchemaBuilder, TableBuilder};
pub use model::{
    CheckConstraint, Column, Constraint, DefaultValue, ForeignKey, GeneratedValue, Index,
    PrimaryKey, Schema, Table, UniqueConstraint, View,
};
pub use physical::{PhysicalColumn, PhysicalIndex, PhysicalSchema, PhysicalTable};
pub use validation::{AcceptAllValues, SchemaError, ValueShapeValidator};
