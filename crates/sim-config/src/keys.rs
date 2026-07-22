//! Canonical config field identity helpers.

use sim_kernel::Expr;

/// Returns the canonical unqualified config field name represented by `key`.
///
/// Config tables accept bare symbol keys and string keys for unqualified field
/// names. Qualified symbols and other expression keys do not spell a config
/// field name.
pub fn config_field_name(key: &Expr) -> Option<&str> {
    match key {
        Expr::Symbol(symbol) if symbol.namespace.is_none() => Some(symbol.name.as_ref()),
        Expr::String(text) => Some(text),
        _ => None,
    }
}

/// Returns true when two keys identify the same config field.
///
/// Unqualified symbol keys and string keys compare by their canonical field
/// names. Keys outside that config-field identity fall back to exact expression
/// equality.
pub fn same_config_field(left: &Expr, right: &Expr) -> bool {
    match (config_field_name(left), config_field_name(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}
