# sim-config

`sim-config` is the Table and Dir substrate for layered SIM configuration. It
keeps configuration as kernel `Expr::Map` data, merges layers with provenance,
binds shape-checked defaults, offers typed read views, and builds safe
per-library config paths.

It does not introduce a private config value model. Decoders, bootloaders,
probes, and explicit overrides all produce the same `ConfigDir` shape, then
callers merge those Dirs from built-in defaults through the highest-precedence
process override.

## What it provides

- `ConfigTable` for one library's map-valued table.
- `ConfigDir` for a library-id-to-table Dir.
- `ConfigLayer`, `EffectiveConfig`, `MergeTrace`, and `merge_layers`.
- Merge rules for scalar replacement, nested table overlays, id-keyed repeated
  table replacement, and absent-field preservation.
- `ConfigShape` and `LibConfigDefaults` for shape-backed defaults, unknown-field
  reports, and secret-field metadata.
- `ConfigView` typed accessors over table fields.
- `ConfigRoots` and `lib_config_path` for root selection and safe
  `libs/<lib-id>.toml` paths.

## Example

```rust
use sim_config::{ConfigDir, ConfigLayer, ConfigSource, merge_layers};
use sim_kernel::Symbol;
use sim_value::build::{map, text};

let lib = Symbol::qualified("model", "defaults");
let built_in = ConfigDir::one(lib.clone(), map(vec![("provider", text("modeled"))]))?;
let work = ConfigDir::one(lib.clone(), map(vec![("provider", text("openai"))]))?;

let effective = merge_layers(&[
    ConfigLayer::new(ConfigSource::BuiltIn { lib: lib.clone() }, built_in),
    ConfigLayer::new(ConfigSource::Explicit { label: "cli".into() }, work),
]);

assert!(effective.dir.table(&lib).is_some());
# Ok::<(), Box<dyn std::error::Error>>(())
```
