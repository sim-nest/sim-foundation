# sim-table-core

`sim-table-core` is the shared table substrate for SIM libraries. It defines the
legal table path segment predicate, a validating path accumulator, canonical
host-effect capability names and aliases, and the table operation model used by
local and remote table backends.

The crate owns protocol shape, not storage behavior. It performs no file-system,
database, network, or process effects; concrete backends implement those effects
above this layer.

## What it provides

- `is_legal_table_segment` and `TablePath` for safe table locations.
- `TableOp` plus `encode_table_op` and `decode_table_op` for the wire shape of
  read, write, list, delete, and directory operations.
- Canonical capability helpers for file, find, edit, exec, and HTTP access.
- A small backend manifest helper for libraries that expose table backends.

## Example

```rust
use sim_table_core::is_legal_table_segment;

assert!(is_legal_table_segment("nodes"));
assert!(!is_legal_table_segment(""));
assert!(!is_legal_table_segment(".."));
assert!(!is_legal_table_segment("a/b"));
```

## Examples, Docs, And Validation

This crate is a protocol substrate, so examples live in rustdoc and unit tests.
It has no recipe directory because backends that execute table operations own
the cookbook entries for those effects.

- API docs: <https://docs.rs/sim-table-core>
- Repository guide: <https://github.com/sim-nest/sim-foundation>

From the `sim-foundation` checkout:

```bash
cargo test -p sim-table-core
RUSTDOCFLAGS="-D warnings" cargo doc -p sim-table-core --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- simdoc --check
```
