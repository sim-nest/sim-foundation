//! Plain data model for the cookbook.
//!
//! A recipe is a tiny runnable lesson shipped by the crate it teaches. These
//! types carry one recipe's data (a [`RecipeCard`]) and the computed grouping
//! of all loaded recipes into books and chapters (a [`CookbookView`]). They are
//! deliberately free of kernel types: recipes flow through the existing
//! Card/registry data surface, so the kernel gains no cookbook enum.

/// Where a recipe came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeSource {
    /// Shipped inside a crate, embedded in its compiled lib.
    Crate {
        /// The lib id that owns the recipe.
        lib: String,
    },
    /// Supplied locally by the user overlay directory.
    Overlay {
        /// The overlay file path the recipe was loaded from.
        path: String,
    },
}

/// One declared expectation, turning a recipe into a generated test.
///
/// After running the recipe's setup, the encoded result of form `form`
/// (0-based) must equal `result` (encoded in the recipe's codec).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expectation {
    /// Index of the setup form whose result is checked.
    pub form: usize,
    /// Expected encoded result, in the recipe's codec.
    pub result: String,
}

/// The in-memory record for one recipe. Registered as a Card of kind
/// `"recipe"`; the cookbook view is a projection over these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeCard {
    /// Globally unique id: `"<book>/<chapter>/<recipe-id>"`.
    pub id: String,
    /// Owning lib id (the book).
    pub book: String,
    /// Chapter directory name (or overlay chapter).
    pub chapter: String,
    /// Resolved human chapter title.
    pub chapter_title: String,
    /// One-line chapter summary (may be empty).
    pub chapter_summary: String,
    /// Human recipe title.
    pub title: String,
    /// Registered codec name for decoding `setup`.
    pub codec: String,
    /// Raw setup bytes, decoded on demand through `codec`.
    pub setup: Vec<u8>,
    /// Purpose document contents (Markdown, ASCII).
    pub purpose: String,
    /// Sort key within the chapter; lower runs first.
    pub order: i64,
    /// Sort key of this recipe's chapter among chapters.
    pub chapter_order: i64,
    /// Sort key of this recipe's book among books.
    pub book_order: i64,
    /// Human book title.
    pub book_title: String,
    /// One-line book summary (may be empty).
    pub book_summary: String,
    /// Free tags for search and filtering.
    pub tags: Vec<String>,
    /// Lib ids that must be loaded for this recipe to run.
    pub requires: Vec<String>,
    /// Declared expectations (empty when the recipe is not a test).
    pub expect: Vec<Expectation>,
    /// Provenance of this recipe.
    pub source: RecipeSource,
}

/// The full computed cookbook: every loaded recipe grouped into books.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CookbookView {
    /// Books sorted by `book_order`, then book id.
    pub books: Vec<BookView>,
}

/// One loadable lib entry in the top-level cookbook tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibView {
    /// Cookbook-facing lib id, such as `numbers/i64`.
    pub id: String,
    /// Human lib title.
    pub title: String,
    /// Whether the lib is loaded in the projected recipe store.
    pub loaded: bool,
    /// Grouped recipe chapters for loaded libs.
    pub groups: Vec<ChapterView>,
    /// Lifecycle load recipes for unloaded libs.
    pub recipes: Vec<RecipeCard>,
}

/// One book (all recipes from one crate) in a [`CookbookView`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookView {
    /// Lib id of the book.
    pub id: String,
    /// Human book title.
    pub title: String,
    /// One-line book summary (may be empty).
    pub summary: String,
    /// Chapters sorted by `chapter_order`, then name.
    pub chapters: Vec<ChapterView>,
}

/// The full catalog grouped into a two-level tree: family -> domain (book) ->
/// chapter -> recipe (COOK8.00 COVERAGE axis).
///
/// The grouping needs NO per-recipe metadata: the level-1 family is derived from
/// each book's id prefix (`numbers/cas` -> `numbers`, `organ/binding` ->
/// `organ`, `audio-dsp` -> `audio`), and the level-2 domain is the book itself.
/// So the whole constellation browses by subsystem without a hand-maintained
/// group list.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GroupedView {
    /// Families in the order their lowest-ordered book appears in
    /// [`CookbookView`] (book order is total, so this order is deterministic).
    pub families: Vec<FamilyView>,
}

/// One level-1 family (subsystem) in a [`GroupedView`], holding its domain books.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyView {
    /// Family id derived from the book prefix (`numbers`, `organ`, `codec`, ...).
    pub family: String,
    /// The domain books in this family, in the same order as [`CookbookView`].
    pub books: Vec<BookView>,
}

