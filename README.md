# sim-foundation

Ergonomic, dependency-light building blocks for working with SIM's kernel data:
build and read `Expr` values, merge table-shaped configuration, validate table
paths, author libraries with proc-macros, shape codec-neutral surfaces, and
check SIM Index graph records -- without re-growing the same substrate in every
library.

SIM is a small Rust protocol kernel plus a large set of loadable libraries (it
is not a Lisp runtime); the kernel defines contracts and libraries provide
behavior. The `sim` command-line binary is a separate install
(`cargo install sim-run`), and the full walkthrough lives in the `sim-say`
front page. This repo is a set of **libraries** -- the shared groundwork many
other SIM libraries build on.

## Quick start

sim-foundation is a set of libraries you add to a Rust project. The primary
crate is `sim-value`, which gives you ergonomic construction and access for the
kernel `Expr` graph:

```bash
cargo add sim-value
```

```rust
use sim_value::access::{field, set};
use sim_value::build::{int, map};

let value = map(vec![("a", int(1)), ("b", int(2))]);
assert_eq!(field(&value, "a"), Some(&int(1)));

// immutable update: siblings are preserved
let updated = set(&value, "a", int(9));
assert_eq!(field(&updated, "a"), Some(&int(9)));
assert_eq!(field(&updated, "b"), Some(&int(2)));
```

(Adapted from the passing doctest in `crates/sim-value/src/lib.rs:16`.)

## How it works

This repo holds the foundation layer -- the dependency-light substrate crates
that many other libraries build on instead of re-growing the same code.

It provides ergonomic construction and access for the kernel `Expr` graph, the
shared configuration table/Dir substrate, the shared table path and operation
protocol, the proc-macro surface for authoring libraries, the crate-local
cookbook engine, reusable HTTP/streaming parsing primitives, and the
codec-neutral surface-card spine, and the checked model for the SIM Index graph.
These crates depend only on `sim-kernel` (and, where noted, on `sim-value`);
they add data ergonomics and protocol shape, not runtime behavior, so they stay
below the kernel boundary.

## Crates

- `sim-value` -- ergonomic construction and access for kernel `Expr` data:
  `build` constructors (`sym`, `int`, `float`, `text`, `list`, `map`), the
  `access` readers and immutable updates (`field`, `set`, `remove`), the one
  `expr_kind` variant classifier, and a `Path` value-addressing primitive.
  Depends only on `sim-kernel`.
- `sim-table-core` -- the shared table substrate: legal-segment path validation
  (`is_legal_table_segment` and the validating `TablePath` accumulator),
  escaped absolute and relative `TablePathRef` resolution, and the `TableOp`
  protocol, whose `encode_table_op`/`decode_table_op` round-trip through the
  kernel `Expr` graph in the wire spellings table backends speak.
  Depends only on `sim-kernel` and `sim-value`.
- `sim-config` -- the shared configuration substrate: `ConfigTable` and
  `ConfigDir` over kernel `Expr::Map`, layered merge/provenance rules,
  `ConfigView` typed accessors, and safe per-library config paths. Depends only
  on `sim-kernel`, `sim-value`, and `sim-table-core`.
- `sim-macros` -- the proc-macro surface for authored SIM libraries: the
  `#[sim_class]`, `#[sim_constructor]`, `#[sim_fn]`, `#[case]`, and `#[shape]`
  markers, plus `#[sim_lib(...)]`, which scans an inline module for those
  markers, validates the contracts, and generates runtime registration and
  optional native ABI glue, rejecting invalid shapes during expansion.
- `sim-cookbook` -- the kernel-free cookbook engine for crate-local tutorial
  recipes: manifest parsing and lint, compile-time embedding, recipe stores,
  projection/search/next behavior, and deterministic user overlays. Recipes
  register as Card records that every surface projects.
- `sim-index-core` -- the SIM Index graph model: subjects, anchors, surfaces,
  features, runnable specimens, grammar contracts, routes, canonical feature
  keys, graph checks, the canonical `IndexRowRef` inventory, and kernel Card
  projections. New projections iterate `IndexDoc::inventory()` instead of
  copying its fields. Depends only on `sim-kernel`.
