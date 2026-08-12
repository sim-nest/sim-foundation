//! Compatibility normalization for cookbook manifests shipped by older crates.
//!
//! The canonical authoring parser remains strict. This module exists only at
//! the embedded/runtime boundary, where a compatible cookbook update must keep
//! already-published recipe trees loadable.

use std::path::Path;

use crate::manifest::{self, RecipeDefaults, RecipeManifest};
use crate::toml_lite::{self, TomlDoc};

pub(crate) fn parse_embedded_recipe(
    text: &str,
    recipe_dir_id: &str,
    conventional_setups: &[&str],
) -> Result<RecipeManifest, String> {
    let doc = toml_lite::parse(text)?;
    let setup = match optional_string(&doc, "setup")? {
        Some(setup) => setup,
        None => match conventional_setups {
            [setup] => setup,
            [] => return Err("missing required key `setup`".to_string()),
            setups => {
                return Err(format!(
                    "legacy manifest has ambiguous conventional setup files: {}",
                    setups.join(", ")
                ));
            }
        },
    };
    let codec = match optional_string(&doc, "codec")? {
        Some(codec) => codec.to_string(),
        None => infer_codec(&doc, setup)?,
    };
    manifest::parse_recipe_doc(
        &doc,
        Some(RecipeDefaults {
            id: recipe_dir_id,
            codec: &codec,
            setup,
        }),
    )
}

fn optional_string<'a>(doc: &'a TomlDoc, key: &str) -> Result<Option<&'a str>, String> {
    doc.get(key)
        .map(|value| value.as_str().map_err(|error| format!("`{key}`: {error}")))
        .transpose()
}

fn string_or<'a>(
    doc: &'a TomlDoc,
    key: &str,
    fallback: Option<&'a str>,
) -> Result<&'a str, String> {
    optional_string(doc, key)?
        .or(fallback)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing required key `{key}`"))
}

fn infer_codec(doc: &TomlDoc, setup: &str) -> Result<String, String> {
    let known_extension = Path::new(setup)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| match extension {
            "rs" => Some("rust"),
            "siml" => Some("lisp"),
            "json" | "lua" => Some(extension),
            "py" => Some("python"),
            "js" => Some("javascript"),
            "ts" => Some("typescript"),
            _ => None,
        });
    if let Some(codec) = known_extension {
        return Ok(codec.to_string());
    }

    let category = string_or(doc, "category", None)?;
    Ok(category.trim().to_ascii_lowercase().replace(' ', "-"))
}
