# sim-value

`sim-value` is the ergonomic construction and access layer for the kernel `Expr`
graph. It gives SIM libraries one shared way to build values, read fields, update
maps immutably, classify variants, parse capability-name expressions, and address
nested data by path.

The crate does not extend the kernel data model and does not add runtime policy.
It depends only on `sim-kernel`, so higher libraries can use the same helpers
without pulling in concrete behavior.

## What it provides

- Constructors for symbols, qualified symbols, numbers, strings, lists, vectors,
  and maps.
- Field readers for bare-symbol keys and mixed symbol/string keys.
- Required-field helpers with consistent diagnostics.
- Immutable update helpers for maps and nested paths.
- The shared `expr_kind` classifier and capability-name parser.
- Side-effect-free text edit helpers for exact and line-range edits.

## Example

```rust
use sim_value::access::{field, set};
use sim_value::build::{int, map};

let value = map(vec![("a", int(1)), ("b", int(2))]);
let updated = set(&value, "a", int(9));

assert_eq!(field(&updated, "a"), Some(&int(9)));
assert_eq!(field(&updated, "b"), Some(&int(2)));
```

## Examples, Docs, And Validation

Rustdoc examples cover the construction, access, update, and path helpers. The
crate has no recipe directory because it stays at the value-substrate layer.

- API docs: <https://docs.rs/sim-value>
- Repository guide: <https://github.com/sim-nest/sim-foundation>

From the `sim-foundation` checkout:

```bash
cargo test -p sim-value
RUSTDOCFLAGS="-D warnings" cargo doc -p sim-value --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- check-package-floors
cargo run -p xtask -- simdoc --check
```
