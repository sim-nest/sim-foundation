//! Project a [`RecipeStore`] into the cookbook view, plus search and
//! next-recipe navigation.
//!
//! The view is computed, never stored: group every loaded recipe by book then
//! chapter and sort deterministically. Loading another lib adds its book; the
//! view always reflects exactly the recipes currently loaded.

use crate::model::{BookView, ChapterView, CookbookView, RecipeCard};
use crate::store::RecipeStore;

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
    fn search_tag_match_beats_purpose_only() {
        let store = store();
        // "intro" is a tag on beta/hello (score 2) and not in alpha recipes.
        let hits = search(&store, "intro");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "beta/01-intro/hello");
    }
}
