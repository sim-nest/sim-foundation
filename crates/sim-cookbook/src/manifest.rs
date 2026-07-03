//! Typed cookbook manifests parsed from the TOML subset, with strict
//! validation and a filesystem lint pass.
//!
//! Three manifest kinds map to three files: `recipe.toml` (one recipe),
//! `book.toml` (one per crate `recipes/` dir), and `chapter.toml` (optional,
//! per chapter). Parsing is strict: required fields must be present and
//! non-empty, and unknown keys are rejected so typos surface immediately.

use std::path::Path;

use crate::model::Expectation;
use crate::toml_lite::{self, TomlDoc};

/// Default sort key when `order` is omitted.
pub const DEFAULT_ORDER: i64 = 1000;

/// One problem found while linting recipe files on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The file or directory the problem concerns.
    pub path: String,
    /// A human-readable description of the problem.
    pub message: String,
}

impl Diagnostic {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Parsed `recipe.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeManifest {
    /// Stable recipe id (last path segment of the runtime id).
    pub id: String,
    /// Human title.
    pub title: String,
    /// Registered codec name used to decode the setup file.
    pub codec: String,
    /// Setup file name, relative to the recipe directory.
    pub setup: String,
    /// Purpose file name, relative to the recipe directory.
    pub purpose: String,
    /// Sort key within the chapter.
    pub order: i64,
    /// Free tags.
    pub tags: Vec<String>,
    /// Lib ids that must be loaded for this recipe (owning lib added later).
    pub requires: Vec<String>,
    /// Declared expectations.
    pub expect: Vec<Expectation>,
}

/// Parsed `book.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookManifest {
    /// Owning lib id.
    pub book: String,
    /// Human title.
    pub title: String,
    /// One-line summary (may be empty).
    pub summary: String,
    /// Sort key among books.
    pub order: i64,
    /// Explicit chapter order by directory name (may be empty).
    pub chapters: Vec<String>,
}

/// Parsed `chapter.toml` (every field optional).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChapterManifest {
    /// Overriding chapter title.
    pub title: Option<String>,
    /// Overriding sort key.
    pub order: Option<i64>,
    /// One-line summary (may be empty).
    pub summary: String,
}

fn required_str(doc: &TomlDoc, key: &str) -> Result<String, String> {
    let value = doc
        .get(key)
        .ok_or_else(|| format!("missing required key `{key}`"))?;
    let text = value.as_str().map_err(|e| format!("`{key}`: {e}"))?;
    if text.is_empty() {
        return Err(format!("`{key}` must not be empty"));
    }
    Ok(text.to_string())
}

fn optional_order(doc: &TomlDoc) -> Result<i64, String> {
    match doc.get("order") {
        Some(value) => value.as_int().map_err(|e| format!("`order`: {e}")),
        None => Ok(DEFAULT_ORDER),
    }
}

fn optional_strings(doc: &TomlDoc, key: &str) -> Result<Vec<String>, String> {
    match doc.get(key) {
        Some(value) => Ok(value
            .as_array()
            .map_err(|e| format!("`{key}`: {e}"))?
            .to_vec()),
        None => Ok(Vec::new()),
    }
}

/// Parse and validate `recipe.toml` text.
pub fn parse_recipe(text: &str) -> Result<RecipeManifest, String> {
    let doc = toml_lite::parse(text)?;
    doc.reject_unknown_top(&[
        "id", "title", "codec", "setup", "purpose", "order", "tags", "requires",
    ])?;
    doc.reject_unknown_tables(&["expect"])?;
    let mut expect = Vec::new();
    for table in doc.tables_named("expect") {
        let form = table
            .iter()
            .find(|(k, _)| k == "form")
            .ok_or("`[[expect]]` missing `form`")?
            .1
            .as_int()
            .map_err(|e| format!("`[[expect]].form`: {e}"))?;
        if form < 0 {
            return Err("`[[expect]].form` must be >= 0".to_string());
        }
        let result = table
            .iter()
            .find(|(k, _)| k == "result")
            .ok_or("`[[expect]]` missing `result`")?
            .1
            .as_str()
            .map_err(|e| format!("`[[expect]].result`: {e}"))?
            .to_string();
        expect.push(Expectation {
            form: form as usize,
            result,
        });
    }
    Ok(RecipeManifest {
        id: required_str(&doc, "id")?,
        title: required_str(&doc, "title")?,
        codec: required_str(&doc, "codec")?,
        setup: required_str(&doc, "setup")?,
        purpose: required_str(&doc, "purpose")?,
        order: optional_order(&doc)?,
        tags: optional_strings(&doc, "tags")?,
        requires: optional_strings(&doc, "requires")?,
        expect,
    })
}

/// Parse and validate `book.toml` text.
pub fn parse_book(text: &str) -> Result<BookManifest, String> {
    let doc = toml_lite::parse(text)?;
    doc.reject_unknown_top(&["book", "title", "summary", "order", "chapters"])?;
    doc.reject_unknown_tables(&[])?;
    Ok(BookManifest {
        book: required_str(&doc, "book")?,
        title: required_str(&doc, "title")?,
        summary: doc
            .get("summary")
            .map(|v| v.as_str().map(str::to_string))
            .transpose()
            .map_err(|e| format!("`summary`: {e}"))?
            .unwrap_or_default(),
        order: optional_order(&doc)?,
        chapters: optional_strings(&doc, "chapters")?,
    })
}

