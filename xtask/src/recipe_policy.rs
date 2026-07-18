//! Recipe policy gate for the sim-foundation workspace.

use std::fs;
use std::path::{Path, PathBuf};

const DESCRIPTOR_TAG: &str = "sandbox-descriptor";

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        let program = args.first().map(String::as_str).unwrap_or("xtask");
        return Err(format!("usage: {program} check-recipes"));
    }

    let root = std::env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let members = workspace_members(&root.join("Cargo.toml"))?;
    let mut problems = Vec::new();
    let mut publishable = 0usize;
    let mut recipe_books = 0usize;
    let mut recipe_count = 0usize;
    let mut exceptions = 0usize;

    for member in members {
        let package_root = root.join(&member);
        let package = package_info(&package_root)?;
        if !package.publish {
            continue;
        }
        publishable += 1;
        let recipes = recipe_files(&package_root.join("recipes"))?;
        recipe_count += recipes.len();
        if !recipes.is_empty() {
            recipe_books += 1;
        }

        let Some(policy) = &package.policy else {
            problems.push(format!(
                "{}: missing [package.metadata.sim-recipes]",
                package.manifest.display()
            ));
            continue;
        };
        if policy.reason.trim().is_empty() {
            problems.push(format!(
                "{}: package.metadata.sim-recipes.reason must not be empty",
                package.manifest.display()
            ));
        }

        match policy.kind.as_str() {
            "descriptor" => check_descriptor_package(&package, &recipes, &mut problems)?,
            "runtime" => {
                if recipes.is_empty() {
                    problems.push(format!(
                        "{}: runtime recipe policy requires a recipes/ directory",
                        package.name
                    ));
                }
            }
            "engine" | "rustdoc" => {
                exceptions += 1;
                if !recipes.is_empty() {
                    problems.push(format!(
                        "{}: {} recipe policy must not ship a recipes/ directory",
                        package.name, policy.kind
                    ));
                }
                check_readme_documents_exception(&package, &policy.kind, &mut problems)?;
            }
            other => problems.push(format!(
                "{}: unsupported recipe policy `{}`",
                package.name, other
            )),
        }
    }

    if !problems.is_empty() {
        for problem in problems {
            eprintln!("error: {problem}");
        }
        return Err("check-recipes: recipe policy violations found".to_string());
    }

    println!(
        "check-recipes: OK ({publishable} publishable package(s), {recipe_books} recipe book(s), {recipe_count} recipe(s), {exceptions} policy exception(s))"
    );
    Ok(())
}

fn check_descriptor_package(
    package: &PackageInfo,
    recipes: &[PathBuf],
    problems: &mut Vec<String>,
) -> Result<(), String> {
    if recipes.is_empty() {
        problems.push(format!(
            "{}: descriptor recipe policy requires a recipes/ directory",
            package.name
        ));
        return Ok(());
    }

    let readme = fs::read_to_string(package.root.join("README.md"))
        .map_err(|err| format!("read {} README: {err}", package.name))?;
    let readme_lower = readme.to_ascii_lowercase();
    if readme_lower.contains("runnable recipes") {
        problems.push(format!(
            "{} README.md: descriptor recipes must not be advertised as runnable",
            package.name
        ));
    }

    for recipe in recipes {
        let text = fs::read_to_string(recipe)
            .map_err(|err| format!("read {}: {err}", recipe.display()))?;
        if !array_contains(&text, "tags", DESCRIPTOR_TAG) {
            problems.push(format!(
                "{}: descriptor recipe must include `{}` in tags",
                recipe.display(),
                DESCRIPTOR_TAG
            ));
        }
        if contains_runnable_claim(&text) {
            problems.push(format!(
                "{}: descriptor recipe manifest must not claim runnable execution",
                recipe.display()
            ));
        }
        let Some(purpose) = string_value(&text, "purpose") else {
            problems.push(format!("{}: missing purpose path", recipe.display()));
            continue;
        };
        let Some(dir) = recipe.parent() else {
            continue;
        };
        let purpose_path = dir.join(purpose);
        let purpose_text = fs::read_to_string(&purpose_path)
            .map_err(|err| format!("read {}: {err}", purpose_path.display()))?;
        let purpose_lower = purpose_text.to_ascii_lowercase();
        if !purpose_lower.contains("descriptor") {
            problems.push(format!(
                "{}: descriptor recipe purpose must state the descriptor contract",
                purpose_path.display()
            ));
        }
        if contains_runnable_claim(&purpose_text) {
            problems.push(format!(
                "{}: descriptor recipe purpose must not claim runnable execution",
                purpose_path.display()
            ));
        }
    }
    Ok(())
}

