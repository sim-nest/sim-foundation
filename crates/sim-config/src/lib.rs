//! Table and Dir substrate for layered SIM configuration.
//!
//! Configuration is plain kernel data: a [`ConfigTable`] is one library's
//! `Expr::Map`, and a [`ConfigDir`] maps library ids to those tables. This crate
//! provides the data model, merge/provenance rules, typed read views, and safe
//! path helpers used by loaders and codecs without introducing a parallel
//! configuration value enum.
//!
//! # Example
//!
//! ```
//! use sim_config::{ConfigDir, ConfigLayer, ConfigSource, merge_layers};
//! use sim_kernel::{Expr, Symbol};
//! use sim_value::build::{entry, map, text};
//!
//! let lib = Symbol::qualified("sim", "cookbook");
//! let lower = ConfigDir::one(lib.clone(), map(vec![("mode", text("built-in"))])).unwrap();
//! let upper = ConfigDir::one(lib.clone(), map(vec![("mode", text("work"))])).unwrap();
//! let effective = merge_layers(&[
//!     ConfigLayer::new(ConfigSource::BuiltIn { lib: lib.clone() }, lower),
//!     ConfigLayer::new(ConfigSource::Explicit { label: "work".into() }, upper),
//! ]);
//!
//! let table = effective.dir.table(&lib).unwrap();
//! assert_eq!(sim_value::access::field(&table.table, "mode"), Some(&Expr::String("work".into())));
//! assert_eq!(effective.trace[0].key, "mode");
//! let _ = entry("ok", Expr::Bool(true));
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fmt;

use sim_kernel::{Expr, Symbol, id::SymbolError};

pub mod merge;
pub mod path;
pub mod report;
pub mod source;
pub mod table;
pub mod view;

pub use merge::{ConfigLayer, EffectiveConfig, MergeTrace, merge_layers};
pub use path::{ConfigRoots, lib_config_path};
pub use report::{ConfigReport, ConfigReportEntry};
pub use source::{ConfigSource, ProbeMode};
pub use table::{ConfigDir, ConfigTable, lib_symbol_from_str};
pub use view::ConfigView;

/// Result type used by `sim-config` helpers.
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Error reported by config table, view, and path helpers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// A Dir expression was not an `Expr::Map`.
    NonDirExpr,
    /// A table expression was not an `Expr::Map`.
    NonTableExpr,
    /// A Dir entry for `lib` did not contain a table value.
    NonTableConfig {
        /// The library whose Dir entry was malformed.
        lib: Symbol,
    },
    /// A Dir key could not be interpreted as a library id.
    UnsupportedDirKey {
        /// The unsupported key expression.
        key: Expr,
    },
    /// A library id or id segment is not safe for config use.
    InvalidLibId {
        /// The rejected id.
        id: String,
    },
    /// A library config path segment was rejected.
    InvalidPathSegment {
        /// The rejected segment.
        segment: String,
    },
    /// A required field was absent from a config table.
    MissingField {
        /// The missing field name.
        key: String,
    },
    /// A config field had a different shape than the caller requested.
    TypeMismatch {
        /// The field name.
        key: String,
        /// The expected shape.
        expected: &'static str,
    },
}

impl From<SymbolError> for ConfigError {
    fn from(error: SymbolError) -> Self {
        Self::InvalidLibId {
            id: error.to_string(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonDirExpr => f.write_str("config dir must be an Expr::Map"),
            Self::NonTableExpr => f.write_str("config table must be an Expr::Map"),
            Self::NonTableConfig { lib } => {
                write!(f, "config for `{}` must be a table", lib.as_qualified_str())
            }
            Self::UnsupportedDirKey { key } => {
                write!(f, "config dir key {key:?} is not a supported library id")
            }
            Self::InvalidLibId { id } => write!(f, "invalid config library id `{id}`"),
            Self::InvalidPathSegment { segment } => {
                write!(f, "invalid config path segment `{segment}`")
            }
            Self::MissingField { key } => write!(f, "config field `{key}` is missing"),
            Self::TypeMismatch { key, expected } => {
                write!(f, "config field `{key}` is not {expected}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}
