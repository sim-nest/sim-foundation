//! Build-time codegen that embeds a crate's `recipes/` directory.
//!
//! A lib crate that ships recipes calls [`write_embed`] from its `build.rs`:
//!
//! ```ignore
//! // build.rs
//! fn main() {
//!     sim_cookbook::write_embed("recipes").unwrap();
//! }
//! ```
//!
//! and includes the generated slice as its [`crate::EmbeddedDir`]:
//!
//! ```ignore
//! pub static RECIPES: sim_cookbook::EmbeddedDir =
//!     include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));
//! ```
//!
//! The generated file is a single `&[(path, include_bytes!(abs))]` literal, so
//! every recipe file is baked into the binary at compile time and rebuilt when
//! the tree changes. A crate with no `recipes/` directory gets an empty slice.

use std::io;
use std::path::{Path, PathBuf};

/// Generate the embed slice and write it to `$OUT_DIR/cookbook_recipes.rs`.
/// `recipes_subdir` is relative to `$CARGO_MANIFEST_DIR`. Intended for use from
/// a crate's `build.rs`.
pub fn write_embed(recipes_subdir: &str) -> io::Result<()> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| io::Error::other("CARGO_MANIFEST_DIR not set (call from build.rs)"))?;
    let out_dir = std::env::var("OUT_DIR")
        .map_err(|_| io::Error::other("OUT_DIR not set (call from build.rs)"))?;
    let root = Path::new(&manifest_dir).join(recipes_subdir);
    println!("cargo:rerun-if-changed={}", root.display());
    let code = generate_embed_code(&root)?;
    std::fs::write(Path::new(&out_dir).join("cookbook_recipes.rs"), code)
}

/// Walk `recipes_root` and return the Rust source for an [`crate::EmbeddedDir`]
/// literal: a sorted `&[(rel-path, include_bytes!(abs-path))]`. A missing or
/// empty directory yields an empty slice.
pub fn generate_embed_code(recipes_root: &Path) -> io::Result<String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    if recipes_root.is_dir() {
        collect(recipes_root, recipes_root, &mut files)?;
    }
    files.sort();
    let mut out = String::from("&[\n");
    for (rel, abs) in &files {
        // `{:?}` emits a valid, escaped Rust string literal for each path.
        out.push_str(&format!(
            "    ({:?}, include_bytes!({:?}) as &[u8]),\n",
            rel,
            abs.to_string_lossy(),
        ));
    }
    out.push_str("]\n");
    Ok(out)
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(base, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .map_err(io::Error::other)?
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push((rel, path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_yields_empty_slice() {
        let dir = std::env::temp_dir().join(format!("sim-cb-embed-empty-{}", std::process::id()));
        let code = generate_embed_code(&dir).unwrap();
        assert_eq!(code, "&[\n]\n");
    }

    #[test]
    fn generates_sorted_include_bytes() {
        let root = std::env::temp_dir().join(format!("sim-cb-embed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("01-basics/add")).unwrap();
        std::fs::write(root.join("book.toml"), b"book = \"x\"\n").unwrap();
        std::fs::write(root.join("01-basics/add/recipe.toml"), b"id=\"a\"\n").unwrap();
        let code = generate_embed_code(&root).unwrap();
        assert!(code.contains("\"01-basics/add/recipe.toml\""), "{code}");
        assert!(code.contains("\"book.toml\""), "{code}");
        assert!(code.contains("include_bytes!("), "{code}");
        // Entries are sorted by relative path: "01-basics/..." precedes
        // "book.toml" lexicographically, so the nested recipe comes first.
        let recipe_at = code.find("recipe.toml").unwrap();
        let book_at = code.find("book.toml").unwrap();
        assert!(recipe_at < book_at, "expected sorted order, got:\n{code}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
