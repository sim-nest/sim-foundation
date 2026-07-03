//! Turn an embedded `recipes/` tree into [`RecipeCard`]s.
//!
//! A crate ships its recipes as files under `recipes/`. A build step embeds
//! that tree into the compiled lib as an [`EmbeddedDir`] -- a flat list of
//! `(relative-path, bytes)` pairs, paths using `/` separators relative to
//! `recipes/`. [`recipes_from_embedded`] parses that list into cards, resolving
//! book/chapter metadata and reading each recipe's setup and purpose files.
//!
//! This keeps recipes traveling inside the crate they teach: nothing reads the
//! filesystem at runtime, so the recipes install with the lib.

use std::collections::BTreeSet;

use crate::manifest::{self, DEFAULT_ORDER};
use crate::model::{RecipeCard, RecipeSource};

/// A crate's embedded `recipes/` tree: `(path-relative-to-recipes, bytes)`.
/// Paths use `/` separators. Produced at build time (see
/// [`crate::generate_embed_code`]).
pub type EmbeddedDir = &'static [(&'static str, &'static [u8])];

fn find<'a>(dir: &'a [(&str, &'a [u8])], path: &str) -> Option<&'a [u8]> {
    dir.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
}

fn bytes_to_str<'a>(bytes: &'a [u8], what: &str) -> Result<&'a str, String> {
    std::str::from_utf8(bytes).map_err(|_| format!("{what} is not valid UTF-8"))
}

/// Turn `name` (a chapter directory like `01-basics`) into a human title by
/// dropping a leading numeric-and-dash prefix and capitalizing.
fn humanize(name: &str) -> String {
    let core = match name.split_once('-') {
        Some((head, tail)) if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) => tail,
        _ => name,
    };
    let spaced = core.replace('-', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => spaced,
    }
}

