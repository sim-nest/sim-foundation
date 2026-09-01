//! Pure, effect-free web evidence records.
//!
//! Capturing, decoding, and policy execution live elsewhere. This crate only
//! names immutable exchange facts and proves selectors against normalized text.

#![forbid(unsafe_code)]

mod policy;
mod records;
mod selector;
mod wire;

pub use policy::*;
pub use records::*;
pub use selector::*;
pub use wire::{RECORD_DESCRIPTORS, RecordDescriptor};

/// Network-free cookbook descriptors embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
