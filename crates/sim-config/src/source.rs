//! Configuration source descriptors.

use std::path::PathBuf;

use sim_kernel::Symbol;

/// Source layer that contributed configuration data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigSource {
    /// Built-in defaults supplied by the named library.
    BuiltIn {
        /// Library whose defaults produced this layer.
        lib: Symbol,
    },
    /// Defaults produced by a modeled or real probe.
    Probe {
        /// Probe implementation id.
        probe: Symbol,
        /// Probe mode for producing the layer.
        mode: ProbeMode,
    },
    /// Per-library file from the central home config root.
    HomeFile {
        /// File path that produced this layer.
        path: PathBuf,
    },
    /// Per-library file from the working config root.
    WorkFile {
        /// File path that produced this layer.
        path: PathBuf,
    },
    /// Single `sim.toml` file containing multiple library tables.
    SingleFile {
        /// File path that produced this layer.
        path: PathBuf,
    },
    /// Opaque site-backed config producer.
    Site {
        /// Site id.
        site: Symbol,
    },
    /// Explicit process-level override.
    Explicit {
        /// Human-readable override label.
        label: String,
    },
}

/// Probe mode for config defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProbeMode {
    /// Deterministic, modeled inventory; performs no real I/O.
    #[default]
    Modeled,
    /// Capability-gated real inventory.
    Real,
}
