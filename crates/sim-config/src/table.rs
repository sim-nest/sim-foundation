//! Config Table and Dir data types.

use sim_kernel::{Expr, Symbol};

use crate::{ConfigError, ConfigResult, config_field_name, same_config_field};

/// One library's configuration table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigTable {
    /// Library id whose configuration this table carries.
    pub lib: Symbol,
    /// Configuration data; always an `Expr::Map` when built by this crate.
    pub table: Expr,
}

impl ConfigTable {
    /// Creates a table, rejecting non-map expressions.
    pub fn new(lib: Symbol, table: Expr) -> ConfigResult<Self> {
        if !matches!(table, Expr::Map(_)) {
            return Err(ConfigError::NonTableExpr);
        }
        reject_duplicate_fields(&table)?;
        Ok(Self {
            lib: canonical_lib_symbol(&lib)?,
            table,
        })
    }

    /// Borrows this table's map entries.
    pub fn entries(&self) -> ConfigResult<&[(Expr, Expr)]> {
        match &self.table {
            Expr::Map(entries) => Ok(entries),
            _ => Err(ConfigError::NonTableExpr),
        }
    }
}

/// A configuration Dir: library id to table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigDir {
    /// Tables carried by this Dir.
    pub entries: Vec<ConfigTable>,
}

impl ConfigDir {
    /// Creates an empty Dir.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a Dir with one library table.
    pub fn one(lib: Symbol, table: Expr) -> ConfigResult<Self> {
        Ok(Self {
            entries: vec![ConfigTable::new(lib, table)?],
        })
    }

    /// Finds a table by library id.
    pub fn table(&self, lib: &Symbol) -> Option<&ConfigTable> {
        self.entries.iter().find(|entry| &entry.lib == lib)
    }

    /// Finds a mutable table by library id.
    pub fn table_mut(&mut self, lib: &Symbol) -> Option<&mut ConfigTable> {
        self.entries.iter_mut().find(|entry| &entry.lib == lib)
    }

    /// Inserts or replaces a table for its library id.
    pub fn upsert(&mut self, table: ConfigTable) {
        if let Some(existing) = self.table_mut(&table.lib) {
            *existing = table;
        } else {
            self.entries.push(table);
        }
    }

    /// Builds a Dir from a map of library id keys to table values.
    pub fn from_dir_expr(expr: &Expr) -> ConfigResult<Self> {
        let Expr::Map(pairs) = expr else {
            return Err(ConfigError::NonDirExpr);
        };
        let mut entries = Vec::new();
        for (key, value) in pairs {
            let lib = symbol_key(key)?;
            if !matches!(value, Expr::Map(_)) {
                return Err(ConfigError::NonTableConfig { lib });
            }
            let table = ConfigTable::new(lib, value.clone())?;
            if entries
                .iter()
                .any(|prior: &ConfigTable| prior.lib == table.lib)
            {
                return Err(ConfigError::DuplicateLibId {
                    lib: table.lib.clone(),
                });
            }
            entries.push(table);
        }
        Ok(Self { entries })
    }

    /// Converts this Dir back to a kernel map expression.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(
            self.entries
                .iter()
                .map(|entry| (Expr::Symbol(entry.lib.clone()), entry.table.clone()))
                .collect(),
        )
    }
}

/// Parses a library id string into a [`Symbol`].
pub fn lib_symbol_from_str(id: &str) -> ConfigResult<Symbol> {
    if id.is_empty() {
        return Err(ConfigError::InvalidLibId { id: id.to_owned() });
    }
    match id.split_once('/') {
        Some((namespace, name)) if !namespace.is_empty() && !name.is_empty() => {
            if name.contains('/') {
                return Err(ConfigError::InvalidLibId { id: id.to_owned() });
            }
            Ok(Symbol::qualified(
                Symbol::checked(namespace)?.name,
                Symbol::checked(name)?.name,
            ))
        }
        Some(_) => Err(ConfigError::InvalidLibId { id: id.to_owned() }),
        None => Ok(Symbol::checked(id)?),
    }
}

fn canonical_lib_symbol(symbol: &Symbol) -> ConfigResult<Symbol> {
    lib_symbol_from_str(&symbol.as_qualified_str())
}

fn symbol_key(key: &Expr) -> ConfigResult<Symbol> {
    match key {
        Expr::Symbol(symbol) => canonical_lib_symbol(symbol),
        Expr::String(text) => lib_symbol_from_str(text),
        other => Err(ConfigError::UnsupportedDirKey { key: other.clone() }),
    }
}

