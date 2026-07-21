# sim-index-core

`sim-index-core` is the model crate for the SIM Index graph. It defines the
plain Rust records for subjects, anchors, surfaces, runnable specimens, features,
routes, grammar contracts, canonical feature keys, and graph edges. Codecs and
generators consume this crate instead of inventing a parallel index shape.

The crate is deliberately small: it stores data, derives canonical feature keys,
checks graph consistency, and projects feature, specimen, and route rows into
ordinary kernel Card records. It does not parse the index wire format, discover
repo facts, render docs, or run examples; those jobs belong to later layers that
feed or consume the checked graph.

## Contract

The checker rejects malformed or unsafe index material before any generated view
uses it:

- non-ASCII text;
- malformed ids and canonical keys;
- duplicate ids and duplicate canonical feature keys;
- authored literal facts instead of discovered ids;
- unresolved anchor, surface, specimen, edge, and doc-anchor references;
- grammar contracts without a closed surface and codec anchor;
- claimed specimens that are not runnable and checked;
- routes with missing feature or specimen steps.

Feature, specimen, and route cards are open Card records. They carry stable
predicate entries such as `kind`, `canonical-key`, `subject`, `specimens`, and
`steps`, so browse, docs, and runtime exploration can project the same checked
graph without adding kernel types.

## Validation

Rustdoc examples and unit tests cover this model crate. It has no recipe
directory because it carries checked records rather than runnable user lessons.

This crate participates in the sim-foundation workspace gates:

```bash
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- check-package-floors
cargo run -p xtask -- check-file-sizes
cargo run -p xtask -- simdoc --check
```
