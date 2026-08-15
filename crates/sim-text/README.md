# sim-text

`sim-text` is the neutral home for exact text values whose representation is
not restricted to Unicode scalar strings. It will own the shared UTF-16
code-unit value used by JavaScript, codecs, and the future JVM implementation.

This first checkpoint deliberately contains no behavior. The existing
`JavascriptCodeUnitString` implementation and its characterized fixtures remain
the only implementation until they are moved here. `sim-value` remains focused
on construction and access ergonomics for kernel `Expr` values.

## Reuse and dependency ledger

| Need | Existing anchor | Disposition |
|---|---|---|
| Exact UTF-16 storage, indexing, slicing, paired iteration, checked scalar conversion | `sim-runtime/crates/sim-lib-lang-javascript/src/text.rs` and its `law_fixtures` | Reuse by move; JavaScript retains a compatibility adapter. |
| Canonical scalar text and bytes | `sim-kernel` `Expr::String` and `Expr::Bytes` | Reuse as projections; do not extend the kernel with foreign-text policy. |
| Expression construction and text/path editing | `sim-foundation/crates/sim-value/src/lib.rs`, `edit.rs`, and `path.rs` | Keep separate; these are `Expr` ergonomics, not exact foreign-text storage. |
| Expression codecs and read construction | installed `sim-codecs` expression codecs and the standard read-construct protocol | Reuse existing protocols; exact text does not create a parallel codec family. |
| UTF conversion | Rust `str::encode_utf16` and `String::from_utf16` used by the JavaScript anchor | Reuse, preserving the typed rejection of lone surrogates. |

The complete planned direct consumer set is: the codec implementation(s) in
the `sim-codecs` workspace, the JavaScript adapter in the `sim-runtime`
workspace, and the future JVM crate. None is a dependency of `sim-foundation`:
the new crate has no dependencies, and the `sim-foundation` workspace manifest
contains no dependency on `sim-codecs`, `sim-runtime`, or a JVM crate. The edge
therefore points only from each higher consumer to `sim-text`; there is no back
edge or dependency cycle.

## Documentation and validation

- API docs: <https://docs.rs/sim-text>
- Repository guide: <https://github.com/sim-nest/sim-foundation>

From the `sim-foundation` checkout:

The crate has no recipe directory while it is behavior-free; crate-level
rustdoc is the documentation lane for this scaffolding checkpoint.

```bash
cargo test -p sim-text
RUSTDOCFLAGS="-D warnings" cargo doc -p sim-text --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- simdoc --check
```