/// One chapter within a [`BookView`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChapterView {
    /// Chapter directory name (stable id within the book).
    pub name: String,
    /// Human chapter title.
    pub title: String,
    /// One-line chapter summary (may be empty).
    pub summary: String,
    /// Recipes sorted by `order`, then id.
    pub recipes: Vec<RecipeCard>,
}

/// The outcome of running a recipe's setup through its codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeRun {
    /// The recipe id that was run.
    pub recipe: String,
    /// Number of setup forms evaluated.
    pub forms: usize,
    /// Encoded result of each form, in the recipe's codec.
    pub results: Vec<String>,
    /// Expectation check results (empty when no expectations).
    pub checks: Vec<CheckResult>,
    /// True when every form evaluated and every check passed.
    pub ok: bool,
}

/// The result of checking one [`Expectation`] after a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckResult {
    /// Index of the checked setup form.
    pub form: usize,
    /// Expected encoded result.
    pub expected: String,
    /// Actual encoded result.
    pub actual: String,
    /// True when `expected == actual`.
    pub pass: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_card() -> RecipeCard {
        RecipeCard {
            id: "numbers-f64/01-basics/add-two-numbers".to_string(),
            book: "numbers-f64".to_string(),
            chapter: "01-basics".to_string(),
            chapter_title: "Basics".to_string(),
            chapter_summary: String::new(),
            title: "Add two numbers".to_string(),
            codec: "lisp".to_string(),
            setup: b"(+ 1 2)".to_vec(),
            purpose: "Add two f64 values.".to_string(),
            order: 100,
            chapter_order: 100,
            book_order: 200,
            book_title: "Numbers (f64)".to_string(),
            book_summary: String::new(),
            tags: vec!["arithmetic".to_string(), "intro".to_string()],
            requires: vec!["numbers-f64".to_string()],
            expect: vec![Expectation {
                form: 0,
                result: "3".to_string(),
            }],
            source: RecipeSource::Crate {
                lib: "numbers-f64".to_string(),
            },
        }
    }

    #[test]
    fn recipe_card_round_trips_its_fields() {
        let card = sample_card();
        let clone = card.clone();
        assert_eq!(card, clone);
        assert_eq!(card.id, "numbers-f64/01-basics/add-two-numbers");
        assert_eq!(card.setup, b"(+ 1 2)");
        assert_eq!(card.expect[0].form, 0);
        assert_eq!(card.expect[0].result, "3");
        assert_eq!(
            card.source,
            RecipeSource::Crate {
                lib: "numbers-f64".to_string()
            }
        );
    }

    #[test]
    fn cookbook_view_nests_book_chapter_recipe() {
        let card = sample_card();
        let view = CookbookView {
            books: vec![BookView {
                id: card.book.clone(),
                title: card.book_title.clone(),
                summary: String::new(),
                chapters: vec![ChapterView {
                    name: card.chapter.clone(),
                    title: card.chapter_title.clone(),
                    summary: String::new(),
                    recipes: vec![card.clone()],
                }],
            }],
        };
        assert_eq!(view.books[0].chapters[0].recipes[0], card);
        assert_eq!(view.books[0].id, "numbers-f64");
    }

    #[test]
    fn lib_view_carries_loaded_state_and_either_groups_or_recipes() {
        let card = sample_card();
        let loaded = LibView {
            id: card.book.clone(),
            title: card.book_title.clone(),
            loaded: true,
            groups: vec![ChapterView {
                name: card.chapter.clone(),
                title: card.chapter_title.clone(),
                summary: String::new(),
                recipes: vec![card.clone()],
            }],
            recipes: Vec::new(),
        };
        let unloaded = LibView {
            id: "numbers-i64".to_string(),
            title: "Numbers (i64)".to_string(),
            loaded: false,
            groups: Vec::new(),
            recipes: vec![card.clone()],
        };

        assert!(loaded.loaded);
        assert!(loaded.recipes.is_empty());
        assert_eq!(loaded.groups[0].recipes[0], card);
        assert!(!unloaded.loaded);
        assert!(unloaded.groups.is_empty());
        assert_eq!(unloaded.recipes.len(), 1);
    }

    #[test]
    fn recipe_run_reports_ok() {
        let run = RecipeRun {
            recipe: "numbers-f64/01-basics/add-two-numbers".to_string(),
            forms: 1,
            results: vec!["3".to_string()],
            checks: vec![CheckResult {
                form: 0,
                expected: "3".to_string(),
                actual: "3".to_string(),
                pass: true,
            }],
            ok: true,
        };
        assert!(run.ok);
        assert!(run.checks[0].pass);
    }
}
