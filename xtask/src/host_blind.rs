//! Structural gate that keeps sim-host-core free of host realization calls.

use std::{fs, path::Path};

const FORBIDDEN: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::env::var",
    "std::thread",
    "tokio::net",
    "libc::",
    "windows_sys::",
];

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        let program = args.first().map(String::as_str).unwrap_or("xtask");
        return Err(format!("usage: {program} check-host-blind"));
    }
    let root = std::env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let source = root.join("crates/sim-host-core/src");
    let mut problems = Vec::new();
    inspect_tree(&source, &mut problems)?;
    if !problems.is_empty() {
        return Err(format!(
            "check-host-blind: concrete host access is forbidden:\n{}",
            problems.join("\n")
        ));
    }
    println!("check-host-blind: OK ({})", source.display());
    Ok(())
}

fn inspect_tree(path: &Path, problems: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|err| format!("read {}: {err}", path.display()))? {
        let path = entry.map_err(|err| format!("read entry: {err}"))?.path();
        if path.is_dir() {
            inspect_tree(&path, problems)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("read {}: {err}", path.display()))?;
            inspect_source(&path, &text, problems);
        }
    }
    Ok(())
}

fn inspect_source(path: &Path, source: &str, problems: &mut Vec<String>) {
    for (line_index, line) in source.lines().enumerate() {
        for forbidden in FORBIDDEN {
            if line.contains(forbidden) {
                problems.push(format!(
                    "{}:{} contains {forbidden}",
                    path.display(),
                    line_index + 1
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_call_fails_the_structural_gate() {
        let mut problems = Vec::new();
        let forbidden_call = ["std", "::", "fs", "::read(\"secret\")"].concat();
        inspect_source(Path::new("fictional.rs"), &forbidden_call, &mut problems);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn sim_host_core_remains_host_blind() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/sim-host-core/src");
        let mut problems = Vec::new();
        inspect_tree(&source, &mut problems).expect("inspect sim-host-core");
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
