//! Open relational records built directly on the kernel data substrate.
//!
//! A logical domain is a [`DomainSpec`] in a [`DomainCatalog`], not an enum
//! variant. Storage providers consume its [`StorageRepr`], while runtime Shape
//! resolution is deliberately left to `sim-relation-shape`.
//!
//! ```
//! use sim_kernel::{Ref, Symbol};
//! use sim_relation_core::{DomainCatalog, DomainId, DomainSpec, DomainTrait, StorageRepr};
//!
//! let uuid = DomainSpec::new(
//!     DomainId::new(Symbol::qualified("example", "uuid")).unwrap(),
//!     StorageRepr::Text,
//!     Ref::Symbol(Symbol::qualified("example", "UuidShape")),
//!     [DomainTrait::Equatable, DomainTrait::Ordered],
//! ).unwrap();
//! let catalog = DomainCatalog::new([uuid]).unwrap();
//! assert!(catalog.get(&DomainId::new(Symbol::qualified("example", "uuid")).unwrap()).is_some());
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod domain;
mod names;
mod record;

pub use domain::{
    BaseDomain, DomainCatalog, DomainError, DomainSpec, DomainTrait, StorageRepr, StorageValue,
};
pub use names::{
    BindingName, ColumnName, ConstraintName, DomainId, FieldName, IndexName, NameError,
    ParameterName, ProviderName, RevisionName, SourceName, TableName,
};
pub use record::{Cell, FieldType, RelationId, Row, RowError, RowType, ToRelationDatum};
