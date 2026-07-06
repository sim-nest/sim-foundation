//! The narrow backend-manifest constructor shared by host-registered table
//! backends.
//!
//! Table backend crates (`sim-table-hash`, `sim-table-lazy`, ...) each declared
//! a byte-identical empty [`LibManifest`]: no exports, no requirements, no
//! capabilities, [`HostRegistered`](LibTarget::HostRegistered), ABI `{0, 1}`.
//! Backend registration itself (the has-lib guard and `register` call) stays in
//! each crate because the registry differs; only the manifest shape is shared
//! here.

use sim_kernel::{AbiVersion, Dependency, Export, LibManifest, LibTarget, Symbol, Version};

/// Build the empty, host-registered [`LibManifest`] a table backend presents.
///
/// The `id` names the library (e.g. `Symbol::qualified("table", "hash")`) and
/// `version` is the crate version literal (typically `env!("CARGO_PKG_VERSION")`).
/// The manifest declares no exports, requirements, or capabilities, targets
/// [`LibTarget::HostRegistered`], and pins ABI `{ major: 0, minor: 1 }`.
pub fn backend_manifest(id: Symbol, version: &'static str) -> LibManifest {
    LibManifest {
        id,
        version: Version(version.to_owned()),
        abi: AbiVersion { major: 0, minor: 1 },
        target: LibTarget::HostRegistered,
        requires: Vec::<Dependency>::new(),
        capabilities: Vec::new(),
        exports: Vec::<Export>::new(),
    }
}
