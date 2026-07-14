//! Project a [`RecipeStore`] into the cookbook view, plus search and
//! next-recipe navigation.
//!
//! The view is computed, never stored: group every loaded recipe by book then
//! chapter and sort deterministically. Loading another lib adds its book; the
//! view always reflects exactly the recipes currently loaded.

use crate::model::{
    BookView, ChapterView, CookbookView, FamilyView, GroupedView, LibView, RecipeCard,
};
use crate::store::RecipeStore;

/// The level-1 family id for a book, derived from its id prefix with NO extra
/// metadata: the segment before the first `/` (`numbers/cas` -> `numbers`,
/// `organ/binding` -> `organ`, `codec/lisp` -> `codec`), else before the first
/// `-` (`audio-dsp` -> `audio`, `stream-audio` -> `stream`), else the whole id
/// (`agent` -> `agent`, `core` -> `core`). This is how the constellation groups
/// by subsystem without a hand-maintained family list.
pub fn family_of(book: &str) -> &str {
    match book.split_once('/') {
        Some((family, _)) => family,
        None => match book.split_once('-') {
            Some((family, _)) => family,
            None => book,
        },
    }
}

/// All cards in deterministic global order: by book order then id, chapter
/// order then name, recipe order then id. Two recipes never compare equal
/// because ids are unique, so the order is total and stable.
pub fn ordered_cards(store: &RecipeStore) -> Vec<&RecipeCard> {
    let mut cards: Vec<&RecipeCard> = store.cards().iter().collect();
    cards.sort_by(|a, b| {
        a.book_order
            .cmp(&b.book_order)
            .then_with(|| a.book.cmp(&b.book))
            .then_with(|| a.chapter_order.cmp(&b.chapter_order))
            .then_with(|| a.chapter.cmp(&b.chapter))
            .then_with(|| a.order.cmp(&b.order))
            .then_with(|| a.id.cmp(&b.id))
    });
    cards
}

/// Group the store's cards into the nested [`CookbookView`].
pub fn view(store: &RecipeStore) -> CookbookView {
    let mut books: Vec<BookView> = Vec::new();
    for card in ordered_cards(store) {
        let bi = match books.iter().position(|b| b.id == card.book) {
            Some(i) => i,
            None => {
                books.push(BookView {
                    id: card.book.clone(),
                    title: card.book_title.clone(),
                    summary: card.book_summary.clone(),
                    chapters: Vec::new(),
                });
                books.len() - 1
            }
        };
        let chapters = &mut books[bi].chapters;
        let ci = match chapters.iter().position(|c| c.name == card.chapter) {
            Some(i) => i,
            None => {
                chapters.push(ChapterView {
                    name: card.chapter.clone(),
                    title: card.chapter_title.clone(),
                    summary: card.chapter_summary.clone(),
                    recipes: Vec::new(),
                });
                chapters.len() - 1
            }
        };
        chapters[ci].recipes.push(card.clone());
    }
    CookbookView { books }
}

/// Group the projected store into one top-level entry per known loadable lib.
///
/// Loaded libs expose their recipe chapters as `groups`; unloaded libs expose
/// the single lifecycle load recipe that projected the lib into the store.
pub fn lib_view(store: &RecipeStore) -> Vec<LibView> {
    let mut libs: Vec<LibView> = Vec::new();
    for card in ordered_cards(store) {
        let target = lib_target(card);
        let index = match libs.iter().position(|lib| lib.id == target.id) {
            Some(index) => index,
            None => {
                libs.push(LibView {
                    id: target.id.clone(),
                    title: card.book_title.clone(),
                    loaded: target.loaded,
                    groups: Vec::new(),
                    recipes: Vec::new(),
                });
                libs.len() - 1
            }
        };
        let lib = &mut libs[index];
        lib.loaded |= target.loaded;
        if target.loaded {
            push_grouped_recipe(lib, card);
        } else {
            lib.recipes.push(card.clone());
        }
    }
    libs
}

struct LibTarget {
    id: String,
    loaded: bool,
}

