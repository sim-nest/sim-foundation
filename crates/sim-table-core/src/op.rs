//! The `table/<op>` operation model and its `Expr` wire encoding.
//!
//! The wire spellings here match `sim-table-remote` exactly (see
//! `remote_dir.rs` for the client `call(cx, "<op>", ...)` sites and `site.rs`
//! for the `answer_table_request` matcher). Two spellings are NOT the obvious
//! ones and are matched deliberately:
//!
//! - [`TableOp::Delete`] encodes to `table/del` (not `table/delete`);
//! - [`TableOp::IsDir`] encodes to `table/dir?` (not `table/isdir`).
//!
//! Table entry ops (`get`, `set`, `has`, `del`) act on symbol-keyed table
//! entries and therefore preserve the full [`Symbol`] surface. Dir ops
//! (`mkdir`, `opendir`, `rmdir`, `dir?`) name child directories and therefore
//! accept only one safe, unqualified table-path segment.

use sim_kernel::{Expr, Symbol};
use sim_value::build::qsym;

use crate::path::is_legal_table_segment;

/// Expected wire state for compare-exchange.
#[derive(Clone, Debug, PartialEq)]
pub enum CompareExpected {
    /// The key must be absent.
    Absent,
    /// The key must contain this expression (including [`Expr::Nil`]).
    Value(Expr),
}

/// Replacement wire state for compare-exchange.
#[derive(Clone, Debug, PartialEq)]
pub enum CompareReplacement {
    /// Delete the key.
    Delete,
    /// Store this expression (including [`Expr::Nil`]).
    Value(Expr),
}

/// A single table operation, independent of any transport.
#[derive(Clone, Debug, PartialEq)]
pub enum TableOp {
    /// Read the value at `key`.
    Get(Symbol),
    /// Store `value` at `key`.
    Set(Symbol, Expr),
    /// Atomically mutate a key when its canonical expression matches.
    CompareExchange(Symbol, CompareExpected, CompareReplacement),
    /// Whether `key` is present.
    Has(Symbol),
    /// Remove `key`, returning the prior value. Wire op: `del`.
    Delete(Symbol),
    /// All keys in this table.
    Keys,
    /// All entries in this table.
    Entries,
    /// The number of entries.
    Len,
    /// Remove every entry.
    Clear,
    /// Create a subdirectory named `name`.
    Mkdir(Symbol),
    /// Open the subdirectory named `name`.
    Opendir(Symbol),
    /// Remove the subdirectory named `name`.
    Rmdir(Symbol),
    /// Whether `name` is a subdirectory. Wire op: `dir?`.
    IsDir(Symbol),
}

/// Why decoding an `Expr` into a [`TableOp`] failed.
#[derive(Clone, Debug, PartialEq)]
pub enum TableOpError {
    /// The `Expr` was not a `table/<op>` call at all.
    NotATableCall,
    /// The operator was in the `table` namespace but is not a known op.
    UnknownOp(String),
    /// The op was known but had the wrong number of arguments.
    BadArity(String),
    /// An argument had the wrong kind for the op.
    BadArg(String),
}

/// The wire op name for `op` (the unqualified `name` of the `table/<name>`
/// operator).
fn wire_name(op: &TableOp) -> &'static str {
    match op {
        TableOp::Get(_) => "get",
        TableOp::Set(_, _) => "set",
        TableOp::CompareExchange(_, _, _) => "cas",
        TableOp::Has(_) => "has",
        TableOp::Delete(_) => "del",
        TableOp::Keys => "keys",
        TableOp::Entries => "entries",
        TableOp::Len => "len",
        TableOp::Clear => "clear",
        TableOp::Mkdir(_) => "mkdir",
        TableOp::Opendir(_) => "opendir",
        TableOp::Rmdir(_) => "rmdir",
        TableOp::IsDir(_) => "dir?",
    }
}

/// Encode a [`TableOp`] as a `table/<op>` call `Expr`.
pub fn encode_table_op(op: &TableOp) -> Expr {
    let args = match op {
        TableOp::Get(key)
        | TableOp::Has(key)
        | TableOp::Delete(key)
        | TableOp::Mkdir(key)
        | TableOp::Opendir(key)
        | TableOp::Rmdir(key)
        | TableOp::IsDir(key) => vec![Expr::Symbol(key.clone())],
        TableOp::Set(key, value) => vec![Expr::Symbol(key.clone()), value.clone()],
        TableOp::CompareExchange(key, expected, replacement) => vec![
            Expr::Symbol(key.clone()),
            encode_expected(expected),
            encode_replacement(replacement),
        ],
        TableOp::Keys | TableOp::Entries | TableOp::Len | TableOp::Clear => Vec::new(),
    };
    Expr::Call {
        operator: Box::new(qsym("table", wire_name(op))),
        args,
    }
}

