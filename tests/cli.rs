use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn read_file(path: &std::path::Path) -> String {
    fs::read_to_string(path).expect("test file should be readable")
}

#[test]
fn formats_file_and_reports_changes() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sample.yaml");
    fs::write(&file_path, "b: 1\na: 2\n").unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("yars_format"))
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Formatted 1 file(s); 1 updated, 0 unchanged.",
        ));

    let contents = read_file(&file_path);
    assert!(contents.starts_with("a: 2"), "file should be reformatted");
}

#[test]
fn check_mode_detects_changes_without_modifying() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sample.yaml");
    fs::write(&file_path, "b: 1\na: 2\n").unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("yars_format"))
        .arg("--check")
        .arg(&file_path)
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains(
            "Checked 1 file(s); 1 would change.",
        ));

    let contents = read_file(&file_path);
    assert_eq!(contents, "b: 1\na: 2\n", "check mode must not rewrite file");
}

#[test]
fn verbose_mode_lists_files() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("formatted.yaml");
    fs::write(&file_path, "a: 1\nb: 2\n").unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("yars_format"))
        .arg("--verbose")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("already formatted")
                .and(predicate::str::contains("Formatted 1 file(s); 0 updated, 1 unchanged.")),
        );
}

#[test]
fn missing_file_reports_error() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("missing.yaml");

    Command::new(assert_cmd::cargo::cargo_bin!("yars_format"))
        .arg(&missing)
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Failed to read file"));
}

#[test]
fn generates_shell_completions() {
    Command::new(assert_cmd::cargo::cargo_bin!("yars_format"))
        .arg("--generate-completions")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicate::str::contains("_yars-format"));
}