- `sim-index-vault-core` -- the syntax-free vault projection. It assigns each
  canonical row one primary note claim and closes an exact `ClaimCertificate`;
  renderers and filesystem exporters live downstream.
- `sim-lib-net-core` -- reusable, side-effect-free HTTP/streaming parsing
  primitives: URL parsing, HTTP response-head parsing, body-mode classification,
  line framing, and SSE/NDJSON record decoders, with no socket/TLS I/O and no
  application policy.
- `sim-lib-surface-card` -- the codec-neutral surface-card spine: the
  `external_name`/`ExternalNamePolicy` rules for projecting a kernel `Symbol`
  onto foreign tool surfaces (MCP, OpenAI, file system, human-facing text) and a
  plain-data `SurfaceDescriptor` over kernel types.
- `sim-lib-protected-state` -- bounded, versioned XChaCha20-Poly1305 envelopes
  for caller-serialized opaque state, with exact purpose/audience/subject/context/expiry
  binding, injected retained keys, cryptographic nonces and wall time, and an optional
  atomic single-use adapter over canonical Table compare-exchange.

## Boundary

These crates are foundation substrate, not runtime behavior. Each is a leaf or
near-leaf in the dependency graph, depending only on `sim-kernel` and
`sim-value`. They keep common code -- value ergonomics, configuration merging,
table path-reference handling, library-authoring macros, cookbook projection, wire
framing, surface naming, and checked index facts -- in one tested home. Concrete
runtime operations layer over these crates elsewhere in the constellation; the
foundation layer adds data ergonomics and protocol shape and does not touch the
kernel boundary.

## Validation

This repo validates from a single clone against the SIM crates published on
crates.io. CI installs the channel named by `rust-toolchain.toml` instead of a
floating stable toolchain. The generated-doc check delegates to the shared
`sim-tooling` encoder; CI checks out `sim-nest/sim-tooling` and sets
`SIMDOC_TOOLING_MANIFEST`, while local runs can use either a sibling
`sim-tooling` checkout or the same environment variable.

```bash
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- check-package-floors
cargo run -p xtask -- simdoc --check
cargo run -p xtask -- check-file-sizes
```

## Documentation Lanes

`cargo run -p xtask -- simdoc` builds the public documentation lanes:

- API docs: `target/doc/`
- Agent cards: `docs/agents/cards.jsonl` and `docs/agents/card-index.json`
- Human docs: `docs/humans/`
- Diagrams: `docs/diagrams/src/` and `docs/diagrams/generated/`

The same command writes split contract files under `docs/generated/`. Everything
under `docs/` is generated; do not hand-edit it.

### Rustdoc conventions

Public API documentation in `src/` follows one house style:

- Every public item opens with a one-line summary sentence, then context.
- A type that builds on a kernel contract states which one: the kernel defines
  the contract; sim-foundation provides the value, table, macro, cookbook, net,
  and surface-card layer built on it.
- The first-reach types carry a `# Examples` doctest that compiles and passes.
- Cross-reference with intra-doc links, and link back to this README rather than
  restating it.

The public API is documentation-gated: each crate's `lib.rs` denies
`missing_docs`, so every public item, field, and macro must be documented for the
crate to build.

### Examples and recipes

`sim-lib-net-core` and `sim-lib-surface-card` ship descriptor cookbook entries
under their `recipes/` directories. They are projection material for surfaces to
show, not sandbox-executed examples. `sim-cookbook` is the cookbook engine itself
(manifest parsing, embedding, recipe stores, and projection), so it hosts no
recipes of its own. The remaining crates teach their substrate contracts through
rustdoc doctests and unit tests instead of empty recipe directories.

`cargo run -p xtask -- check-recipes` enforces that contract. Publishable
packages declare their recipe policy in `[package.metadata.sim-recipes]`;
descriptor recipe manifests must carry the `sandbox-descriptor` tag, and
rustdoc-only or engine crates must document why they have no `recipes/`
directory.
