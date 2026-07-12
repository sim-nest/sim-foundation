//! Capability-name readers for `Expr` data.

use sim_kernel::{CapabilityName, Error, Expr, Result};

/// Parses a nil, singleton symbol/string, list, or vector into capability names.
///
/// Symbols are rendered through their standard display form, so qualified
/// symbols keep their namespace in the resulting capability name.
pub fn capability_names_from_expr(expr: &Expr) -> Result<Vec<CapabilityName>> {
    match expr {
        Expr::Nil => Ok(Vec::new()),
        Expr::List(items) | Expr::Vector(items) => {
            items.iter().map(capability_name_from_expr).collect()
        }
        Expr::Symbol(_) | Expr::String(_) => Ok(vec![capability_name_from_expr(expr)?]),
        _ => Err(Error::TypeMismatch {
            expected: "capability list",
            found: "non-list",
        }),
    }
}

fn capability_name_from_expr(expr: &Expr) -> Result<CapabilityName> {
    match expr {
        Expr::Symbol(symbol) => Ok(CapabilityName::new(symbol.to_string())),
        Expr::String(text) => Ok(CapabilityName::new(text.clone())),
        _ => Err(Error::TypeMismatch {
            expected: "capability symbol or string",
            found: "non-capability",
        }),
    }
}
