//! Reading and immutable updates for kernel `Expr` data.
//!
//! `field` matches an unqualified symbol key equal to `name` -- the existing
//! majority behavior across the repo. `field_q` covers qualified keys. The
//! split prevents a silent behavior change for callers that relied on either
//! form. `set`/`remove` are immutable (clone, then modify) to match every
//! current caller.

use sim_kernel::{Error, Expr, Result, Symbol};

use crate::build::sym;

fn key_is(key: &Expr, name: &str) -> bool {
    matches!(key, Expr::Symbol(symbol) if &*symbol.name == name && symbol.namespace.is_none())
}

/// True for a bare-symbol key OR an `Expr::String` key equal to `name`.
fn key_is_any(key: &Expr, name: &str) -> bool {
    key_is(key, name) || matches!(key, Expr::String(text) if text == name)
}

/// The unqualified field name spelled by a key, if it has one. Bare symbol keys
/// report their name; string keys report their text; qualified symbol and other
/// keys report `None`.
fn key_name(key: &Expr) -> Option<&str> {
    match key {
        Expr::Symbol(symbol) if symbol.namespace.is_none() => Some(&symbol.name),
        Expr::String(text) => Some(text),
        _ => None,
    }
}

/// Look up an unqualified-keyed field in a map's entry slice. The slice-level
/// primitive [`field`] delegates to; use it when a caller already holds the
/// `&[(Expr, Expr)]` entries (provider codecs, MCP) instead of rebuilding a map.
pub fn entry_field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries
        .iter()
        .find_map(|(key, value)| key_is(key, name).then_some(value))
}

/// Look up a field in an entry slice, accepting a bare-symbol OR `Expr::String`
/// key (the slice primitive behind [`field_any`]).
pub fn entry_field_any<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries
        .iter()
        .find_map(|(key, value)| key_is_any(key, name).then_some(value))
}

/// Look up an unqualified-keyed field by name.
pub fn field<'a>(map: &'a Expr, name: &str) -> Option<&'a Expr> {
    match map {
        Expr::Map(entries) => entry_field(entries, name),
        _ => None,
    }
}

/// Look up a qualified-keyed field by namespace and name.
pub fn field_q<'a>(map: &'a Expr, ns: &str, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = map else {
        return None;
    };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(symbol) if symbol.namespace.as_deref() == Some(ns) && &*symbol.name == name)
            .then_some(value)
    })
}

/// Look up a field by name, accepting either a bare-symbol key or an
/// `Expr::String` key. Use this for provider-style records (OpenAI, Ollama,
/// MCP) that mix symbol and string keys; use [`field`] when only the
/// bare-symbol form is valid.
pub fn field_any<'a>(map: &'a Expr, name: &str) -> Option<&'a Expr> {
    match map {
        Expr::Map(entries) => entry_field_any(entries, name),
        _ => None,
    }
}

/// Look up a required field, returning a context-labeled error when it is
/// missing. Accepts either key form, matching [`field_any`].
pub fn required<'a>(map: &'a Expr, name: &str, context: &str) -> Result<&'a Expr> {
    field_any(map, name).ok_or_else(|| Error::Eval(format!("{context} is missing field {name}")))
}

/// Look up a required field in a map's entry slice, with a context-labeled error
/// when missing. The slice analog of [`required`] and the one home for the
/// `required_field(entries, name)` forks. Accepts either key form.
pub fn entry_required<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    context: &str,
) -> Result<&'a Expr> {
    entry_field_any(entries, name)
        .ok_or_else(|| Error::Eval(format!("{context} is missing field {name}")))
}

/// Read a required string-valued field, with a context label for diagnostics.
/// This is the one home for the `string_field`/`required_field`-style readers
/// that coerce to `&str`; callers wanting a domain-specific error keep a thin
/// local wrapper. Accepts either key form, matching [`field_any`].
pub fn required_str<'a>(map: &'a Expr, name: &str, context: &str) -> Result<&'a str> {
    as_str(required(map, name, context)?)
        .ok_or_else(|| Error::Eval(format!("{context} field {name} is not a string")))
}

/// Read a required symbol-valued field, with a context label for diagnostics.
pub fn required_sym(map: &Expr, name: &str, context: &str) -> Result<Symbol> {
    match required(map, name, context)? {
        Expr::Symbol(symbol) => Ok(symbol.clone()),
        _ => Err(Error::Eval(format!(
            "{context} field {name} is not a symbol"
        ))),
    }
}