fn lib_target(card: &RecipeCard) -> LibTarget {
    let action = tag_value(card, "cookbook-action:");
    let lib = tag_value(card, "cookbook-lib:");
    match (action, lib) {
        (Some("load"), Some(id)) => LibTarget {
            id: id.to_owned(),
            loaded: false,
        },
        (Some("unload"), Some(id)) => LibTarget {
            id: id.to_owned(),
            loaded: true,
        },
        _ => LibTarget {
            id: card.book.clone(),
            loaded: true,
        },
    }
}

fn tag_value<'a>(card: &'a RecipeCard, prefix: &str) -> Option<&'a str> {
    card.tags.iter().find_map(|tag| tag.strip_prefix(prefix))
}

fn push_grouped_recipe(lib: &mut LibView, card: &RecipeCard) {
    let index = match lib
        .groups
        .iter()
        .position(|group| group.name == card.chapter)
    {
        Some(index) => index,
        None => {
            lib.groups.push(ChapterView {
                name: card.chapter.clone(),
                title: card.chapter_title.clone(),
                summary: card.chapter_summary.clone(),
                recipes: Vec::new(),
            });
            lib.groups.len() - 1
        }
    };
    lib.groups[index].recipes.push(card.clone());
}

/// Group the store's cards into the two-level [`GroupedView`]: family -> domain
/// book -> chapter -> recipe. Books keep the deterministic [`view`] order, and a
/// family appears where its lowest-ordered book falls, so the whole catalog
/// presents by subsystem stably.
pub fn grouped_view(store: &RecipeStore) -> GroupedView {
    let mut families: Vec<FamilyView> = Vec::new();
    for book in view(store).books {
        let family = family_of(&book.id).to_string();
        match families.iter_mut().find(|f| f.family == family) {
            Some(f) => f.books.push(book),
            None => families.push(FamilyView {
                family,
                books: vec![book],
            }),
        }
    }
    // `view` already ordered books by book_order; the first book in each family
    // is therefore its lowest-ordered book, giving a stable family order.
    GroupedView { families }
}

/// Rank recipes matching `query` (case-insensitive). A title match scores
/// highest, then a tag match, then a purpose match; scores add. Recipes that
/// match nothing are dropped. Ties keep deterministic global order.
pub fn search<'a>(store: &'a RecipeStore, query: &str) -> Vec<&'a RecipeCard> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(i32, &RecipeCard)> = Vec::new();
    for card in ordered_cards(store) {
        let mut score = 0;
        if card.title.to_ascii_lowercase().contains(&q) {
            score += 3;
        }
        if card
            .tags
            .iter()
            .any(|t| t.to_ascii_lowercase().contains(&q))
        {
            score += 2;
        }
        if card.purpose.to_ascii_lowercase().contains(&q) {
            score += 1;
        }
        if score > 0 {
            scored.push((score, card));
        }
    }
    // Stable sort by descending score; `ordered_cards` already gave a
    // deterministic secondary order, preserved for equal scores.
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, card)| card).collect()
}