fn check_readme_documents_exception(
    package: &PackageInfo,
    policy: &str,
    problems: &mut Vec<String>,
) -> Result<(), String> {
    let readme_path = package.root.join("README.md");
    let readme = fs::read_to_string(&readme_path)
        .map_err(|err| format!("read {}: {err}", readme_path.display()))?;
    let lower = readme.to_ascii_lowercase();
    let documented = match policy {
        "engine" => lower.contains("recipe engine itself") && lower.contains("recipe directory"),
        "rustdoc" => lower.contains("rustdoc") && lower.contains("no recipe"),
        _ => false,
    };
    if !documented {
        problems.push(format!(
            "{}: README.md must document the `{policy}` recipe-policy exception",
            package.name
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct PackageInfo {
    name: String,
    root: PathBuf,
    manifest: PathBuf,
    publish: bool,
    policy: Option<RecipePolicy>,
}

#[derive(Debug)]
struct RecipePolicy {
    kind: String,
    reason: String,
}

fn workspace_members(manifest: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(manifest)
        .map_err(|err| format!("read {}: {err}", manifest.display()))?;
    let mut in_workspace = false;
    let mut collecting_members = false;
    let mut members = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_workspace = line == "[workspace]";
            collecting_members = false;
        }
        if collecting_members {
            members.extend(quoted_strings(line));
            if line.contains(']') {
                collecting_members = false;
            }
            continue;
        }
        if in_workspace
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "members"
        {
            members.extend(quoted_strings(value));
            if !value.contains(']') {
                collecting_members = true;
            }
        }
    }
    if members.is_empty() {
        return Err("workspace has no members".to_string());
    }
    Ok(members)
}

fn package_info(root: &Path) -> Result<PackageInfo, String> {
    let manifest = root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .map_err(|err| format!("read {}: {err}", manifest.display()))?;
    let mut section = "";
    let mut name = None;
    let mut publish = true;
    let mut policy_kind = None;
    let mut policy_reason = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            section = line;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match section {
            "[package]" if key == "name" => name = string_literal(value),
            "[package]" if key == "publish" => publish = value == "true",
            "[package.metadata.sim-recipes]" if key == "policy" => {
                policy_kind = string_literal(value)
            }
            "[package.metadata.sim-recipes]" if key == "reason" => {
                policy_reason = string_literal(value)
            }
            _ => {}
        }
    }

    let name = name.ok_or_else(|| format!("{}: missing package name", manifest.display()))?;
    let policy = policy_kind.map(|kind| RecipePolicy {
        kind,
        reason: policy_reason.unwrap_or_default(),
    });
    Ok(PackageInfo {
        name,
        root: root.to_path_buf(),
        manifest,
        publish,
        policy,
    })
}

fn recipe_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    if root.is_dir() {
        collect_recipe_files(root, &mut out)?;
    }
    out.sort();
    Ok(out)
}

fn collect_recipe_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|err| format!("read {}: {err}", dir.display()))? {
        let entry = entry.map_err(|err| format!("read {} entry: {err}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("read {} file type: {err}", path.display()))?;
        if file_type.is_dir() {
            collect_recipe_files(&path, out)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("recipe.toml") {
            out.push(path);
        }
    }
    Ok(())
}

fn string_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some((candidate, value)) = line.split_once('=') else {
            continue;
        };
        if candidate.trim() == key {
            return string_literal(value.trim());
        }
    }
    None
}

fn array_contains(text: &str, key: &str, expected: &str) -> bool {
    for line in text.lines() {
        let line = line.trim();
        let Some((candidate, value)) = line.split_once('=') else {
            continue;
        };
        if candidate.trim() == key {
            return quoted_strings(value).iter().any(|item| item == expected);
        }
    }
    false
}

fn contains_runnable_claim(text: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
        .any(|word| {
            matches!(
                word.to_ascii_lowercase().as_str(),
                "runnable" | "executable"
            )
        })
}

fn string_literal(value: &str) -> Option<String> {
    let strings = quoted_strings(value);
    strings.into_iter().next()
}

fn quoted_strings(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in value.chars() {
        if in_string {
            if escaped {
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                out.push(std::mem::take(&mut current));
                in_string = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_string = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{array_contains, contains_runnable_claim, quoted_strings};

    #[test]
    fn quoted_strings_reads_arrays() {
        assert_eq!(
            quoted_strings("[\"net\", \"sandbox-descriptor\"]"),
            ["net", "sandbox-descriptor"]
        );
    }

    #[test]
    fn array_contains_checks_named_array() {
        let text = "tags = [\"surface-card\", \"sandbox-descriptor\"]\n";
        assert!(array_contains(text, "tags", "sandbox-descriptor"));
        assert!(!array_contains(text, "tags", "runtime"));
    }

    #[test]
    fn runnable_claim_detection_uses_words() {
        assert!(contains_runnable_claim("ship runnable recipes"));
        assert!(contains_runnable_claim("an executable lesson"));
        assert!(!contains_runnable_claim(
            "not a sandbox-evaluable expression"
        ));
    }
}