/// Read a required bool-valued field (`Expr::Bool`), with a context label.
pub fn required_bool(map: &Expr, name: &str, context: &str) -> Result<bool> {
    match required(map, name, context)? {
        Expr::Bool(value) => Ok(*value),
        _ => Err(Error::Eval(format!("{context} field {name} is not a bool"))),
    }
}

/// Borrow a required map-valued field's entries, with a context label. This is
/// the context-carrying counterpart of [`map_entries`] for a named field.
pub fn required_map<'a>(map: &'a Expr, name: &str, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match required(map, name, context)? {
        Expr::Map(entries) => Ok(entries),
        _ => Err(Error::Eval(format!("{context} field {name} is not a map"))),
    }
}

/// Borrow a map value's entries, or return a `TypeMismatch` error labelled with
/// `expected`. This is the one home for the `map_fields(expr, "...")` helper
/// that MCP, skill, and codec crates each re-grew.
pub fn map_entries<'a>(map: &'a Expr, expected: &'static str) -> Result<&'a [(Expr, Expr)]> {
    match map {
        Expr::Map(entries) => Ok(entries),
        _ => Err(Error::TypeMismatch {
            expected,
            found: "non-map",
        }),
    }
}

/// List the field names present in `map` that are not in `known`. Keys that are
/// neither bare symbols nor strings are ignored. Use this for open-record
/// validation (reject or warn on unexpected fields).
pub fn extra_fields<'a>(map: &'a Expr, known: &[&str]) -> Vec<&'a str> {
    let Expr::Map(entries) = map else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|(key, _)| key_name(key))
        .filter(|name| !known.contains(name))
        .collect()
}

/// Read a symbol-valued field.
pub fn field_sym(map: &Expr, name: &str) -> Option<Symbol> {
    match field(map, name) {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

/// Read a string-valued field.
pub fn field_str<'a>(map: &'a Expr, name: &str) -> Option<&'a str> {
    field(map, name).and_then(as_str)
}

/// Read an integer-valued field.
pub fn field_i64(map: &Expr, name: &str) -> Option<i64> {
    field(map, name).and_then(as_i64)
}

/// Read a float-valued field.
pub fn field_f64(map: &Expr, name: &str) -> Option<f64> {
    field(map, name).and_then(as_f64)
}

/// Read a bool-valued field (`Expr::Bool`). Returns `None` when absent or not a
/// bool. This is the optional counterpart of [`required_bool`].
pub fn field_bool(map: &Expr, name: &str) -> Option<bool> {
    match field_any(map, name) {
        Some(Expr::Bool(value)) => Some(*value),
        _ => None,
    }
}

/// Read a number value's canonical literal as `i64`.
pub fn as_i64(value: &Expr) -> Option<i64> {
    match value {
        Expr::Number(number) => number.canonical.parse::<i64>().ok(),
        _ => None,
    }
}

/// Read a number value's canonical literal as `f64`.
pub fn as_f64(value: &Expr) -> Option<f64> {
    match value {
        Expr::Number(number) => number.canonical.parse::<f64>().ok(),
        _ => None,
    }
}

/// Borrow a string value's contents.
pub fn as_str(value: &Expr) -> Option<&str> {
    match value {
        Expr::String(text) => Some(text),
        _ => None,
    }
}

/// Set (or insert) an unqualified-keyed field, preserving sibling keys, in a
/// new map value.
pub fn set(map: &Expr, name: &str, value: Expr) -> Expr {
    let mut entries = match map {
        Expr::Map(entries) => entries.clone(),
        _ => Vec::new(),
    };
    if let Some(slot) = entries.iter_mut().find(|(key, _)| key_is(key, name)) {
        slot.1 = value;
    } else {
        entries.push((sym(name), value));
    }
    Expr::Map(entries)
}

/// Remove an unqualified-keyed field, returning a new map value.
pub fn remove(map: &Expr, name: &str) -> Expr {
    let entries = match map {
        Expr::Map(entries) => entries.clone(),
        _ => Vec::new(),
    };
    Expr::Map(
        entries
            .into_iter()
            .filter(|(key, _)| !key_is(key, name))
            .collect(),
    )
}