fn tagged(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        operator: Box::new(qsym("table", name)),
        args,
    }
}

fn encode_expected(expected: &CompareExpected) -> Expr {
    match expected {
        CompareExpected::Absent => tagged("absent", Vec::new()),
        CompareExpected::Value(value) => tagged("value", vec![value.clone()]),
    }
}

fn encode_replacement(replacement: &CompareReplacement) -> Expr {
    match replacement {
        CompareReplacement::Delete => tagged("delete", Vec::new()),
        CompareReplacement::Value(value) => tagged("value", vec![value.clone()]),
    }
}

fn decode_tag(expr: &Expr, expected: bool) -> Result<Option<Expr>, TableOpError> {
    let Expr::Call { operator, args } = expr else {
        return Err(TableOpError::BadArg("cas".into()));
    };
    let Expr::Symbol(symbol) = operator.as_ref() else {
        return Err(TableOpError::BadArg("cas".into()));
    };
    if symbol.namespace.as_deref() != Some("table") {
        return Err(TableOpError::BadArg("cas".into()));
    }
    match (expected, symbol.name.as_ref(), args.as_slice()) {
        (true, "absent", []) | (false, "delete", []) => Ok(None),
        (_, "value", [value]) => Ok(Some(value.clone())),
        _ => Err(TableOpError::BadArg("cas".into())),
    }
}

/// Pull the sole [`Symbol`] argument from `args` for an op named `op`.
fn one_key(op: &str, args: &[Expr]) -> Result<Symbol, TableOpError> {
    match args {
        [Expr::Symbol(key)] => Ok(key.clone()),
        [_] => Err(TableOpError::BadArg(op.to_owned())),
        _ => Err(TableOpError::BadArity(op.to_owned())),
    }
}

/// Pull the sole [`Symbol`] argument from `args` and require that it names one
/// safe, unqualified directory segment.
fn one_dir_segment(op: &str, args: &[Expr]) -> Result<Symbol, TableOpError> {
    let key = one_key(op, args)?;
    if key.namespace.is_none() && is_legal_table_segment(key.name.as_ref()) {
        Ok(key)
    } else {
        Err(TableOpError::BadArg(op.to_owned()))
    }
}

/// Require that `args` is empty for a nullary op named `op`.
fn no_args(op: &str, args: &[Expr]) -> Result<(), TableOpError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(TableOpError::BadArity(op.to_owned()))
    }
}

/// Decode a `table/<op>` call `Expr` back into a [`TableOp`].
pub fn decode_table_op(expr: &Expr) -> Result<TableOp, TableOpError> {
    let Expr::Call { operator, args } = expr else {
        return Err(TableOpError::NotATableCall);
    };
    let Expr::Symbol(symbol) = operator.as_ref() else {
        return Err(TableOpError::NotATableCall);
    };
    if symbol.namespace.as_deref() != Some("table") {
        return Err(TableOpError::NotATableCall);
    }
    let name = symbol.name.as_ref();
    let op = match name {
        "get" => TableOp::Get(one_key(name, args)?),
        "set" => match args.as_slice() {
            [Expr::Symbol(key), value] => TableOp::Set(key.clone(), value.clone()),
            [_, _] => return Err(TableOpError::BadArg(name.to_owned())),
            _ => return Err(TableOpError::BadArity(name.to_owned())),
        },
        "cas" => match args.as_slice() {
            [Expr::Symbol(key), expected, replacement] => {
                let expected = match decode_tag(expected, true)? {
                    None => CompareExpected::Absent,
                    Some(value) => CompareExpected::Value(value),
                };
                let replacement = match decode_tag(replacement, false)? {
                    None => CompareReplacement::Delete,
                    Some(value) => CompareReplacement::Value(value),
                };
                TableOp::CompareExchange(key.clone(), expected, replacement)
            }
            [_, _, _] => return Err(TableOpError::BadArg(name.to_owned())),
            _ => return Err(TableOpError::BadArity(name.to_owned())),
        },
        "has" => TableOp::Has(one_key(name, args)?),
        "del" => TableOp::Delete(one_key(name, args)?),
        "keys" => {
            no_args(name, args)?;
            TableOp::Keys
        }
        "entries" => {
            no_args(name, args)?;
            TableOp::Entries
        }
        "len" => {
            no_args(name, args)?;
            TableOp::Len
        }
        "clear" => {
            no_args(name, args)?;
            TableOp::Clear
        }
        "mkdir" => TableOp::Mkdir(one_dir_segment(name, args)?),
        "opendir" => TableOp::Opendir(one_dir_segment(name, args)?),
        "rmdir" => TableOp::Rmdir(one_dir_segment(name, args)?),
        "dir?" => TableOp::IsDir(one_dir_segment(name, args)?),
        other => return Err(TableOpError::UnknownOp(other.to_owned())),
    };
    Ok(op)
}
