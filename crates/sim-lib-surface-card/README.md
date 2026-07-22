# sim-lib-surface-card

`sim-lib-surface-card` is the codec-neutral spine for presenting SIM tools to
outside systems and people. It translates a kernel `Symbol` into destination-safe
external names and carries a small `SurfaceDescriptor` record over kernel data.

The crate does not render a UI, choose a transport, or define a tool protocol.
Concrete surfaces use it as their shared naming and descriptor substrate.

## What it provides

- `ExternalNamePolicy` for OpenAI tool names, MCP names, file-system names, and
  human-readable names.
- `external_name` for stable projection from a kernel symbol to the selected
  external naming policy.
- `SurfaceDescriptor` for a plain-data description of a surface-facing item.
- Build-time cookbook recipe embedding for the external-name walkthrough.

## Example

```rust
use sim_kernel::Symbol;
use sim_lib_surface_card::{ExternalNamePolicy, external_name};

let symbol = Symbol::qualified("skill", "do.thing");
assert_eq!(
    external_name(&symbol, ExternalNamePolicy::OpenAiTool),
    "skill_do_thing"
);
```

## Recipes, Docs, And Validation

The `recipes/` directory includes an external-name walkthrough. Rustdoc examples
cover the naming policies and descriptor behavior.

- API docs: <https://docs.rs/sim-lib-surface-card>
- Repository guide: <https://github.com/sim-nest/sim-foundation>

From the `sim-foundation` checkout:

```bash
cargo test -p sim-lib-surface-card
RUSTDOCFLAGS="-D warnings" cargo doc -p sim-lib-surface-card --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- check-package-floors
cargo run -p xtask -- simdoc --check
```
