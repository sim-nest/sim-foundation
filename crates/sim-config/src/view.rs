//! Typed read view over a config table.

use sim_kernel::Expr;
use sim_value::access::{as_i64, as_str};

use crate::{ConfigError, ConfigResult, ConfigTable, config_field_name};

/// Borrowed typed accessors over one config table.
#[derive(Clone, Copy, Debug)]
pub struct ConfigView<'a> {
    entries: &'a [(Expr, Expr)],
}

impl<'a> ConfigView<'a> {
    /// Creates a view over `table`.
    pub fn new(table: &'a ConfigTable) -> Self {
        let Expr::Map(entries) = &table.table else {
            unreachable!("ConfigTable guarantees an Expr::Map table")
        };
        Self { entries }
    }

    /// Creates a view over already-borrowed table entries.
    pub fn from_entries(entries: &'a [(Expr, Expr)]) -> Self {
        Self { entries }
    }

    /// Returns the raw expression for `key`.
    pub fn get(&self, key: &str) -> Option<&'a Expr> {
        self.entries.iter().find_map(|(field_key, value)| {
            config_field_name(field_key)
                .is_some_and(|field| field == key)
                .then_some(value)
        })
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

    /// Reads an optional list of string values.
    ///
    /// Missing fields read as an empty vector so callers can treat absent
    /// arrays as the natural zero value. A present non-list field, or any
    /// non-string list item, is reported as a type mismatch.
    pub fn string_array(&self, key: &str) -> ConfigResult<Vec<String>> {
        let Some(value) = self.get(key) else {
            return Ok(Vec::new());
        };
        let Expr::List(items) = value else {
            return Err(ConfigError::TypeMismatch {
                key: key.to_owned(),
                expected: "a string list",
            });
        };
        items
            .iter()
            .enumerate()
            .map(|(index, item)| match item {
                Expr::String(value) => Ok(value.clone()),
                _ => Err(ConfigError::TypeMismatch {
                    key: format!("{key}[{index}]"),
                    expected: "a string",
                }),
            })
            .collect()
    }

    /// Reads an optional repeated table field as borrowed table views.
    ///
    /// Config codecs represent `[[name]]` repeated tables as a list of map
    /// expressions. Missing fields read as an empty vector. A present field must
    /// be a list whose items are maps.
    pub fn tables(&self, key: &str) -> ConfigResult<Vec<ConfigView<'a>>> {
        let Some(value) = self.get(key) else {
            return Ok(Vec::new());
        };
        let Expr::List(items) = value else {
            return Err(ConfigError::TypeMismatch {
                key: key.to_owned(),
                expected: "a table list",
            });
        };
        items
            .iter()
            .enumerate()
            .map(|(index, item)| match item {
                Expr::Map(entries) => Ok(ConfigView::from_entries(entries)),
                _ => Err(ConfigError::TypeMismatch {
                    key: format!("{key}[{index}]"),
                    expected: "a table",
                }),
            })
            .collect()
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
    use sim_value::build::{int, list, map, text};

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

    #[test]
    fn view_reads_string_keyed_config_fields() {
        let table = ConfigTable::new(
            Symbol::qualified("model", "defaults"),
            Expr::Map(vec![
                (Expr::String("provider".to_owned()), text("modeled")),
                (Expr::String("limit".to_owned()), int(3)),
            ]),
        )
        .unwrap();
        let view = ConfigView::new(&table);

        assert_eq!(view.required_string("provider").unwrap(), "modeled");
        assert_eq!(view.required_i64("limit").unwrap(), 3);
    }

    #[test]
    fn view_reads_string_arrays_and_repeated_tables() {
        let table = ConfigTable::new(
            Symbol::qualified("sim", "cookbook"),
            map(vec![
                (
                    "minimum_loaded",
                    list(vec![text("codec/lisp"), text("numbers/i64")]),
                ),
                (
                    "loadable_lib",
                    list(vec![
                        map(vec![
                            ("id", text("numbers/i64")),
                            ("source", text("symbol:numbers/i64")),
                        ]),
                        map(vec![
                            ("id", text("numbers/cas")),
                            ("source", text("symbol:numbers/cas")),
                        ]),
                    ]),
                ),
            ]),
        )
        .unwrap();
        let view = ConfigView::new(&table);

        assert_eq!(
            view.string_array("minimum_loaded").unwrap(),
            ["codec/lisp", "numbers/i64"]
        );
        let loadable = view.tables("loadable_lib").unwrap();
        assert_eq!(loadable.len(), 2);
        assert_eq!(loadable[0].string("id"), Some("numbers/i64"));
        assert_eq!(loadable[1].string("source"), Some("symbol:numbers/cas"));
        assert!(view.string_array("missing").unwrap().is_empty());
        assert!(view.tables("missing").unwrap().is_empty());
    }

    #[test]
    fn view_reports_array_shape_errors() {
        let table = ConfigTable::new(
            Symbol::qualified("sim", "cookbook"),
            map(vec![
                ("minimum_loaded", text("codec/lisp")),
                ("loadable_lib", list(vec![text("bad")])),
            ]),
        )
        .unwrap();
        let view = ConfigView::new(&table);

        assert_eq!(
            view.string_array("minimum_loaded").unwrap_err(),
            ConfigError::TypeMismatch {
                key: "minimum_loaded".to_owned(),
                expected: "a string list"
            }
        );
        assert_eq!(
            view.tables("loadable_lib").unwrap_err(),
            ConfigError::TypeMismatch {
                key: "loadable_lib[0]".to_owned(),
                expected: "a table"
            }
        );
    }
}
