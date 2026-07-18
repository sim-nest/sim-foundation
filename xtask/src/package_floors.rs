//! Internal dependency floor gate for publishable packages.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        let program = args.first().map(String::as_str).unwrap_or("xtask");
        return Err(format!("usage: {program} check-package-floors"));
    }

    let root = std::env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let mut packages = Vec::new();
    let mut by_root = BTreeMap::new();

    for member in workspace_members(&root.join("Cargo.toml"))? {
        let package = package_info(&root.join(member))?;
        let canonical_root = fs::canonicalize(&package.root)
            .map_err(|err| format!("canonicalize {}: {err}", package.root.display()))?;
        by_root.insert(canonical_root, packages.len());
        packages.push(package);
    }

    let mut problems = Vec::new();
    let mut checked = 0usize;
    for package in &packages {
        if !package.publish {
            continue;
        }
        for dependency in &package.dependencies {
            let dependency_root =
                fs::canonicalize(package.root.join(&dependency.path)).map_err(|err| {
                    format!(
                        "{}: canonicalize dependency path {}: {err}",
                        package.name,
                        dependency.path.display()
                    )
                })?;
            let Some(target_index) = by_root.get(&dependency_root) else {
                continue;
            };
            let target = &packages[*target_index];
            if !target.publish {
                problems.push(format!(
                    "{} {} depends on unpublished workspace package {} through {}",
                    package.name, dependency.section, target.name, dependency.name
                ));
                continue;
            }
            checked += 1;
            match dependency.version.as_deref() {
                Some(version) if version == target.version => {}
                Some(version) => problems.push(format!(
                    "{} {} dependency {} declares version {}, but local {} is {}",
                    package.name,
                    dependency.section,
                    dependency.name,
                    version,
                    target.name,
                    target.version
                )),
                None => problems.push(format!(
                    "{} {} dependency {} has a local path but no publishable version floor",
                    package.name, dependency.section, dependency.name
                )),
            }
        }
    }

    if !problems.is_empty() {
        for problem in problems {
            eprintln!("error: {problem}");
        }
        return Err("check-package-floors: internal dependency floor violations found".to_string());
    }

    println!(
        "check-package-floors: OK ({} package(s), {checked} internal path dependency floor(s))",
        packages.len()
    );
    Ok(())
}

#[derive(Debug)]
struct PackageInfo {
    name: String,
    version: String,
    root: PathBuf,
    publish: bool,
    dependencies: Vec<DependencyFloor>,
}

#[derive(Debug, PartialEq, Eq)]
struct DependencyFloor {
    name: String,
    section: String,
    version: Option<String>,
    path: PathBuf,
}

fn workspace_members(manifest: &Path) -> Result<Vec<PathBuf>, String> {
    let text = fs::read_to_string(manifest)
        .map_err(|err| format!("read {}: {err}", manifest.display()))?;
    let mut in_workspace = false;
    let mut collecting_members = false;
    let mut members = Vec::new();
    for raw in text.lines() {
        let stripped = strip_comment(raw);
        let line = stripped.trim();
        if line.starts_with('[') {
            in_workspace = line == "[workspace]";
            collecting_members = false;
        }
        if collecting_members {
            members.extend(quoted_strings(line).into_iter().map(PathBuf::from));
            if line.contains(']') {
                collecting_members = false;
            }
            continue;
        }
        if in_workspace
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "members"
        {
            members.extend(quoted_strings(value).into_iter().map(PathBuf::from));
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
    let mut section = String::new();
    let mut name = None;
    let mut version = None;
    let mut publish = true;
    let mut dependencies = Vec::new();

    for raw in text.lines() {
        let stripped = strip_comment(raw);
        let line = stripped.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = line.trim_matches(&['[', ']'][..]).to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match section.as_str() {
            "package" if key == "name" => name = string_literal(value),
            "package" if key == "version" => version = string_literal(value),
            "package" if key == "publish" => publish = value != "false",
            _ if is_dependency_section(&section) => {
                if let Some(dependency) = parse_dependency(key, value, &section) {
                    dependencies.push(dependency);
                }
            }
            _ => {}
        }
    }

    Ok(PackageInfo {
        name: name.ok_or_else(|| format!("{}: missing package name", manifest.display()))?,
        version: version
            .ok_or_else(|| format!("{}: missing package version", manifest.display()))?,
        root: root.to_path_buf(),
        publish,
        dependencies,
    })
}

fn is_dependency_section(section: &str) -> bool {
    matches!(
        section,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    ) || section.ends_with(".dependencies")
        || section.ends_with(".dev-dependencies")
        || section.ends_with(".build-dependencies")
}

fn parse_dependency(key: &str, value: &str, section: &str) -> Option<DependencyFloor> {
    let path = field_string(value, "path")?;
    let dependency_name = field_string(value, "package").unwrap_or_else(|| dependency_key(key));
    Some(DependencyFloor {
        name: dependency_name,
        section: section.to_string(),
        version: field_string(value, "version"),
        path: PathBuf::from(path),
    })
}

fn dependency_key(key: &str) -> String {
    string_literal(key).unwrap_or_else(|| key.trim().to_string())
}

fn field_string(value: &str, wanted: &str) -> Option<String> {
    let mut in_table = value.trim();
    if let Some(rest) = in_table.strip_prefix('{') {
        in_table = rest;
    }
    if let Some(rest) = in_table.strip_suffix('}') {
        in_table = rest;
    }
    for item in in_table.split(',') {
        let Some((key, value)) = item.split_once('=') else {
            continue;
        };
        if key.trim() == wanted {
            return string_literal(value.trim());
        }
    }
    None
}

fn string_literal(value: &str) -> Option<String> {
    quoted_strings(value).into_iter().next()
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

fn strip_comment(line: &str) -> String {
    let mut in_string = false;
    let mut escaped = false;
    let mut out = String::new();
    for ch in line.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            out.push(ch);
        } else if ch == '"' {
            in_string = true;
            out.push(ch);
        } else if ch == '#' {
            break;
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{DependencyFloor, field_string, is_dependency_section, parse_dependency};
    use std::path::PathBuf;

    #[test]
    fn dependency_sections_include_build_and_target_deps() {
        assert!(is_dependency_section("dependencies"));
        assert!(is_dependency_section("build-dependencies"));
        assert!(is_dependency_section("target.'cfg(unix)'.dependencies"));
        assert!(!is_dependency_section("package.metadata.sim-recipes"));
    }

    #[test]
    fn dependency_tables_expose_path_version_and_package_name() {
        let value = "{ package = \"sim-value\", version = \"0.1.2\", path = \"../sim-value\" }";
        assert_eq!(field_string(value, "version").as_deref(), Some("0.1.2"));
        assert_eq!(field_string(value, "path").as_deref(), Some("../sim-value"));
        assert_eq!(field_string(value, "package").as_deref(), Some("sim-value"));
    }

    #[test]
    fn path_dependency_parser_keeps_section_and_version() {
        let value = "{ version = \"0.1.3\", path = \"../sim-cookbook\" }";
        assert_eq!(
            parse_dependency("sim-cookbook", value, "build-dependencies"),
            Some(DependencyFloor {
                name: "sim-cookbook".to_string(),
                section: "build-dependencies".to_string(),
                version: Some("0.1.3".to_string()),
                path: PathBuf::from("../sim-cookbook"),
            })
        );
    }
}
