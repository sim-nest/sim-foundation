# sim-macros

`sim-macros` turns an inline Rust module into a SIM library declaration. It
reads marker attributes such as `#[sim_class]`, `#[sim_constructor]`,
`#[sim_fn]`, `#[sim_macro]`, `#[sim_codec]`, `#[sim_number_domain]`, and
`#[sim_site]`, validates their contracts, and generates the `Lib` manifest and
registration code that the runtime expects.

Most consumers pull this surface in through the SIM facade:

```toml
[dependencies]
sim = { package = "sim-nest", version = "0.1", default-features = false, features = ["core", "shape", "proc-macros"] }
```

## Example

```rust
use sim::{case, shape, sim_class, sim_constructor, sim_fn, sim_lib};

#[sim_lib(id = "geometry", version = "0.1.0")]
mod geometry {
    use super::{case, shape, sim_class, sim_constructor, sim_fn};

    #[sim_class(name = "Point")]
    #[shape("(fields (:x Number) (:y Number))")]
    pub struct Point {
        x: f64,
        y: f64,
    }

    #[sim_constructor(class = "Point")]
    #[case(args = "((capture x Number) (capture y Number))", result = "Point")]
    pub fn point(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[sim_fn(name = "distance")]
    #[case(args = "((capture left Point) (capture right Point))", result = "Number")]
    pub fn distance(left: &Point, right: &Point) -> f64 {
        let dx = left.x - right.x;
        let dy = left.y - right.y;
        (dx * dx + dy * dy).sqrt()
    }
}
```

## What `#[sim_lib]` reads

- `#[sim_class]` for generated class exports.
- `#[sim_constructor]` plus one or more `#[case(...)]` declarations.
- `#[sim_fn]` plus one or more `#[case(...)]` declarations.
- `#[sim_macro]`, `#[sim_codec]`, `#[sim_number_domain]`, and `#[sim_site]`
  marker declarations.
- Optional `#[shape("...")]` literals on classes and functions.
- The `id`, `version`, and optional `native_export` entries on `#[sim_lib(...)]`.

`#[sim_lib]` requires an inline module. The marker attributes are inert on their
own; they are consumed when the enclosing module is expanded.

## What it generates

- A `<ModuleName>Lib` type that implements `::sim::kernel::Lib`.
- Manifest exports for classes, functions, macros, codecs, number domains, and
  sites declared in the module.
- Generated class wrappers, constructor wiring, and function adapters.
- Load-time registration for generated classes, functions, and macros.
- A `__SIM_LIB_EXPANSION` string snapshot for inspection in tests.
- Optional native ABI glue when `native_export = true`.

## What fails at compile time

- Missing or duplicate `id`, `version`, or marker entries.
- Malformed shape literals.
- A `#[sim_class]` without a matching `#[sim_constructor]`.
- Unsupported borrowed builtin arguments.
- `native_export = true` signatures that require generated class values where
  the ABI only supports scalar, `String`, `Symbol`, or `Expr` traffic.
- Non-inline `#[sim_lib]` modules.

These failures are deliberate: the crate rejects invalid library declarations at
build time instead of widening the runtime contract.

## Examples In Tree

- `crates/sim-macros/tests/ui/pass/basic-lib`
- `crates/sim-macros/tests/ui/pass/marker-surface`
- `crates/sim-macros/tests/ui/smoke/consumer`