/// Parse an embedded `recipes/` tree into recipe cards (unsorted; the cookbook
/// view applies ordering). Returns an error string on the first malformed
/// manifest, missing file, or non-UTF-8 purpose document.
pub fn recipes_from_embedded(dir: &[(&str, &[u8])]) -> Result<Vec<RecipeCard>, String> {
    let book_bytes = find(dir, "book.toml").ok_or("missing book.toml")?;
    let book = manifest::parse_book(bytes_to_str(book_bytes, "book.toml")?)?;

    // Chapter order from the book's explicit `chapters` list: (idx+1)*100.
    let chapter_order_of = |chapter: &str| -> i64 {
        match book.chapters.iter().position(|c| c == chapter) {
            Some(idx) => (idx as i64 + 1) * 100,
            None => DEFAULT_ORDER,
        }
    };

    // Discover recipe dirs: any `<chapter>/<recipe-id>/recipe.toml`.
    let mut recipe_dirs: BTreeSet<(String, String)> = BTreeSet::new();
    for (path, _) in dir {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 3 && parts[2] == "recipe.toml" {
            recipe_dirs.insert((parts[0].to_string(), parts[1].to_string()));
        }
    }

    let mut cards = Vec::new();
    for (chapter, recipe_id) in recipe_dirs {
        let prefix = format!("{chapter}/{recipe_id}");
        let recipe_bytes = find(dir, &format!("{prefix}/recipe.toml"))
            .ok_or_else(|| format!("{prefix}: missing recipe.toml"))?;
        let recipe = manifest::parse_recipe(bytes_to_str(recipe_bytes, &prefix)?)
            .map_err(|e| format!("{prefix}/recipe.toml: {e}"))?;

        let chapter_manifest = match find(dir, &format!("{chapter}/chapter.toml")) {
            Some(bytes) => manifest::parse_chapter(bytes_to_str(bytes, "chapter.toml")?)
                .map_err(|e| format!("{chapter}/chapter.toml: {e}"))?,
            None => manifest::ChapterManifest::default(),
        };
        let chapter_title = chapter_manifest
            .title
            .clone()
            .unwrap_or_else(|| humanize(&chapter));
        let chapter_order = chapter_manifest
            .order
            .unwrap_or_else(|| chapter_order_of(&chapter));

        let setup = find(dir, &format!("{prefix}/{}", recipe.setup))
            .ok_or_else(|| format!("{prefix}: setup file `{}` not embedded", recipe.setup))?
            .to_vec();
        let purpose_bytes = find(dir, &format!("{prefix}/{}", recipe.purpose))
            .ok_or_else(|| format!("{prefix}: purpose file `{}` not embedded", recipe.purpose))?;
        let purpose = bytes_to_str(purpose_bytes, &format!("{prefix} purpose"))?.to_string();

        let requires = if recipe.requires.is_empty() {
            vec![book.book.clone()]
        } else {
            recipe.requires.clone()
        };

        cards.push(RecipeCard {
            id: format!("{}/{}/{}", book.book, chapter, recipe_id),
            book: book.book.clone(),
            chapter: chapter.clone(),
            chapter_title,
            chapter_summary: chapter_manifest.summary.clone(),
            title: recipe.title,
            codec: recipe.codec,
            setup,
            purpose,
            order: recipe.order,
            chapter_order,
            book_order: book.order,
            book_title: book.title.clone(),
            book_summary: book.summary.clone(),
            tags: recipe.tags,
            requires,
            expect: recipe.expect,
            source: RecipeSource::Crate {
                lib: book.book.clone(),
            },
        });
    }
    Ok(cards)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<(&'static str, &'static [u8])> {
        vec![
            (
                "book.toml",
                b"book = \"numbers-f64\"\ntitle = \"Numbers (f64)\"\norder = 200\nchapters = [\"01-basics\", \"02-rounding\"]\n" as &[u8],
            ),
            (
                "01-basics/add/recipe.toml",
                b"id = \"add\"\ntitle = \"Add\"\ncodec = \"lisp\"\nsetup = \"setup.siml\"\npurpose = \"purpose.md\"\norder = 100\n[[expect]]\nform = 0\nresult = \"3\"\n",
            ),
            ("01-basics/add/setup.siml", b"(+ 1 2)"),
            ("01-basics/add/purpose.md", b"Add two numbers."),
            (
                "02-rounding/round/recipe.toml",
                b"id = \"round\"\ntitle = \"Round\"\ncodec = \"lisp\"\nsetup = \"s.siml\"\npurpose = \"p.md\"\n",
            ),
            ("02-rounding/round/s.siml", b"(round 1.5)"),
            ("02-rounding/round/p.md", b"Round to even."),
        ]
    }

    #[test]
    fn parses_two_chapters() {
        let cards = recipes_from_embedded(&fixture()).unwrap();
        assert_eq!(cards.len(), 2);
        let add = cards.iter().find(|c| c.id.ends_with("/add")).unwrap();
        assert_eq!(add.id, "numbers-f64/01-basics/add");
        assert_eq!(add.book_title, "Numbers (f64)");
        assert_eq!(add.book_order, 200);
        assert_eq!(add.chapter_title, "Basics"); // humanized from 01-basics
        assert_eq!(add.chapter_order, 100); // first in book.chapters
        assert_eq!(add.setup, b"(+ 1 2)");
        assert_eq!(add.purpose, "Add two numbers.");
        assert_eq!(add.requires, ["numbers-f64"]); // defaulted to owning lib
        assert_eq!(add.expect[0].result, "3");

        let round = cards.iter().find(|c| c.id.ends_with("/round")).unwrap();
        assert_eq!(round.chapter_order, 200); // second in book.chapters
        assert_eq!(round.order, DEFAULT_ORDER); // omitted -> default
    }

    #[test]
    fn missing_book_toml_errors() {
        let err = recipes_from_embedded(&[("x/y/recipe.toml", b"")]).unwrap_err();
        assert!(err.contains("missing book.toml"), "{err}");
    }

    #[test]
    fn missing_setup_file_errors() {
        let dir: Vec<(&str, &[u8])> = vec![
            ("book.toml", b"book = \"b\"\ntitle = \"B\"\n"),
            (
                "c/r/recipe.toml",
                b"id = \"r\"\ntitle = \"R\"\ncodec = \"lisp\"\nsetup = \"setup.siml\"\npurpose = \"p.md\"\n",
            ),
            ("c/r/p.md", b"x"),
        ];
        let err = recipes_from_embedded(&dir).unwrap_err();
        assert!(
            err.contains("setup file `setup.siml` not embedded"),
            "{err}"
        );
    }

    #[test]
    fn humanize_strips_numeric_prefix() {
        assert_eq!(humanize("01-basics"), "Basics");
        assert_eq!(humanize("rounding"), "Rounding");
        assert_eq!(humanize("10-deep-dive"), "Deep dive");
    }
}
