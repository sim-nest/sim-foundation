//! The user overlay: reorder, hide, retitle, and add recipes without editing
//! any crate.
//!
//! The overlay directory (default `~/.config/sim/cookbook/`) may hold an
//! `overlay.toml` of directives and, optionally, a `book.toml`-rooted recipe
//! tree laid out exactly like a crate's `recipes/` dir. Overlay recipes carry
//! [`RecipeSource::Overlay`]; giving the overlay book the same `book` id as a
//! crate's book merges the recipes into that book (the view groups by book id).
//!
//! Merge precedence (Section 11 of the design): crate recipes load first;
//! overlay recipes with the same id replace them (`upsert`), new ids are
//! appended, then directives apply.

use std::path::Path;

use crate::embed::recipes_from_embedded;
use crate::model::RecipeSource;
use crate::store::RecipeStore;
use crate::toml_lite;

/// Parsed `overlay.toml` directives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OverlayDirectives {
    /// `(recipe id, new order)` from `[[reorder]]`.
    pub reorder: Vec<(String, i64)>,
    /// recipe ids from `[[hide]]`.
    pub hide: Vec<String>,
    /// `(recipe id, new title)` from `[[retitle]]`.
    pub retitle: Vec<(String, String)>,
}

fn table_str(table: &[(String, toml_lite::TomlValue)], key: &str) -> Result<String, String> {
    table
        .iter()
        .find(|(k, _)| k == key)
        .ok_or_else(|| format!("`[[..]]` table missing `{key}`"))?
        .1
        .as_str()
        .map(str::to_string)
        .map_err(|e| format!("`{key}`: {e}"))
}

fn table_int(table: &[(String, toml_lite::TomlValue)], key: &str) -> Result<i64, String> {
    table
        .iter()
        .find(|(k, _)| k == key)
        .ok_or_else(|| format!("`[[..]]` table missing `{key}`"))?
        .1
        .as_int()
        .map_err(|e| format!("`{key}`: {e}"))
}

/// Parse `overlay.toml` text into directives.
pub fn parse_overlay(text: &str) -> Result<OverlayDirectives, String> {
    let doc = toml_lite::parse(text)?;
    doc.reject_unknown_top(&[])?;
    doc.reject_unknown_tables(&["reorder", "hide", "retitle"])?;
    let mut directives = OverlayDirectives::default();
    for table in doc.tables_named("reorder") {
        directives
            .reorder
            .push((table_str(table, "recipe")?, table_int(table, "order")?));
    }
    for table in doc.tables_named("hide") {
        directives.hide.push(table_str(table, "recipe")?);
    }
    for table in doc.tables_named("retitle") {
        directives
            .retitle
            .push((table_str(table, "recipe")?, table_str(table, "title")?));
    }
    Ok(directives)
}

/// Apply directives to a store: hide removes, reorder sets `order`, retitle sets
/// `title`. Directives naming an unknown recipe are ignored (the overlay may
/// reference recipes from libs not currently loaded).
pub fn apply_directives(store: &mut RecipeStore, directives: &OverlayDirectives) {
    for id in &directives.hide {
        store.remove(id);
    }
    for (id, order) in &directives.reorder {
        if let Some(card) = store.card_mut(id) {
            card.order = *order;
        }
    }
    for (id, title) in &directives.retitle {
        if let Some(card) = store.card_mut(id) {
            card.title = title.clone();
        }
    }
}

/// Read a `book.toml`-rooted recipe tree from disk into the embedded form.
/// Skips `overlay.toml`. Returns an empty list if there is no `book.toml`.
fn read_tree(root: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    if !root.join("book.toml").is_file() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    Ok(files)
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut entries: Vec<_> = entries
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(base, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if rel == "overlay.toml" {
                continue;
            }
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            out.push((rel, bytes));
        }
    }
    Ok(())
}