/// Parse `chapter.toml` text (all fields optional).
pub fn parse_chapter(text: &str) -> Result<ChapterManifest, String> {
    let doc = toml_lite::parse(text)?;
    doc.reject_unknown_top(&["title", "order", "summary"])?;
    doc.reject_unknown_tables(&[])?;
    let title = match doc.get("title") {
        Some(v) => Some(v.as_str().map_err(|e| format!("`title`: {e}"))?.to_string()),
        None => None,
    };
    let order = match doc.get("order") {
        Some(v) => Some(v.as_int().map_err(|e| format!("`order`: {e}"))?),
        None => None,
    };
    let summary = match doc.get("summary") {
        Some(v) => v
            .as_str()
            .map_err(|e| format!("`summary`: {e}"))?
            .to_string(),
        None => String::new(),
    };
    Ok(ChapterManifest {
        title,
        order,
        summary,
    })
}

/// Lint one recipe directory on disk: parse `recipe.toml`, confirm the declared
/// setup and purpose files exist, and that required fields are present. Returns
/// every problem found, so an author sees all errors at once.
pub fn lint_dir(dir: &Path) -> Result<(), Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let recipe_path = dir.join("recipe.toml");
    let text = match std::fs::read_to_string(&recipe_path) {
        Ok(text) => text,
        Err(err) => {
            return Err(vec![Diagnostic::new(
                recipe_path.display().to_string(),
                format!("cannot read recipe.toml: {err}"),
            )]);
        }
    };
    let manifest = match parse_recipe(&text) {
        Ok(manifest) => manifest,
        Err(err) => {
            return Err(vec![Diagnostic::new(
                recipe_path.display().to_string(),
                err,
            )]);
        }
    };
    if !dir.join(&manifest.setup).is_file() {
        problems.push(Diagnostic::new(
            recipe_path.display().to_string(),
            format!("setup file `{}` does not exist", manifest.setup),
        ));
    }
    if !dir.join(&manifest.purpose).is_file() {
        problems.push(Diagnostic::new(
            recipe_path.display().to_string(),
            format!("purpose file `{}` does not exist", manifest.purpose),
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_RECIPE: &str = r#"
id = "add-two-numbers"
title = "Add two numbers"
codec = "lisp"
setup = "setup.siml"
purpose = "purpose.md"
order = 100
tags = ["arithmetic", "intro"]
requires = ["numbers-f64"]
[[expect]]
form = 0
result = "3"
"#;

    #[test]
    fn parses_valid_recipe() {
        let m = parse_recipe(VALID_RECIPE).unwrap();
        assert_eq!(m.id, "add-two-numbers");
        assert_eq!(m.codec, "lisp");
        assert_eq!(m.order, 100);
        assert_eq!(m.tags, ["arithmetic", "intro"]);
        assert_eq!(
            m.expect,
            [Expectation {
                form: 0,
                result: "3".into()
            }]
        );
    }

    #[test]
    fn recipe_order_defaults() {
        let m = parse_recipe(
            "id = \"x\"\ntitle = \"X\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\n",
        )
        .unwrap();
        assert_eq!(m.order, DEFAULT_ORDER);
        assert!(m.tags.is_empty());
        assert!(m.requires.is_empty());
    }

    #[test]
    fn missing_required_field_errors_clearly() {
        let err = parse_recipe("title = \"X\"\ncodec = \"lisp\"\n").unwrap_err();
        assert!(err.contains("missing required key `id`"), "{err}");
    }

    #[test]
    fn empty_required_field_errors() {
        let err = parse_recipe(
            "id = \"\"\ntitle = \"X\"\ncodec = \"l\"\nsetup = \"s\"\npurpose = \"p\"\n",
        )
        .unwrap_err();
        assert!(err.contains("must not be empty"), "{err}");
    }

    #[test]
    fn unknown_key_rejected() {
        let err = parse_recipe(
            "id = \"x\"\ntitle = \"X\"\ncodec = \"l\"\nsetup = \"s\"\npurpose = \"p\"\nbogus = 1\n",
        )
        .unwrap_err();
        assert!(err.contains("unknown key `bogus`"), "{err}");
    }

    #[test]
    fn parses_book_and_chapter() {
        let book = parse_book(
            "book = \"numbers-f64\"\ntitle = \"Numbers\"\norder = 200\nchapters = [\"01-basics\"]\n",
        )
        .unwrap();
        assert_eq!(book.book, "numbers-f64");
        assert_eq!(book.order, 200);
        assert_eq!(book.chapters, ["01-basics"]);

        let chapter = parse_chapter("title = \"Basics\"\norder = 10\n").unwrap();
        assert_eq!(chapter.title.as_deref(), Some("Basics"));
        assert_eq!(chapter.order, Some(10));
    }

    #[test]
    fn expect_missing_result_errors() {
        let err = parse_recipe(
            "id = \"x\"\ntitle = \"X\"\ncodec = \"l\"\nsetup = \"s\"\npurpose = \"p\"\n[[expect]]\nform = 0\n",
        )
        .unwrap_err();
        assert!(err.contains("missing `result`"), "{err}");
    }

    fn temp_recipe_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sim-cookbook-lint-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lint_dir_accepts_complete_recipe() {
        let dir = temp_recipe_dir("ok");
        std::fs::write(dir.join("recipe.toml"), VALID_RECIPE).unwrap();
        std::fs::write(dir.join("setup.siml"), "(+ 1 2)").unwrap();
        std::fs::write(dir.join("purpose.md"), "Add.").unwrap();
        assert!(lint_dir(&dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lint_dir_reports_missing_setup_file() {
        let dir = temp_recipe_dir("missing-setup");
        std::fs::write(dir.join("recipe.toml"), VALID_RECIPE).unwrap();
        std::fs::write(dir.join("purpose.md"), "Add.").unwrap();
        let problems = lint_dir(&dir).unwrap_err();
        assert!(
            problems.iter().any(|d| d.message.contains("setup.siml")),
            "{problems:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
