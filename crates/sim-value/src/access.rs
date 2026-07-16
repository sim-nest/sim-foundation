//! Reading and immutable updates for kernel `Expr` data.
//!
//! `field` matches an unqualified symbol key equal to `name` -- the authored
//! SIM-record behavior. `field_q` covers qualified keys, and `field_any`
//! accepts either a bare-symbol key or an `Expr::String` key for provider-style
//! maps. The split prevents a silent behavior change for callers that relied on
//! either form.
//!
//! Immutable update helpers follow the same distinction. [`set`] and
//! [`remove`] operate on the visible field name across bare-symbol and string
//! keys so provider/config maps do not retain a stale readable value beside a
//! newly written one. [`set_strict`] and [`remove_strict`] are the authored
//! SIM-record variants: they only touch bare-symbol keys and leave provider
//! string keys alone.

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

/// Build a [`Error::TypeMismatch`] whose `found` label names the actual `Expr`
/// variant via [`expr_kind`](crate::kind::expr_kind). The one home for the
/// `Err(Error::TypeMismatch { expected, found: expr_kind(other) })` tail that
/// every typed slice reader across the constellation re-grew.
fn type_mismatch(expected: &'static str, found: &Expr) -> Error {
    Error::TypeMismatch {
        expected,
        found: crate::kind::expr_kind(found),
    }
}

/// Look up a required *bare-symbol*-keyed field in an entry slice, with a
/// context-labeled error when missing. The bare-key analog of [`entry_required`]
/// (which also accepts `Expr::String` keys): the typed `entry_required_*`
/// readers build on this so they are drop-in replacements for the bare-symbol
/// `string_field`/`symbol_field`/`bool_field`/`list_field` forks that stream,
/// music, fabric, and view crates each re-grew without loosening key matching.
fn entry_required_bare<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    context: &str,
) -> Result<&'a Expr> {
    entry_field(entries, name)
        .ok_or_else(|| Error::Eval(format!("{context} is missing field {name}")))
}

/// Read a required string-valued field from an entry slice by *bare-symbol* key.
/// Returns a [`Error::TypeMismatch`] naming the found variant when the field is
/// present but not an `Expr::String`. The typed, slice-level counterpart of
/// [`required_str`] and the one home for the bare-symbol `string_field` readers.
/// Use [`entry_required_str_any`] when string keys must also match.
pub fn entry_required_str<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<&'a str> {
    match entry_required_bare(entries, name, expected)? {
        Expr::String(value) => Ok(value),
        other => Err(type_mismatch(expected, other)),
    }
}

/// Read a required symbol-valued field from an entry slice by *bare-symbol* key,
/// borrowing the [`Symbol`]. Bare-symbol counterpart of the `symbol_field` forks.
pub fn entry_required_sym<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<&'a Symbol> {
    match entry_required_bare(entries, name, expected)? {
        Expr::Symbol(value) => Ok(value),
        other => Err(type_mismatch(expected, other)),
    }
}

/// Read a required bool-valued field (`Expr::Bool`) from an entry slice by
/// *bare-symbol* key. Bare-symbol counterpart of the `bool_field` forks.
pub fn entry_required_bool(
    entries: &[(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<bool> {
    match entry_required_bare(entries, name, expected)? {
        Expr::Bool(value) => Ok(*value),
        other => Err(type_mismatch(expected, other)),
    }
}

/// Borrow a required list-valued field's items (`Expr::List`) from an entry
/// slice by *bare-symbol* key. Bare-symbol counterpart of the `list_field` forks.
pub fn entry_required_list<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<&'a [Expr]> {
    match entry_required_bare(entries, name, expected)? {
        Expr::List(items) => Ok(items),
        other => Err(type_mismatch(expected, other)),
    }
}

/// Namespace-agnostic sibling of [`entry_required_str`]: matches a bare-symbol
/// OR `Expr::String` key (via [`entry_required`]/[`entry_field_any`]). Use this
/// for provider records (OpenAI, Ollama, MCP) that mix symbol and string keys.
pub fn entry_required_str_any<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<&'a str> {
    match entry_required(entries, name, expected)? {
        Expr::String(value) => Ok(value),
        other => Err(type_mismatch(expected, other)),
    }
}

/// Namespace-agnostic sibling of [`entry_required_sym`] (bare-symbol OR string
/// key), borrowing the [`Symbol`].
pub fn entry_required_sym_any<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<&'a Symbol> {
    match entry_required(entries, name, expected)? {
        Expr::Symbol(value) => Ok(value),
        other => Err(type_mismatch(expected, other)),
    }
}

