//! Config Table and Dir data types.

use sim_kernel::{Expr, Symbol};

use crate::{ConfigError, ConfigResult};

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
        Ok(Self { lib, table })
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
            entries.push(ConfigTable {
                lib,
                table: value.clone(),
            });
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

fn symbol_key(key: &Expr) -> ConfigResult<Symbol> {
    match key {
        Expr::Symbol(symbol) => Ok(symbol.clone()),
        Expr::String(text) => lib_symbol_from_str(text),
        other => Err(ConfigError::UnsupportedDirKey { key: other.clone() }),
    }
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
}
