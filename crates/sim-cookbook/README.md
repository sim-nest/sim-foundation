# sim-cookbook

`sim-cookbook` is the kernel-free engine for crate-local SIM lessons. It parses
recipe manifests, lints recipe directories, embeds recipe files at build time,
stores recipe cards, projects them into books and chapters, supports search and
next-lesson selection, and applies deterministic user overlays.

The crate does not execute recipes or own a user interface. Runtime commands,
help, browse, web, and assistant surfaces consume the projected recipe data from
libraries that load this engine.

## What it provides

- Manifest parsers for `book.toml`, `chapter.toml`, and `recipe.toml`.
- Filesystem linting for safe recipe paths and complete lesson directories.
- Compile-time embedding helpers for crates that ship `recipes/`.
- `RecipeStore` plus projections for books, chapters, grouped views, search, and
  next-lesson traversal.
- Stable digest helpers for recipe result checks and small binary/audio
  comparisons.
- Overlay parsing so user notes and local ordering rules compose with shipped
  lessons.

## Example

```rust
use sim_cookbook::{RecipeStore, view};

let store = RecipeStore::new();
let cookbook = view(&store);
assert!(cookbook.books.is_empty());
```

## Examples, Docs, And Validation

This crate is the recipe engine itself, so it carries rustdoc examples rather
than its own tutorial recipe directory. Crates that teach concrete behavior place
their lessons in their own `recipes/` directories and use this crate to embed and
project them.

- API docs: <https://docs.rs/sim-cookbook>
- Repository guide: <https://github.com/sim-nest/sim-foundation>

From the `sim-foundation` checkout:

```bash
cargo test -p sim-cookbook
RUSTDOCFLAGS="-D warnings" cargo doc -p sim-cookbook --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- check-package-floors
cargo run -p xtask -- simdoc --check
```
