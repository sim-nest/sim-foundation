//! Typed cookbook manifests parsed from the TOML subset, with strict
//! validation and a filesystem lint pass.
//!
//! Three manifest kinds map to three files: `recipe.toml` (one recipe),
//! `book.toml` (one per crate `recipes/` dir), and `chapter.toml` (optional,
//! per chapter). Parsing is strict: required fields must be present and
//! non-empty, and unknown keys are rejected so typos surface immediately.

use std::path::{Path, PathBuf};

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
    /// Registered codec name for decoding the setup file.
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

impl RecipeManifest {
    /// Validate the manifest fields whose meaning depends on a recipe
    /// directory: embedded relative paths and other per-dir invariants.
    pub fn validate_for_dir(&self) -> Result<(), Vec<String>> {
        let mut problems = Vec::new();
        if let Err(err) = validate_recipe_rel_path("setup", &self.setup) {
            problems.push(err);
        }
        if let Err(err) = validate_recipe_rel_path("purpose", &self.purpose) {
            problems.push(err);
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
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

fn validate_recipe_rel_path(field: &str, value: &str) -> Result<(), String> {
    if value.starts_with('/') || value.starts_with('\\') {
        return Err(format!("`{field}` must be a relative slash path"));
    }
    if value.len() >= 2 && value.as_bytes()[1] == b':' && value.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(format!("`{field}` must be a relative slash path"));
    }
    if value.contains('\\') {
        return Err(format!("`{field}` must use `/` separators only"));
    }
    for component in value.split('/') {
        if component.is_empty() {
            return Err(format!("`{field}` must not contain empty path components"));
        }
        if component == "." {
            return Err(format!("`{field}` must not contain `.` path components"));
        }
        if component == ".." {
            return Err(format!("`{field}` must not contain `..` path components"));
        }
    }
    Ok(())
}

fn resolve_recipe_rel_path(base: &Path, value: &str) -> PathBuf {
    let mut path = base.to_path_buf();
    for component in value.split('/') {
        path.push(component);
    }
    path
}

/// Parse and validate `recipe.toml` text.
///
/// Unknown top-level keys and unknown `[[table]]`s are IGNORED, not rejected
/// (COOK8.00). A recipe may carry a rich descriptor vocabulary beyond the core
/// recipe fields -- the `30-agents` `a30-*` descriptors add `recipe_number`,
/// `source_chapter`, `descriptor_shape`, `assert_*`, and more -- and every such
/// recipe must still embed and aggregate into the complete cookbook catalog.
/// Rejecting unknown keys blocked embedding those libs (the EMBED-ALL HAZARD),
/// which the COVERAGE goal cannot tolerate. Typo protection on the CORE fields
/// stays: a missing or empty required key (`id`/`title`/`codec`/`setup`/
/// `purpose`) still errors, and the on-disk `cookbook-lint` gate remains the
/// place for structural recipe linting.
pub fn parse_recipe(text: &str) -> Result<RecipeManifest, String> {
    let doc = toml_lite::parse(text)?;
    parse_recipe_doc(&doc, None)
}

#[derive(Clone, Copy)]
pub(crate) struct RecipeDefaults<'a> {
    pub(crate) id: &'a str,
    pub(crate) codec: &'a str,
    pub(crate) setup: &'a str,
}

fn required_str_or(doc: &TomlDoc, key: &str, fallback: Option<&str>) -> Result<String, String> {
    match doc.get(key) {
        Some(_) => required_str(doc, key),
        None => fallback
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("missing required key `{key}`")),
    }
}

