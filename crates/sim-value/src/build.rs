//! Constructors for kernel `Expr` data.
//!
//! These are the one home for "make a symbol / number / string / map".
//! Float canonicalization matches Rust's `f64` `Display` (which already drops a
//! trailing `.0`), so values built here are byte-identical to a direct
//! `number(f64)` construction.

use sim_kernel::{Expr, NumberLiteral, Symbol};

/// An unqualified symbol value: `Expr::Symbol(Symbol::new(name))`.
pub fn sym(name: &str) -> Expr {
    Expr::Symbol(Symbol::new(name))
}

/// A qualified symbol value: `Expr::Symbol(Symbol::qualified(ns, name))`.
pub fn qsym(ns: &str, name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(ns, name))
}

/// A bare keyword `Symbol` (NOT wrapped in `Expr`). This is the one home for
/// common `fn field(name) -> Symbol` / `Symbol::new(name)` wrappers in test and
/// codec crates; use [`sym`] when an `Expr` is wanted.
pub fn keyword(name: &str) -> Symbol {
    Symbol::new(name)
}

/// An `i64`-domain number value.
pub fn int(value: i64) -> Expr {
    num("i64", &value.to_string())
}

/// A `u64`-domain number value from an unsigned integer.
pub fn uint(value: u64) -> Expr {
    num("u64", &value.to_string())
}

/// An `f64`-domain number value with a canonical literal.
pub fn float(value: f64) -> Expr {
    num("f64", &format!("{value}"))
}

/// A number value in `domain` with the given `canonical` literal.
pub fn num(domain: &str, canonical: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new(domain),
        canonical: canonical.to_owned(),
    })
}

/// A number value whose domain may be namespace-qualified. `num_q(None, name,
/// canonical)` is identical to [`num`]; `num_q(Some(ns), name, canonical)`
/// emits a qualified domain symbol (for example `numbers/f64`). Use this when a
/// caller must preserve an existing qualified number domain rather than the
/// unqualified spelling produced by [`float`] or [`num`].
pub fn num_q(ns: Option<&str>, name: &str, canonical: &str) -> Expr {
    let domain = match ns {
        Some(ns) => Symbol::qualified(ns, name),
        None => Symbol::new(name),
    };
    Expr::Number(NumberLiteral {
        domain,
        canonical: canonical.to_owned(),
    })
}

/// A string value.
pub fn text(value: impl Into<String>) -> Expr {
    Expr::String(value.into())
}

/// A list value.
pub fn list(items: Vec<Expr>) -> Expr {
    Expr::List(items)
}

/// A vector value.
pub fn vector(items: Vec<Expr>) -> Expr {
    Expr::Vector(items)
}

/// A single map entry with a bare-symbol key: `(sym(name), value)`. This is the
/// one home for the `fn entry(name, value)` tuple builder used by domain crates
/// (topology, agent, reflection).
pub fn entry(name: &str, value: Expr) -> (Expr, Expr) {
    (sym(name), value)
}

/// A map value from string-keyed entries (keys become unqualified symbols).
pub fn map(entries: Vec<(&str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (sym(key), value))
            .collect(),
    )
}