/// Load the overlay at `root` into `store`: upsert any overlay recipes (so they
/// override crate recipes by id and merge into matching books), then apply the
/// `overlay.toml` directives. A missing directory is a no-op.
pub fn load_overlay(store: &mut RecipeStore, root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let owned = read_tree(root)?;
    if !owned.is_empty() {
        let borrowed: Vec<(&str, &[u8])> = owned
            .iter()
            .map(|(p, b)| (p.as_str(), b.as_slice()))
            .collect();
        let path = root.display().to_string();
        for mut card in recipes_from_embedded(&borrowed)? {
            card.source = RecipeSource::Overlay { path: path.clone() };
            store.upsert_card(card);
        }
    }
    let overlay_toml = root.join("overlay.toml");
    if overlay_toml.is_file() {
        let text = std::fs::read_to_string(&overlay_toml).map_err(|e| e.to_string())?;
        let directives = parse_overlay(&text)?;
        apply_directives(store, &directives);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::view;

    fn base_store() -> RecipeStore {
        let alpha: Vec<(&str, &[u8])> = vec![
            (
                "book.toml",
                b"book = \"alpha\"\ntitle = \"Alpha\"\norder = 100\n" as &[u8],
            ),
            (
                "01-basics/add/recipe.toml",
                b"id = \"add\"\ntitle = \"Add\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\norder = 100\n",
            ),
            ("01-basics/add/s", b"(+ 1 2)"),
            ("01-basics/add/p", b"add"),
            (
                "01-basics/sub/recipe.toml",
                b"id = \"sub\"\ntitle = \"Subtract\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\norder = 200\n",
            ),
            ("01-basics/sub/s", b"(- 3 1)"),
            ("01-basics/sub/p", b"sub"),
        ];
        let mut store = RecipeStore::new();
        store.register_book(&alpha).unwrap();
        store
    }

    #[test]
    fn parse_overlay_reads_all_directives() {
        let d = parse_overlay(
            "[[reorder]]\nrecipe = \"alpha/01-basics/sub\"\norder = 1\n[[hide]]\nrecipe = \"alpha/01-basics/add\"\n[[retitle]]\nrecipe = \"alpha/01-basics/sub\"\ntitle = \"Minus\"\n",
        )
        .unwrap();
        assert_eq!(d.reorder, [("alpha/01-basics/sub".to_string(), 1)]);
        assert_eq!(d.hide, ["alpha/01-basics/add"]);
        assert_eq!(
            d.retitle,
            [("alpha/01-basics/sub".to_string(), "Minus".to_string())]
        );
    }

    #[test]
    fn hide_reorder_retitle_apply() {
        let mut store = base_store();
        let directives = OverlayDirectives {
            reorder: vec![("alpha/01-basics/sub".to_string(), 1)],
            hide: vec!["alpha/01-basics/add".to_string()],
            retitle: vec![("alpha/01-basics/sub".to_string(), "Minus".to_string())],
        };
        apply_directives(&mut store, &directives);
        assert!(store.card("alpha/01-basics/add").is_none()); // hidden
        let sub = store.card("alpha/01-basics/sub").unwrap();
        assert_eq!(sub.order, 1); // reordered
        assert_eq!(sub.title, "Minus"); // retitled
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sim-cb-overlay-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn overlay_adds_to_foreign_book_and_applies_directives() {
        let root = temp_dir("foreign");
        std::fs::create_dir_all(root.join("99-extra/mul")).unwrap();
        // Same book id "alpha" -> merges into the existing book.
        std::fs::write(
            root.join("book.toml"),
            b"book = \"alpha\"\ntitle = \"Alpha\"\norder = 100\n",
        )
        .unwrap();
        std::fs::write(
            root.join("99-extra/mul/recipe.toml"),
            b"id = \"mul\"\ntitle = \"Multiply\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\norder = 100\n",
        )
        .unwrap();
        std::fs::write(root.join("99-extra/mul/s"), b"(* 2 3)").unwrap();
        std::fs::write(root.join("99-extra/mul/p"), b"multiply").unwrap();
        std::fs::write(
            root.join("overlay.toml"),
            b"[[hide]]\nrecipe = \"alpha/01-basics/sub\"\n",
        )
        .unwrap();

        let mut store = base_store();
        load_overlay(&mut store, &root).unwrap();

        // The overlay recipe joined book "alpha".
        let mul = store.card("alpha/99-extra/mul").unwrap();
        assert_eq!(
            mul.source,
            RecipeSource::Overlay {
                path: root.display().to_string()
            }
        );
        // The directive hid sub.
        assert!(store.card("alpha/01-basics/sub").is_none());
        // The view shows alpha with two chapters (01-basics add, 99-extra mul).
        let view = view(&store);
        assert_eq!(view.books.len(), 1);
        let chapters: Vec<&str> = view.books[0]
            .chapters
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(chapters, ["01-basics", "99-extra"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn overlay_recipe_overrides_crate_recipe_by_id() {
        let root = temp_dir("override");
        std::fs::create_dir_all(root.join("01-basics/add")).unwrap();
        std::fs::write(
            root.join("book.toml"),
            b"book = \"alpha\"\ntitle = \"Alpha\"\norder = 100\n",
        )
        .unwrap();
        std::fs::write(
            root.join("01-basics/add/recipe.toml"),
            b"id = \"add\"\ntitle = \"Overridden Add\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\n",
        )
        .unwrap();
        std::fs::write(root.join("01-basics/add/s"), b"(+ 9 9)").unwrap();
        std::fs::write(root.join("01-basics/add/p"), b"overridden").unwrap();

        let mut store = base_store();
        assert_eq!(store.len(), 2);
        load_overlay(&mut store, &root).unwrap();
        assert_eq!(store.len(), 2); // upsert, not append
        assert_eq!(
            store.card("alpha/01-basics/add").unwrap().title,
            "Overridden Add"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_overlay_dir_is_noop() {
        let mut store = base_store();
        load_overlay(&mut store, Path::new("/nonexistent/sim/cookbook")).unwrap();
        assert_eq!(store.len(), 2);
    }
}
