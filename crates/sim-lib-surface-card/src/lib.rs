//! Shared, plain-data surface-card spine.
//!
//! This crate holds the codec-neutral pieces of a "surface card": the
//! external-name policy used when projecting a kernel [`Symbol`](sim_kernel::Symbol)
//! onto a foreign tool surface (MCP, OpenAI, file system, human-facing text) and
//! a plain-data [`SurfaceDescriptor`] spine that depends only on kernel types.
//!
//! Concrete surfaces (for example `sim-lib-skill`'s OpenAI tool descriptor)
//! derive their external names through [`external_name`], keeping a single
//! source of truth for the mangling rules.
//!
//! # Example
//!
//! ```
//! use sim_kernel::Symbol;
//! use sim_lib_surface_card::{ExternalNamePolicy, external_name};
//!
//! let symbol = Symbol::qualified("skill", "do.thing");
//! assert_eq!(external_name(&symbol, ExternalNamePolicy::OpenAiTool), "skill_do_thing");
//! assert_eq!(external_name(&symbol, ExternalNamePolicy::HumanReadable), "skill/do.thing");
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod descriptor;
mod name;

pub use descriptor::SurfaceDescriptor;
pub use name::{ExternalNamePolicy, external_name};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));