fn reject_duplicate_fields(expr: &Expr) -> ConfigResult<()> {
    match expr {
        Expr::Map(entries) => {
            for (index, (key, value)) in entries.iter().enumerate() {
                if entries[..index]
                    .iter()
                    .any(|(prior_key, _)| same_config_field(prior_key, key))
                {
                    return Err(ConfigError::DuplicateField {
                        key: config_field_label(key),
                    });
                }
                reject_duplicate_fields(value)?;
            }
        }
        Expr::List(items) => {
            for item in items {
                reject_duplicate_fields(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn config_field_label(key: &Expr) -> String {
    config_field_name(key)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{key:?}"))
}

#[cfg(test)]
mod tests {
    use sim_value::build::{map, text};

    use super::*;

    #[test]
    fn dir_expr_accepts_symbol_and_string_keys() {
        let expr = Expr::Map(vec![
            (
                Expr::Symbol(Symbol::qualified("sim", "cookbook")),
                map(vec![("minimum_loaded", Expr::List(vec![]))]),
            ),
            (
                Expr::String("stream/host".to_owned()),
                map(vec![("audio_backend_regex", text("modeled"))]),
            ),
        ]);

        let dir = ConfigDir::from_dir_expr(&expr).unwrap();

        assert!(dir.table(&Symbol::qualified("sim", "cookbook")).is_some());
        assert!(dir.table(&Symbol::qualified("stream", "host")).is_some());
    }

    #[test]
    fn dir_expr_canonicalizes_single_symbol_slash_spelling() {
        let expr = Expr::Map(vec![(
            Expr::Symbol(Symbol::new("sim/cookbook")),
            map(vec![("minimum_loaded", Expr::List(vec![]))]),
        )]);

        let dir = ConfigDir::from_dir_expr(&expr).unwrap();

        assert_eq!(dir.entries[0].lib, Symbol::qualified("sim", "cookbook"));
    }

    #[test]
    fn config_dir_one_emits_canonical_symbol_key() {
        let dir = ConfigDir::one(
            Symbol::new("sim/cookbook"),
            map(vec![("minimum_loaded", Expr::List(vec![]))]),
        )
        .unwrap();

        assert_eq!(
            dir.to_expr(),
            Expr::Map(vec![(
                Expr::Symbol(Symbol::qualified("sim", "cookbook")),
                map(vec![("minimum_loaded", Expr::List(vec![]))]),
            )])
        );
    }

    #[test]
    fn dir_expr_rejects_non_table_entries() {
        let expr = Expr::Map(vec![(Expr::String("sim/cookbook".to_owned()), Expr::Nil)]);

        let error = ConfigDir::from_dir_expr(&expr).unwrap_err();

        assert_eq!(
            error,
            ConfigError::NonTableConfig {
                lib: Symbol::qualified("sim", "cookbook")
            }
        );
    }

    #[test]
    fn dir_expr_rejects_empty_library_ids() {
        let expr = Expr::Map(vec![(Expr::String(String::new()), map(vec![]))]);

        let error = ConfigDir::from_dir_expr(&expr).unwrap_err();

        assert_eq!(error, ConfigError::InvalidLibId { id: String::new() });
    }

    #[test]
    fn dir_expr_rejects_repeated_slashes() {
        let expr = Expr::Map(vec![(
            Expr::String("sim//cookbook".to_owned()),
            map(vec![("minimum_loaded", Expr::List(vec![]))]),
        )]);

        let error = ConfigDir::from_dir_expr(&expr).unwrap_err();

        assert_eq!(
            error,
            ConfigError::InvalidLibId {
                id: "sim//cookbook".to_owned(),
            }
        );
    }

    #[test]
    fn dir_expr_rejects_duplicate_equivalent_library_ids() {
        let expr = Expr::Map(vec![
            (
                Expr::String("sim/cookbook".to_owned()),
                map(vec![("minimum_loaded", Expr::List(vec![]))]),
            ),
            (
                Expr::Symbol(Symbol::new("sim/cookbook")),
                map(vec![("minimum_loaded", Expr::List(vec![]))]),
            ),
        ]);

        let error = ConfigDir::from_dir_expr(&expr).unwrap_err();

        assert_eq!(
            error,
            ConfigError::DuplicateLibId {
                lib: Symbol::qualified("sim", "cookbook"),
            }
        );
    }

    #[test]
    fn table_rejects_duplicate_equivalent_field_keys() {
        let table = Expr::Map(vec![
            (Expr::Symbol(Symbol::new("mode")), text("built-in")),
            (Expr::String("mode".to_owned()), text("work")),
        ]);

        let error = ConfigTable::new(Symbol::qualified("sim", "cookbook"), table).unwrap_err();

        assert_eq!(
            error,
            ConfigError::DuplicateField {
                key: "mode".to_owned()
            }
        );
    }

    #[test]
    fn table_rejects_duplicate_equivalent_nested_field_keys() {
        let table = Expr::Map(vec![(
            Expr::Symbol(Symbol::new("settings")),
            Expr::Map(vec![
                (Expr::Symbol(Symbol::new("mode")), text("built-in")),
                (Expr::String("mode".to_owned()), text("work")),
            ]),
        )]);

        let error = ConfigTable::new(Symbol::qualified("sim", "cookbook"), table).unwrap_err();

        assert_eq!(
            error,
            ConfigError::DuplicateField {
                key: "mode".to_owned()
            }
        );
    }
}
