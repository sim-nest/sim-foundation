//! Portable host-side build support for cookbook recipe embedding.
//!
//! Product crates use this library only from `build.rs`. Runtime cookbook
//! behavior remains in `sim-cookbook`, over already-supplied embedded bytes.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::io;
use std::path::{Path, PathBuf};

/// Generate the embed slice and write it to `$OUT_DIR/cookbook_recipes.rs`.
/// `recipes_subdir` is relative to `$CARGO_MANIFEST_DIR`.
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

/// Return the deterministic Rust source embedding all files below `recipes_root`.
pub fn generate_embed_code(recipes_root: &Path) -> io::Result<String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    if recipes_root.is_dir() {
        collect(recipes_root, recipes_root, &mut files)?;
    }
    files.sort();
    validate_embedded_tree(recipes_root, &files)?;
    let mut out = String::from("&[\n");
    for (relative, absolute) in &files {
        out.push_str(&format!(
            "    ({:?}, include_bytes!({:?}) as &[u8]),\n",
            relative,
            absolute.to_string_lossy(),
        ));
    }
    out.push_str("]\n");
    Ok(out)
}

fn validate_embedded_tree(recipes_root: &Path, files: &[(String, PathBuf)]) -> io::Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    let owned = files
        .iter()
        .map(|(relative, path)| std::fs::read(path).map(|bytes| (relative.as_str(), bytes)))
        .collect::<io::Result<Vec<_>>>()?;
    let embedded = owned
        .iter()
        .map(|(relative, bytes)| (*relative, bytes.as_slice()))
        .collect::<Vec<_>>();
    sim_cookbook::recipes_from_embedded(&embedded).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid cookbook tree at {}: {error}",
                recipes_root.display()
            ),
        )
    })?;
    Ok(())
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    let mut entries = std::fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(base, &path, out)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(base)
                .map_err(io::Error::other)?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_bytes_match_the_legacy_contract() {
        let root = std::env::temp_dir().join(format!("sim-cookbook-build-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("01-basics/add")).unwrap();
        std::fs::write(
            root.join("book.toml"),
            b"book = \"x\"\ntitle = \"X\"\nchapters = [\"01-basics\"]\n",
        )
        .unwrap();
        std::fs::write(root.join("01-basics/add/recipe.toml"), b"id = \"a\"\ntitle = \"A\"\ncodec = \"lisp\"\nsetup = \"setup.siml\"\npurpose = \"purpose.md\"\n").unwrap();
        std::fs::write(root.join("01-basics/add/setup.siml"), b"(+ 1 2)\n").unwrap();
        std::fs::write(root.join("01-basics/add/purpose.md"), b"Add values.\n").unwrap();
        let code = generate_embed_code(&root).unwrap();
        assert!(code.starts_with("&[\n    (\"01-basics/add/purpose.md\", include_bytes!("));
        assert!(code.ends_with(") as &[u8]),\n]\n"));
        let _ = std::fs::remove_dir_all(root);
    }
}
