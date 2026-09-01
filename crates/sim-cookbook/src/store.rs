//! The in-memory store of loaded recipes.
//!
//! Each loaded lib registers its embedded book into a [`RecipeStore`]; the
//! cookbook view is a projection over the store's cards. The
//! store lives in this crate, not the kernel: recipes are plain data, so no
//! kernel cookbook type is needed. A server or CLI builds one store at startup
//! by registering every loaded lib's book.

use crate::embed::recipes_from_embedded;
use crate::model::RecipeCard;

/// A registry of loaded recipe cards, keyed by their globally unique id.
#[derive(Clone, Debug, Default)]
pub struct RecipeStore {
    cards: Vec<RecipeCard>,
}

impl RecipeStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse one crate's embedded `recipes/` tree and add its cards. Returns an
    /// error if a manifest is malformed or a recipe id collides with one
    /// already registered (ids must be globally unique).
    pub fn register_book(&mut self, dir: &[(&str, &[u8])]) -> Result<(), String> {
        let cards = recipes_from_embedded(dir)?;
        for card in &cards {
            if self.cards.iter().any(|existing| existing.id == card.id) {
                return Err(format!("duplicate recipe id `{}`", card.id));
            }
        }
        self.cards.extend(cards);
        Ok(())
    }

    /// Add a single already-built card. Errors on a duplicate id.
    pub fn insert_card(&mut self, card: RecipeCard) -> Result<(), String> {
        if self.cards.iter().any(|existing| existing.id == card.id) {
            return Err(format!("duplicate recipe id `{}`", card.id));
        }
        self.cards.push(card);
        Ok(())
    }

    /// Insert a card, replacing any existing card with the same id (overlay
    /// override semantics). Returns whether an existing card was replaced.
    pub fn upsert_card(&mut self, card: RecipeCard) -> bool {
        match self
            .cards
            .iter_mut()
            .find(|existing| existing.id == card.id)
        {
            Some(existing) => {
                *existing = card;
                true
            }
            None => {
                self.cards.push(card);
                false
            }
        }
    }

    /// Remove a card by id. Returns whether one was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.cards.len();
        self.cards.retain(|c| c.id != id);
        self.cards.len() != before
    }

    /// Mutable access to one card by id (for overlay reorder/retitle).
    pub fn card_mut(&mut self, id: &str) -> Option<&mut RecipeCard> {
        self.cards.iter_mut().find(|c| c.id == id)
    }

    /// All registered cards, in registration order.
    pub fn cards(&self) -> &[RecipeCard] {
        &self.cards
    }

    /// Look up one card by its full id.
    pub fn card(&self, id: &str) -> Option<&RecipeCard> {
        self.cards.iter().find(|c| c.id == id)
    }

    /// Number of registered cards.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Whether the store holds no cards.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(id: &'static str) -> Vec<(&'static str, &'static [u8])> {
        // One tiny book whose lib id is `id`. We leak `id` into the embedded
        // strings via a fixed template per test input.
        match id {
            "alpha" => vec![
                ("book.toml", b"book = \"alpha\"\ntitle = \"Alpha\"\n" as &[u8]),
                (
                    "c/r/recipe.toml",
                    b"id = \"r\"\ntitle = \"R\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\n",
                ),
                ("c/r/s", b"(quote a)"),
                ("c/r/p", b"alpha recipe"),
            ],
            _ => vec![
                ("book.toml", b"book = \"beta\"\ntitle = \"Beta\"\n" as &[u8]),
                (
                    "c/r/recipe.toml",
                    b"id = \"r\"\ntitle = \"R\"\ncodec = \"lisp\"\nsetup = \"s\"\npurpose = \"p\"\n",
                ),
                ("c/r/s", b"(quote b)"),
                ("c/r/p", b"beta recipe"),
            ],
        }
    }

    #[test]
    fn registers_multiple_books() {
        let mut store = RecipeStore::new();
        store.register_book(&book("alpha")).unwrap();
        store.register_book(&book("beta")).unwrap();
        assert_eq!(store.len(), 2);
        assert!(store.card("alpha/c/r").is_some());
        assert!(store.card("beta/c/r").is_some());
    }

    #[test]
    fn rejects_duplicate_book() {
        let mut store = RecipeStore::new();
        store.register_book(&book("alpha")).unwrap();
        let err = store.register_book(&book("alpha")).unwrap_err();
        assert!(err.contains("duplicate recipe id `alpha/c/r`"), "{err}");
    }
}