/// The recipe immediately after `id` in global order, for "continue" buttons.
/// `None` if `id` is unknown or is the last recipe.
pub fn next<'a>(store: &'a RecipeStore, id: &str) -> Option<&'a RecipeCard> {
    let ordered = ordered_cards(store);
    let pos = ordered.iter().position(|c| c.id == id)?;
    ordered.get(pos + 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two books, deliberately registered out of final order to prove sorting.
    fn store() -> RecipeStore {
        let beta: Vec<(&str, &[u8])> = vec![
            (
                "book.toml",
                b"book = \"beta\"\ntitle = \"Beta\"\nsummary = \"Second book.\"\norder = 300\n" as &[u8],
            ),
            (
                "01-intro/hello/recipe.toml",
                b"id = \"hello\"\ntitle = \"Hello\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"purpose\"\ntags = [\"intro\"]\n",
            ),
            ("01-intro/hello/s", b"(quote hi)"),
            ("01-intro/hello/purpose", b"a greeting recipe"),
        ];
        let alpha: Vec<(&str, &[u8])> = vec![
            (
                "book.toml",
                b"book = \"alpha\"\ntitle = \"Alpha\"\norder = 100\nchapters = [\"01-basics\"]\n",
            ),
            (
                "01-basics/add/recipe.toml",
                b"id = \"add\"\ntitle = \"Add\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\norder = 100\ntags = [\"arithmetic\"]\n",
            ),
            ("01-basics/add/s", b"(+ 1 2)"),
            ("01-basics/add/p", b"add numbers"),
            (
                "01-basics/sub/recipe.toml",
                b"id = \"sub\"\ntitle = \"Subtract\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\norder = 200\n",
            ),
            ("01-basics/sub/s", b"(- 3 1)"),
            ("01-basics/sub/p", b"subtract numbers"),
        ];
        let mut store = RecipeStore::new();
        store.register_book(&beta).unwrap();
        store.register_book(&alpha).unwrap();
        store
    }

    #[test]
    fn view_orders_books_chapters_recipes() {
        let view = view(&store());
        // alpha (order 100) before beta (order 300) despite registration order.
        assert_eq!(view.books.len(), 2);
        assert_eq!(view.books[0].id, "alpha");
        assert_eq!(view.books[1].id, "beta");
        assert_eq!(view.books[1].summary, "Second book.");
        let basics = &view.books[0].chapters[0];
        assert_eq!(basics.name, "01-basics");
        // add (order 100) before sub (order 200).
        assert_eq!(basics.recipes[0].id, "alpha/01-basics/add");
        assert_eq!(basics.recipes[1].id, "alpha/01-basics/sub");
    }

    #[test]
    fn next_walks_global_order() {
        let store = store();
        assert_eq!(
            next(&store, "alpha/01-basics/add").unwrap().id,
            "alpha/01-basics/sub"
        );
        assert_eq!(
            next(&store, "alpha/01-basics/sub").unwrap().id,
            "beta/01-intro/hello"
        );
        assert!(next(&store, "beta/01-intro/hello").is_none()); // last
        assert!(next(&store, "nope").is_none());
    }

    #[test]
    fn search_ranks_title_over_purpose() {
        let store = store();
        // "add" matches the title of add (3) and nothing else strongly.
        let hits = search(&store, "add");
        assert_eq!(hits[0].id, "alpha/01-basics/add");
        // "numbers" only appears in purposes -> score 1, both basics recipes.
        let hits = search(&store, "numbers");
        assert_eq!(hits.len(), 2);
        // empty query returns nothing.
        assert!(search(&store, "  ").is_empty());
    }

    #[test]
    fn family_of_derives_from_prefix() {
        assert_eq!(family_of("numbers/cas"), "numbers");
        assert_eq!(family_of("organ/binding"), "organ");
        assert_eq!(family_of("codec/lisp"), "codec");
        assert_eq!(family_of("audio-dsp"), "audio");
        assert_eq!(family_of("stream-audio"), "stream");
        assert_eq!(family_of("agent-runner-core"), "agent");
        assert_eq!(family_of("agent"), "agent");
        assert_eq!(family_of("core"), "core");
    }

    fn lifecycle_card(
        id: &str,
        lib: &str,
        title: &str,
        action: &str,
        book_order: i64,
    ) -> RecipeCard {
        RecipeCard {
            id: id.to_string(),
            book: if action == "load" {
                "cookbook/loadable".to_string()
            } else {
                lib.to_string()
            },
            chapter: "cookbook-lifecycle".to_string(),
            chapter_title: "Lifecycle".to_string(),
            chapter_summary: String::new(),
            title: title.to_string(),
            codec: "lisp".to_string(),
            setup: b"(quote ok)".to_vec(),
            purpose: title.to_string(),
            order: if action == "load" { 0 } else { i64::MAX },
            chapter_order: if action == "load" { 0 } else { i64::MAX },
            book_order,
            book_title: title.to_string(),
            book_summary: String::new(),
            tags: vec![
                format!("cookbook-action:{action}"),
                format!("cookbook-lib:{lib}"),
            ],
            requires: Vec::new(),
            expect: Vec::new(),
            source: crate::RecipeSource::Crate {
                lib: "sim/cookbook".to_string(),
            },
        }
    }

    #[test]
    fn lib_view_uses_top_level_lib_entries_for_loaded_and_unloaded_libs() {
        let mut store = RecipeStore::new();
        store
            .insert_card(lifecycle_card(
                "cookbook/load/numbers/i64",
                "numbers/i64",
                "Numbers (i64)",
                "load",
                50,
            ))
            .unwrap();
        for card in ordered_cards(&family_store())
            .into_iter()
            .filter(|card| card.book == "codec/lisp")
        {
            store.insert_card(card.clone()).unwrap();
        }
        store
            .insert_card(lifecycle_card(
                "codec/lisp/cookbook-lifecycle/unload",
                "codec/lisp",
                "Lisp",
                "unload",
                200,
            ))
            .unwrap();

        let libs = lib_view(&store);

        assert_eq!(libs.len(), 2);
        assert_eq!(libs[0].id, "numbers/i64");
        assert!(!libs[0].loaded);
        assert!(libs[0].groups.is_empty());
        assert_eq!(libs[0].recipes.len(), 1);
        assert_eq!(libs[0].recipes[0].id, "cookbook/load/numbers/i64");
        assert_eq!(libs[1].id, "codec/lisp");
        assert!(libs[1].loaded);
        assert!(libs[1].recipes.is_empty());
        assert_eq!(
            libs[1].groups[0].recipes[0].id,
            "codec/lisp/01-basics/quote"
        );
        assert_eq!(
            libs[1].groups.last().unwrap().recipes[0].id,
            "codec/lisp/cookbook-lifecycle/unload"
        );
    }

    // Two books under the same family plus one under another, to prove grouping.
    fn family_store() -> RecipeStore {
        let cas: Vec<(&str, &[u8])> = vec![
            (
                "book.toml",
                b"book = \"numbers/cas\"\ntitle = \"CAS\"\norder = 210\n" as &[u8],
            ),
            (
                "01-basics/simplify/recipe.toml",
                b"id = \"simplify\"\ntitle = \"Simplify\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\n",
            ),
            ("01-basics/simplify/s", b"(quote x)"),
            ("01-basics/simplify/p", b"simplify"),
        ];
        let f64: Vec<(&str, &[u8])> = vec![
            (
                "book.toml",
                b"book = \"numbers/f64\"\ntitle = \"F64\"\norder = 200\n" as &[u8],
            ),
            (
                "01-basics/add/recipe.toml",
                b"id = \"add\"\ntitle = \"Add\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\n",
            ),
            ("01-basics/add/s", b"(+ 1 2)"),
            ("01-basics/add/p", b"add"),
        ];
        let lisp: Vec<(&str, &[u8])> = vec![
            (
                "book.toml",
                b"book = \"codec/lisp\"\ntitle = \"Lisp\"\norder = 100\n" as &[u8],
            ),
            (
                "01-basics/quote/recipe.toml",
                b"id = \"quote\"\ntitle = \"Quote\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\n",
            ),
            ("01-basics/quote/s", b"(quote a)"),
            ("01-basics/quote/p", b"quote"),
        ];
        let mut store = RecipeStore::new();
        store.register_book(&cas).unwrap();
        store.register_book(&f64).unwrap();
        store.register_book(&lisp).unwrap();
        store
    }

    #[test]
    fn grouped_view_nests_family_domain_book() {
        let grouped = grouped_view(&family_store());
        // codec (book_order 100) sorts before numbers (lowest book_order 200).
        assert_eq!(grouped.families.len(), 2);
        assert_eq!(grouped.families[0].family, "codec");
        assert_eq!(grouped.families[0].books.len(), 1);
        assert_eq!(grouped.families[0].books[0].id, "codec/lisp");
        // numbers holds both f64 (200) and cas (210), f64 first by book_order.
        let numbers = &grouped.families[1];
        assert_eq!(numbers.family, "numbers");
        let ids: Vec<&str> = numbers.books.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["numbers/f64", "numbers/cas"]);
        // Leaves reach the recipes intact.
        assert_eq!(
            numbers.books[0].chapters[0].recipes[0].id,
            "numbers/f64/01-basics/add"
        );
    }

    #[test]
    fn search_tag_match_beats_purpose_only() {
        let store = store();
        // "intro" is a tag on beta/hello (score 2) and not in alpha recipes.
        let hits = search(&store, "intro");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "beta/01-intro/hello");
    }
}
