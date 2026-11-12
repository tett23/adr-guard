use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/migrate-adr-status");

fn git(root: &Path, args: &[&str]) {
    let result = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(result.status.success(), "git {args:?}: {result:?}");
}

fn write(root: &Path, path: &str, content: &[u8]) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn repo() -> TempDir {
    let root = TempDir::new().unwrap();
    git(root.path(), &["init", "-q"]);
    write(
        root.path(),
        "docs/adr/0001-first.md",
        b"# ADR 0001\r\n\r\nStatus: Proposed\r\n",
    );
    write(
        root.path(),
        "docs/adr/0002-second.md",
        "# ADR 0002

ステータス: 採択→一部は[ADR 0001](0001-first.md)
"
        .as_bytes(),
    );
    git(root.path(), &["add", "."]);
    root
}

fn migrate(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(SCRIPT)
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

#[test]
fn inv5_preview_write_and_repeat() {
    let repo = repo();
    let root = repo.path();
    let before = fs::read(root.join("docs/adr/0001-first.md")).unwrap();
    let preview = migrate(root, &[]);
    assert!(preview.status.success(), "{preview:?}");
    assert!(String::from_utf8_lossy(&preview.stdout).contains("Status: proposed"));
    assert_eq!(
        fs::read(root.join("docs/adr/0001-first.md")).unwrap(),
        before
    );

    let write_result = migrate(root, &["--write"]);
    assert!(write_result.status.success(), "{write_result:?}");
    let first = fs::read(root.join("docs/adr/0001-first.md")).unwrap();
    let second = fs::read_to_string(root.join("docs/adr/0002-second.md")).unwrap();
    assert_eq!(first, b"# ADR 0001\r\n\r\nStatus: proposed\r\n");
    let raw = second
        .lines()
        .find_map(|line| line.strip_prefix("Status: superseded "))
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(
        parsed,
        serde_json::json!([{"part":"一部","by":["docs/adr/0001-first.md"]}])
    );

    let repeated = migrate(root, &["--write"]);
    assert!(repeated.status.success(), "{repeated:?}");
    assert_eq!(
        fs::read(root.join("docs/adr/0001-first.md")).unwrap(),
        first
    );
    assert_eq!(
        fs::read_to_string(root.join("docs/adr/0002-second.md")).unwrap(),
        second
    );
}

#[test]
fn inv5_invalid_entry_prevents_all_writes() {
    let repo = repo();
    let root = repo.path();
    write(
        root,
        "docs/adr/0003-bad.md",
        "Status: Superseded by ADR 9999
"
        .as_bytes(),
    );
    let before = fs::read(root.join("docs/adr/0001-first.md")).unwrap();
    let result = migrate(root, &["--write"]);
    assert!(!result.status.success(), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("0003-bad.md"));
    assert_eq!(
        fs::read(root.join("docs/adr/0001-first.md")).unwrap(),
        before
    );
}

#[test]
fn inv5_unknown_with_note_fails() {
    let repo = repo();
    let root = repo.path();
    write(root, "docs/adr/0003-bad.md", b"Status: Possibly accepted\n");
    let result = migrate(root, &["--write"]);
    assert!(!result.status.success(), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("0003-bad.md"));
}

#[test]
fn inv5_superseded_annotation_fails() {
    let repo = repo();
    let root = repo.path();
    write(
        root,
        "docs/adr/0003-bad.md",
        b"Status: Superseded by ADR 0001 on 2026-10-01\n",
    );
    let result = migrate(root, &["--write"]);
    assert!(!result.status.success(), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("0003-bad.md"));
}

#[test]
fn inv5_bare_number_without_separator_fails() {
    let repo = repo();
    let root = repo.path();
    write(
        root,
        "docs/adr/0003-bad.md",
        b"Status: Superseded by ADR 0001 2026\n",
    );
    let result = migrate(root, &["--write"]);
    assert!(!result.status.success(), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("0003-bad.md"));
}
