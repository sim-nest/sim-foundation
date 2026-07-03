# sim-foundation

sim-foundation is a repository in the SIM constellation. SIM is an expandable
Rust runtime built around a small protocol kernel plus a large set of loadable
libraries: the kernel defines contracts, libraries provide behavior. This repo
holds the foundation layer -- the dependency-light substrate crates that many
other libraries build on instead of re-growing the same code.

It provides ergonomic construction and access for the kernel `Expr` graph, the
shared table path and operation protocol, the proc-macro surface for authoring
libraries, the crate-local cookbook engine, reusable HTTP/streaming parsing
primitives, and the codec-neutral surface-card spine. These crates depend only
on `sim-kernel` (and, where noted, on `sim-value`); they add data ergonomics and
protocol shape, not runtime behavior, so they stay below the kernel boundary.

## Crates

- `sim-value` -- ergonomic construction and access for kernel `Expr` data:
  `build` constructors (`sym`, `int`, `float`, `text`, `list`, `map`), the
  `access` readers and immutable updates (`field`, `set`, `remove`), the one
  `expr_kind` variant classifier, and a `Path` value-addressing primitive.
  Depends only on `sim-kernel`.
- `sim-table-core` -- the shared table substrate: legal-segment path validation
  (`is_legal_table_segment` and the validating `TablePath` accumulator) and the
  `TableOp` protocol, whose `encode_table_op`/`decode_table_op` round-trip
  through the kernel `Expr` graph in the wire spellings table backends speak.
  Depends only on `sim-kernel` and `sim-value`.
- `sim-macros` -- the proc-macro surface for authored SIM libraries: the
  `#[sim_class]`, `#[sim_constructor]`, `#[sim_fn]`, `#[case]`, and `#[shape]`
  markers, plus `#[sim_lib(...)]`, which scans an inline module for those
  markers, validates the contracts, and generates runtime registration and
  optional native ABI glue, rejecting invalid shapes during expansion.
- `sim-cookbook` -- the kernel-free cookbook engine for crate-local tutorial
  recipes: manifest parsing and lint, compile-time embedding, recipe stores,
  projection/search/next behavior, and deterministic user overlays. Recipes
  register as Card records that every surface projects.
- `sim-lib-net-core` -- reusable, side-effect-free HTTP/streaming parsing
  primitives: URL parsing, HTTP response-head parsing, body-mode classification,
  line framing, and SSE/NDJSON record decoders, with no socket/TLS I/O and no
  application policy.
- `sim-lib-surface-card` -- the codec-neutral surface-card spine: the
  `external_name`/`ExternalNamePolicy` rules for projecting a kernel `Symbol`
  onto foreign tool surfaces (MCP, OpenAI, file system, human-facing text) and a
  plain-data `SurfaceDescriptor` over kernel types.

## Boundary

These crates are foundation substrate, not runtime behavior. Each is a leaf or
near-leaf in the dependency graph, depending only on `sim-kernel` and
`sim-value`. They exist because the same code -- value ergonomics, table path
validation, library-authoring macros, cookbook projection, wire framing, and
surface naming -- was independently re-grown across libs, and this repo is the
single tested home for it. Concrete runtime operations layer over these crates
elsewhere in the constellation; the foundation layer adds data ergonomics and
protocol shape and does not touch the kernel boundary.

## Validation

These commands run in the constellation workspace; only `sim-kernel` builds from a lone clone today (see `DEVELOPING.md` in `sim-sdk`). A single-repo build lands with the first crates.io publish.

```bash
cargo fmt --check && cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo doc --workspace --no-deps
cargo run -p xtask -- simdoc --check
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

`sim-lib-net-core` and `sim-lib-surface-card` ship runnable recipes under their
`recipes/` directories. `sim-cookbook` is the cookbook engine itself (manifest
parsing, embedding, recipe stores, and projection), so it hosts no recipes of its
own. The remaining crates' examples are their rustdoc doctests; no stub recipe
directories are added.
