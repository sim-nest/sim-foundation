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

#[cfg(test)]
mod tests {
    use sim_kernel::Symbol;

    use super::*;

    #[test]
    fn config_field_name_is_key_identity_not_map_lookup() {
        assert_eq!(
            config_field_name(&Expr::Symbol(Symbol::new("mode"))),
            Some("mode")
        );
        assert_eq!(
            config_field_name(&Expr::String("mode".into())),
            Some("mode")
        );
        assert_eq!(
            config_field_name(&Expr::Symbol(Symbol::qualified("config", "mode"))),
            None
        );
        assert_eq!(config_field_name(&Expr::Bool(true)), None);
    }
}
