//! Typed read view over a config table.

use sim_kernel::Expr;
use sim_value::access::{as_i64, as_str, field_any};

use crate::{ConfigError, ConfigResult, ConfigTable};

/// Borrowed typed accessors over one config table.
#[derive(Clone, Copy, Debug)]
pub struct ConfigView<'a> {
    table: &'a ConfigTable,
}

impl<'a> ConfigView<'a> {
    /// Creates a view over `table`.
    pub fn new(table: &'a ConfigTable) -> Self {
        Self { table }
    }

    /// Returns the raw expression for `key`.
    pub fn get(&self, key: &str) -> Option<&'a Expr> {
        field_any(&self.table.table, key)
    }

    /// Reads an optional string field.
    pub fn string(&self, key: &str) -> Option<&'a str> {
        self.get(key).and_then(as_str)
    }

    /// Reads a required string field.
    pub fn required_string(&self, key: &str) -> ConfigResult<&'a str> {
        self.string(key)
            .ok_or_else(|| self.required_error(key, "a string"))
    }

    /// Reads an optional boolean field.
    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(Expr::Bool(value)) => Some(*value),
            _ => None,
        }
    }

    /// Reads a required boolean field.
    pub fn required_bool(&self, key: &str) -> ConfigResult<bool> {
        self.bool(key)
            .ok_or_else(|| self.required_error(key, "a bool"))
    }

    /// Reads an optional integer field.
    pub fn i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(as_i64)
    }

    /// Reads a required integer field.
    pub fn required_i64(&self, key: &str) -> ConfigResult<i64> {
        self.i64(key)
            .ok_or_else(|| self.required_error(key, "an integer"))
    }

    /// Borrows a list field.
    pub fn list(&self, key: &str) -> Option<&'a [Expr]> {
        match self.get(key) {
            Some(Expr::List(items)) => Some(items),
            _ => None,
        }
    }

    /// Borrows a nested table field.
    pub fn table(&self, key: &str) -> Option<&'a [(Expr, Expr)]> {
        match self.get(key) {
            Some(Expr::Map(entries)) => Some(entries),
            _ => None,
        }
    }

    fn required_error(&self, key: &str, expected: &'static str) -> ConfigError {
        if self.get(key).is_some() {
            ConfigError::TypeMismatch {
                key: key.to_owned(),
                expected,
            }
        } else {
            ConfigError::MissingField {
                key: key.to_owned(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sim_kernel::Symbol;
    use sim_value::build::{int, map, text};

    use super::*;

    #[test]
    fn view_reads_typed_fields() {
        let table = ConfigTable::new(
            Symbol::qualified("model", "defaults"),
            map(vec![
                ("provider", text("modeled")),
                ("enabled", Expr::Bool(true)),
                ("limit", int(3)),
            ]),
        )
        .unwrap();
        let view = ConfigView::new(&table);

        assert_eq!(view.required_string("provider").unwrap(), "modeled");
        assert!(view.required_bool("enabled").unwrap());
        assert_eq!(view.required_i64("limit").unwrap(), 3);
        assert_eq!(
            view.required_string("limit").unwrap_err(),
            ConfigError::TypeMismatch {
                key: "limit".to_owned(),
                expected: "a string"
            }
        );
    }
}
