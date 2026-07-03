//! Codec-neutral surface-descriptor spine.

use sim_kernel::{Symbol, Value};

use crate::name::{ExternalNamePolicy, external_name};

/// Plain-data spine describing a surface (a callable projected onto a foreign
/// tool surface). Depends only on kernel types; concrete codecs render it.
#[derive(Debug, Clone)]
pub struct SurfaceDescriptor {
    /// Stable surface id (for example a skill id).
    pub id: String,
    /// Kernel symbol that names the underlying callable.
    pub symbol: Symbol,
    /// Human-facing title.
    pub title: String,
    /// Human-facing description.
    pub description: String,
    /// Shape describing accepted input.
    pub input_shape: Value,
    /// Shape describing produced output, when known.
    pub output_shape: Option<Value>,
    /// Roles this surface is published under.
    pub roles: Vec<Symbol>,
    /// Capabilities the surface requires.
    pub capabilities: Vec<Symbol>,
    /// Free-form `(key, value)` annotations.
    pub annotations: Vec<(String, String)>,
}

impl SurfaceDescriptor {
    /// Project this surface's symbol onto an external name under `policy`.
    pub fn external_name(&self, policy: ExternalNamePolicy) -> String {
        external_name(&self.symbol, policy)
    }
}
