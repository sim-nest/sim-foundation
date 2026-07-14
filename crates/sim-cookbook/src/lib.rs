//! Cookbook engine for SIM: runnable, crate-local tutorial recipes.
//!
//! A *recipe* is a tiny lesson -- a runnable setup (in Lisp or any registered
//! codec) plus a purpose document -- that ships inside the crate it teaches.
//! When a lib loads, it registers its recipes as Card records; the *cookbook*
//! is then a projection over those cards, grouped into books and chapters and
//! rendered by every surface (CLI, WebUI, browse, agent).
//!
//! This crate owns the kernel-free cookbook engine: manifest parsing and lint,
//! compile-time embedding, recipe stores, projection/search/next behavior, and
//! deterministic user overlays. Runtime operations live in `sim-lib-cookbook`;
//! the CLI, WebUI, browse/help, and agent surfaces all use that shared runtime
//! projection instead of parallel cookbook logic.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod config;
mod digest;
mod embed;
mod embed_codegen;
mod manifest;
mod model;
mod overlay;
mod project;
mod store;
mod toml_lite;

pub use digest::{audio_digest, fnv1a64, fnv1a64_hex, frame_digest};
pub use embed::{EmbeddedDir, recipes_from_embedded};
pub use embed_codegen::{generate_embed_code, write_embed};
pub use manifest::{
    BookManifest, ChapterManifest, DEFAULT_ORDER, Diagnostic, RecipeManifest, lint_dir, parse_book,
    parse_chapter, parse_recipe,
};
pub use model::{
    BookView, ChapterView, CheckResult, CookbookView, Expectation, FamilyView, GroupedView,
    LibView, RecipeCard, RecipeRun, RecipeSource,
};
pub use overlay::{OverlayDirectives, apply_directives, load_overlay, parse_overlay};
pub use project::{family_of, grouped_view, lib_view, next, ordered_cards, search, view};
pub use store::RecipeStore;
pub use toml_lite::TomlValue;