/// Namespace-agnostic sibling of [`entry_required_bool`] (bare-symbol OR string
/// key).
pub fn entry_required_bool_any(
    entries: &[(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<bool> {
    match entry_required(entries, name, expected)? {
        Expr::Bool(value) => Ok(*value),
        other => Err(type_mismatch(expected, other)),
    }
}

/// Namespace-agnostic sibling of [`entry_required_list`] (bare-symbol OR string
/// key), borrowing the list items.
pub fn entry_required_list_any<'a>(
    entries: &'a [(Expr, Expr)],
    name: &str,
    expected: &'static str,
) -> Result<&'a [Expr]> {
    match entry_required(entries, name, expected)? {
        Expr::List(items) => Ok(items),
        other => Err(type_mismatch(expected, other)),
    }
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
        Expr::Number(number) if number.domain.name.as_ref() == "i64" => {
            number.canonical.parse::<i64>().ok()
        }
        _ => None,
    }
}

/// Read a number value's canonical literal as `u64`.
pub fn as_u64(value: &Expr) -> Option<u64> {
    match value {
        Expr::Number(number) if number.domain.name.as_ref() == "u64" => {
            number.canonical.parse::<u64>().ok()
        }
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

fn set_matching<F>(map: &Expr, name: &str, value: Expr, matches: F) -> Expr
where
    F: Fn(&Expr, &str) -> bool,
{
    let entries: &[(Expr, Expr)] = match map {
        Expr::Map(entries) => entries,
        _ => &[],
    };
    let mut updated = Vec::with_capacity(entries.len().saturating_add(1));
    let mut replacement = Some(value);
    let mut matched = false;

    for (key, existing) in entries {
        if matches(key, name) {
            if !matched {
                updated.push((key.clone(), replacement.take().unwrap()));
                matched = true;
            }
        } else {
            updated.push((key.clone(), existing.clone()));
        }
    }

    if !matched {
        updated.push((sym(name), replacement.take().unwrap()));
    }

    Expr::Map(updated)
}

/// Set (or insert) a visible field by name, matching either a bare-symbol or
/// string key, preserving sibling keys in a new map value.
///
/// When duplicates exist under the same visible name, the first matching entry
/// keeps its original key spelling and later duplicates are dropped.
pub fn set(map: &Expr, name: &str, value: Expr) -> Expr {
    set_matching(map, name, value, key_is_any)
}

/// Set (or insert) a strict bare-symbol field, preserving sibling keys in a
/// new map value.
///
/// This authored-SIM-record variant ignores provider-style string keys with the
/// same visible name rather than creating an ambiguous overlay.
pub fn set_strict(map: &Expr, name: &str, value: Expr) -> Expr {
    if matches!(map, Expr::Map(entries) if entries.iter().any(|(key, _)| key_is_any(key, name)) && !entries.iter().any(|(key, _)| key_is(key, name)))
    {
        return map.clone();
    }
    set_matching(map, name, value, key_is)
}

fn remove_matching<F>(map: &Expr, name: &str, matches: F) -> Expr
where
    F: Fn(&Expr, &str) -> bool,
{
    let entries: &[(Expr, Expr)] = match map {
        Expr::Map(entries) => entries,
        _ => &[],
    };
    Expr::Map(
        entries
            .iter()
            .filter(|(key, _)| !matches(key, name))
            .cloned()
            .collect(),
    )
}

/// Remove a visible field by name, matching either a bare-symbol or string
/// key, and returning a new map value.
pub fn remove(map: &Expr, name: &str) -> Expr {
    remove_matching(map, name, key_is_any)
}

/// Remove a strict bare-symbol field, returning a new map value.
pub fn remove_strict(map: &Expr, name: &str) -> Expr {
    remove_matching(map, name, key_is)
}
