//! Field encoders used by table/list citizen descriptors.
//!
//! These helpers keep descriptor read constructors on the same validators as
//! live table backends: path fields go through [`TablePath`], and operation
//! fields go through [`decode_table_op`].

use sim_kernel::{Error, Expr, Result, Symbol};

use crate::{TablePath, decode_table_op};

/// Encode/decode `Vec<(Symbol, Expr)>` as an expression map.
pub mod entries {
    use super::*;

    /// Encodes the symbol/expr entries as an [`Expr::Map`].
    pub fn encode(entries: &[(Symbol, Expr)]) -> Expr {
        Expr::Map(
            entries
                .iter()
                .map(|(key, value)| (Expr::Symbol(key.clone()), value.clone()))
                .collect(),
        )
    }

    /// Decodes an [`Expr::Map`] into symbol/expr entries, erroring on a non-map
    /// expression or a non-symbol key.
    pub fn decode(expr: &Expr) -> Result<Vec<(Symbol, Expr)>> {
        let Expr::Map(entries) = expr else {
            return Err(field_error("entries", "expected map"));
        };
        entries
            .iter()
            .map(|(key, value)| {
                let Expr::Symbol(key) = key else {
                    return Err(field_error("entries", "expected symbol key"));
                };
                Ok((key.clone(), value.clone()))
            })
            .collect()
    }
}

/// Encode/decode `Vec<Expr>` as an expression list.
pub mod expr_list {
    use super::*;

    /// Encodes the items as an [`Expr::List`].
    pub fn encode(items: &[Expr]) -> Expr {
        Expr::List(items.to_vec())
    }

    /// Decodes an [`Expr::List`] into its items, erroring on a non-list
    /// expression.
    pub fn decode(expr: &Expr) -> Result<Vec<Expr>> {
        let Expr::List(items) = expr else {
            return Err(field_error("items", "expected list"));
        };
        Ok(items.clone())
    }
}

/// Encode/decode `Symbol` as an expression symbol.
pub mod symbol {
    use super::*;

    /// Encodes the symbol as an [`Expr::Symbol`].
    pub fn encode(symbol: &Symbol) -> Expr {
        Expr::Symbol(symbol.clone())
    }

    /// Decodes an [`Expr::Symbol`], erroring on any other expression.
    pub fn decode(expr: &Expr) -> Result<Symbol> {
        let Expr::Symbol(symbol) = expr else {
            return Err(field_error("symbol", "expected symbol"));
        };
        Ok(symbol.clone())
    }
}

/// Encode/decode validated path segments.
pub mod path_segments {
    use super::*;

    /// Encodes the path segments as an [`Expr::List`] of strings.
    pub fn encode(segments: &[String]) -> Expr {
        Expr::List(
            segments
                .iter()
                .map(|segment| Expr::String(segment.clone()))
                .collect(),
        )
    }

    /// Decodes a list of string segments, validating each through [`TablePath`]
    /// and erroring on a non-list, a non-string segment, or an illegal segment.
    pub fn decode(expr: &Expr) -> Result<Vec<String>> {
        let Expr::List(items) = expr else {
            return Err(field_error("path", "expected list"));
        };
        let mut path = TablePath::new();
        for item in items {
            let Expr::String(segment) = item else {
                return Err(field_error("path", "expected string segment"));
            };
            path.push(segment)
                .map_err(|_| field_error("path", format!("illegal segment {segment:?}")))?;
        }
        Ok(path.segments().to_vec())
    }
}

/// Encode/decode a table operation expression validated by `decode_table_op`.
pub mod table_op_expr {
    use super::*;

    /// Returns the table-operation expression unchanged for encoding.
    pub fn encode(op: &Expr) -> Expr {
        op.clone()
    }

    /// Validates the expression as a table operation via [`decode_table_op`] and
    /// returns it unchanged, erroring when it is not a valid operation.
    pub fn decode(expr: &Expr) -> Result<Expr> {
        decode_table_op(expr)
            .map_err(|err| field_error("operation", format!("invalid table operation: {err:?}")))?;
        Ok(expr.clone())
    }
}

fn field_error(field: &str, message: impl Into<String>) -> Error {
    Error::Eval(format!("table citizen field {field}: {}", message.into()))
}
