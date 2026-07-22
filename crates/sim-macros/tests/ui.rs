use std::path::{Path, PathBuf};
use std::process::Command;

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

fn unique_target_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("sim-macros-ui-{nanos}"))
}

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/ui")
        .join(name)
}

const UI_PATCHES: &[(&str, &str, &str)] = &[
    ("sim-nest", "sim-sdk", "."),
    ("sim-kernel", "sim-kernel", "."),
    ("sim-value", "sim-foundation", "crates/sim-value"),
    ("sim-lib-core", "sim-runtime", "crates/sim-lib-core"),
    ("sim-macros", "sim-foundation", "crates/sim-macros"),
    ("sim-run-loaders", "sim-run", "crates/sim-run-loaders"),
    ("sim-shape", "sim-shape", "."),
    ("sim-codec", "sim-codecs", "crates/sim-codec"),
    ("sim-codec-binary", "sim-codecs", "crates/sim-codec-binary"),
];

fn local_patch_path(crate_name: &str, repo_name: &str, source_path: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "packages")
    {
        return manifest_dir
            .parent()
            .expect("meta-workspace package should have a packages parent")
            .join(crate_name);
    }

    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("sim-macros should live under crates/<name>");
    if repo_name == "sim-foundation" {
        return repo_root.join(source_path);
    }
    repo_root
        .parent()
        .expect("sim-foundation checkout should have sibling repos")
        .join(repo_name)
        .join(source_path)
}

fn toml_string(path: &Path) -> String {
    let raw = path.to_string_lossy();
    format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
}

fn add_ui_patch_args(command: &mut Command) {
    for (crate_name, repo_name, source_path) in UI_PATCHES {
        let path = local_patch_path(crate_name, repo_name, source_path);
        if !path.join("Cargo.toml").exists() {
            continue;
        }
        command.arg("--config").arg(format!(
            "patch.crates-io.{crate_name}.path={}",
            toml_string(&path)
        ));
    }
}

fn run_fixture(
    cargo_args: &[&str],
    manifest_path: PathBuf,
    target_dir: &Path,
) -> std::process::Output {
    let mut command = Command::new(cargo_bin());
    command.args(cargo_args);
    command
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--target-dir")
        .arg(target_dir);
    add_ui_patch_args(&mut command);
    command
        .output()
        .expect("cargo command for proc-macro UI fixture should start")
}

fn stderr_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn remove_dir_all_if_exists(path: &Path) {
    if path.exists() {
        let _ = std::fs::remove_dir_all(path);
    }
}

fn assert_fixture_checks(name: &str) {
    let target_dir = unique_target_dir();
    let output = run_fixture(
        &["check"],
        fixture_dir(name).join("Cargo.toml"),
        &target_dir,
    );
    assert!(
        output.status.success(),
        "{name} fixture failed to compile:\n{}",
        stderr_text(&output)
    );
    remove_dir_all_if_exists(&target_dir);
}

fn assert_fixture_fails(name: &str, needles: &[&str]) {
    let target_dir = unique_target_dir();
    let output = run_fixture(
        &["check"],
        fixture_dir(name).join("Cargo.toml"),
        &target_dir,
    );
    assert!(
        !output.status.success(),
        "{name} fixture unexpectedly compiled"
    );
    let stderr = stderr_text(&output);
    for needle in needles {
        assert!(
            stderr.contains(needle),
            "expected `{needle}` in stderr for {name}, got:\n{stderr}"
        );
    }
    remove_dir_all_if_exists(&target_dir);
}

#[test]
fn compile_pass_fixtures_build_cleanly() {
    assert_fixture_checks("pass/basic-lib");
    assert_fixture_checks("pass/marker-surface");
}

#[test]
fn missing_required_sim_lib_keys_fail_compilation() {
    assert_fixture_fails(
        "fail/missing-lib-id",
        &["expected #[sim_lib(id = \"...\")]"],
    );
}

#[test]
fn duplicate_keys_fail_compilation() {
    assert_fixture_fails(
        "fail/duplicate-lib-id",
        &["duplicate #[sim_lib(id = ...)] entry"],
    );
    assert_fixture_fails(
        "fail/duplicate-marker-key",
        &["duplicate symbol entry in marker attribute"],
    );
}

#[test]
fn missing_marker_entries_fail_compilation() {
    assert_fixture_fails(
        "fail/missing-site-realize",
        &["#[sim_site] requires realize"],
    );
}

#[test]
fn invalid_shape_literals_fail_compilation() {
    let target_dir = unique_target_dir();
    let output = run_fixture(
        &["check"],
        fixture_dir("fail/invalid-shape").join("Cargo.toml"),
        &target_dir,
    );
    assert!(
        !output.status.success(),
        "fail/invalid-shape fixture unexpectedly compiled"
    );
    let stderr = stderr_text(&output);
    assert!(
        stderr.contains("unterminated list") || stderr.contains("unexpected end of input"),
        "expected a shape parse failure, got:\n{stderr}"
    );
    remove_dir_all_if_exists(&target_dir);
}

#[test]
fn unsupported_native_export_signatures_fail_compilation() {
    assert_fixture_fails(
        "fail/bad-native-export-signature",
        &["native_export = true only supports f64, bool, String, Symbol, and Expr arguments"],
    );
}

#[test]
fn non_inline_modules_fail_compilation() {
    assert_fixture_fails(
        "fail/non-inline-module",
        &["#[sim_lib] requires an inline module"],
    );
}

#[test]
fn consumer_smoke_fixture_runs() {
    let target_dir = unique_target_dir();
    let output = run_fixture(
        &["test"],
        fixture_dir("smoke/consumer").join("Cargo.toml"),
        &target_dir,
    );
    assert!(
        output.status.success(),
        "consumer smoke fixture failed:\n{}",
        stderr_text(&output)
    );
    remove_dir_all_if_exists(&target_dir);
}