pub(crate) fn parse_recipe_doc(
    doc: &TomlDoc,
    defaults: Option<RecipeDefaults<'_>>,
) -> Result<RecipeManifest, String> {
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
        id: required_str_or(doc, "id", defaults.map(|value| value.id))?,
        title: required_str(doc, "title")?,
        codec: required_str_or(doc, "codec", defaults.map(|value| value.codec))?,
        setup: required_str_or(doc, "setup", defaults.map(|value| value.setup))?,
        purpose: required_str(doc, "purpose")?,
        order: optional_order(doc)?,
        tags: optional_strings(doc, "tags")?,
        requires: optional_strings(doc, "requires")?,
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
///
/// Like [`parse_recipe`], unknown keys and tables are IGNORED rather than
/// rejected (COOK8.03). Rich `30-agents`/`40-atelier` chapters across the
/// constellation carry an extended vocabulary (e.g. `tags`) beyond the core
/// `title`/`order`/`summary`, and every such chapter must embed and aggregate
/// into the complete catalog -- rejecting unknown keys blocked embedding those
/// books (the EMBED-ALL HAZARD). Structural chapter linting stays with the
/// on-disk `cookbook-lint` gate.
pub fn parse_chapter(text: &str) -> Result<ChapterManifest, String> {
    let doc = toml_lite::parse(text)?;
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
    lint_dir_impl(dir, false)
}

/// Lint one recipe directory and reject bare-quote setup files.
///
/// This stricter gate is kept separate from [`lint_dir`] while the constellation
/// still contains older recipe setups that are being converted repo by repo.
pub fn lint_dir_strict_no_quote(dir: &Path) -> Result<(), Vec<Diagnostic>> {
    lint_dir_impl(dir, true)
}

fn lint_dir_impl(dir: &Path, strict_no_quote: bool) -> Result<(), Vec<Diagnostic>> {
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
    let validated = match manifest.validate_for_dir() {
        Ok(()) => true,
        Err(errors) => {
            for err in errors {
                problems.push(Diagnostic::new(recipe_path.display().to_string(), err));
            }
            false
        }
    };
    let setup_path = if validated {
        Some(resolve_recipe_rel_path(dir, &manifest.setup))
    } else {
        None
    };
    if let Some(setup_path) = setup_path.as_ref() {
        if !setup_path.is_file() {
            problems.push(Diagnostic::new(
                recipe_path.display().to_string(),
                format!("setup file `{}` does not exist", manifest.setup),
            ));
        } else if strict_no_quote {
            match std::fs::read_to_string(setup_path) {
                Ok(setup) => {
                    if setup_is_bare_quote(&setup) {
                        problems.push(Diagnostic::new(
                            setup_path.display().to_string(),
                            "recipe setup must not be a bare quote; use an operation, codec form, or read-construct",
                        ));
                    }
                }
                Err(err) => problems.push(Diagnostic::new(
                    setup_path.display().to_string(),
                    format!("cannot read setup file `{}`: {err}", manifest.setup),
                )),
            }
        }
    }
    if validated {
        let purpose_path = resolve_recipe_rel_path(dir, &manifest.purpose);
        if !purpose_path.is_file() {
            problems.push(Diagnostic::new(
                recipe_path.display().to_string(),
                format!("purpose file `{}` does not exist", manifest.purpose),
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn setup_is_bare_quote(source: &str) -> bool {
    let trimmed = source.trim_start();
    trimmed.starts_with("(quote") || trimmed.starts_with("( quote")
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
    fn unknown_key_ignored_not_rejected() {
        // COOK8.00: unknown top-level keys are ignored so rich descriptors embed.
        let m = parse_recipe(
            "id = \"x\"\ntitle = \"X\"\ncodec = \"l\"\nsetup = \"s\"\npurpose = \"p\"\nbogus = 1\n",
        )
        .unwrap();
        assert_eq!(m.id, "x");
    }

    #[test]
    fn manifest_validate_for_dir_rejects_unsafe_paths() {
        let mut manifest = parse_recipe(
            "id = \"x\"\ntitle = \"X\"\ncodec = \"l\"\nsetup = \"../setup.siml\"\npurpose = \"/tmp/purpose.md\"\n",
        )
        .unwrap();
        let err = manifest.validate_for_dir().unwrap_err();
        assert!(
            err.iter()
                .any(|msg| msg.contains("`setup` must not contain `..`")),
            "{err:?}"
        );
        assert!(
            err.iter()
                .any(|msg| msg.contains("`purpose` must be a relative slash path")),
            "{err:?}"
        );

        manifest.setup = "nested\\setup.siml".to_string();
        manifest.purpose = "notes//purpose.md".to_string();
        let err = manifest.validate_for_dir().unwrap_err();
        assert!(
            err.iter()
                .any(|msg| msg.contains("`setup` must use `/` separators only")),
            "{err:?}"
        );
        assert!(
            err.iter()
                .any(|msg| msg.contains("`purpose` must not contain empty path components")),
            "{err:?}"
        );
    }

    #[test]
    fn parses_rich_descriptor_keys() {
        // An `a30-*` descriptor carries a structured vocabulary beyond the core
        // recipe fields. It must parse (and thus embed) unchanged; the extra
        // keys are ignored, and the core fields still resolve.
        let rich = r#"
id = "a30-009-agentic-workflow"
title = "Agentic workflow"
codec = "lisp"
setup = "setup.siml"
purpose = "purpose.md"
order = 9
tags = ["30-agents", "sandbox-descriptor"]
requires = ["agent", "codec/lisp"]
recipe_number = 9
source_chapter = 7
architecture_family = "agentic-workflow"
runner_mode = "fake"
safety_posture = "offline"
capabilities = ["read-eval", "workflow-state"]
descriptor_shape = "agentic-workflow-trace"
assert_tags = ["30-agents", "chapter-07"]
assert_capabilities = ["read-eval"]
assert_setup_codec = "lisp"
expected = "expected.txt"
[[expect]]
form = 0
result = "(agentic-workflow-trace)"
"#;
        let m = parse_recipe(rich).unwrap();
        assert_eq!(m.id, "a30-009-agentic-workflow");
        assert_eq!(m.codec, "lisp");
        assert_eq!(m.order, 9);
        assert_eq!(m.requires, ["agent", "codec/lisp"]);
        assert_eq!(m.expect[0].result, "(agentic-workflow-trace)");
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
    fn chapter_unknown_key_ignored_not_rejected() {
        // COOK8.03: rich 30-agents chapters carry `tags` beyond the core chapter
        // fields; they must embed unchanged, so unknown keys are ignored.
        let chapter = parse_chapter(
            "title = \"30 Agents\"\norder = 20\nsummary = \"s\"\ntags = [\"30-agents\"]\n",
        )
        .unwrap();
        assert_eq!(chapter.title.as_deref(), Some("30 Agents"));
        assert_eq!(chapter.order, Some(20));
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

    #[test]
    fn lint_dir_allows_manifest_id_that_differs_from_the_directory_name() {
        let dir = temp_recipe_dir("browser-facade");
        std::fs::write(
            dir.join("recipe.toml"),
            VALID_RECIPE.replace("add-two-numbers", "frame-facade"),
        )
        .unwrap();
        std::fs::write(dir.join("setup.siml"), "(+ 1 2)").unwrap();
        std::fs::write(dir.join("purpose.md"), "Add.").unwrap();
        assert!(lint_dir(&dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lint_dir_reports_parent_path_components() {
        let dir = temp_recipe_dir("parent-path");
        std::fs::write(
            dir.join("recipe.toml"),
            VALID_RECIPE.replace("setup.siml", "../setup.siml"),
        )
        .unwrap();
        std::fs::write(dir.join("purpose.md"), "Add.").unwrap();
        let problems = lint_dir(&dir).unwrap_err();
        assert!(
            problems
                .iter()
                .any(|d| d.message.contains("`setup` must not contain `..`")),
            "{problems:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lint_dir_reports_absolute_and_backslash_paths() {
        let dir = temp_recipe_dir("absolute-path");
        std::fs::write(
            dir.join("recipe.toml"),
            VALID_RECIPE
                .replace("setup.siml", "nested\\\\setup.siml")
                .replace("purpose.md", "/tmp/purpose.md"),
        )
        .unwrap();
        let problems = lint_dir(&dir).unwrap_err();
        assert!(
            problems
                .iter()
                .any(|d| d.message.contains("`setup` must use `/` separators only")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|d| d
                .message
                .contains("`purpose` must be a relative slash path")),
            "{problems:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn write_recipe_with_setup(dir: &Path, setup: &str) {
        std::fs::write(dir.join("recipe.toml"), VALID_RECIPE).unwrap();
        std::fs::write(dir.join("setup.siml"), setup).unwrap();
        std::fs::write(dir.join("purpose.md"), "Add.").unwrap();
    }

    #[test]
    fn default_lint_allows_bare_quote_until_strict_gate_is_requested() {
        let dir = temp_recipe_dir("default-quote");
        write_recipe_with_setup(&dir, "(quote add-two-numbers)");
        assert!(lint_dir(&dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strict_lint_reports_bare_quote_setups() {
        for (tag, setup) in [
            ("single-line-quote", "(quote add-two-numbers)"),
            ("spaced-quote", "  ( quote add-two-numbers)"),
            ("multi-line-quote", "\n(quote\n  add-two-numbers\n)"),
        ] {
            let dir = temp_recipe_dir(tag);
            write_recipe_with_setup(&dir, setup);
            let problems = lint_dir_strict_no_quote(&dir).unwrap_err();
            assert!(
                problems.iter().any(|d| {
                    d.path.ends_with("setup.siml") && d.message.contains("must not be a bare quote")
                }),
                "{tag}: {problems:?}"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
