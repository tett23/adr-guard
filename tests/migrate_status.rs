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
    let preview = migrate(root, &["--dry-run"]);
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

// ADR 0024: --dry-run と、失敗を最後まで数えること。

const MIGRATABLE: &str = "# a\n\nStatus: Proposed\n";
const MIGRATED: &str = "# b\n\nStatus: accepted\n";
const FAILING: &str = "# c\n\nステータス: 謎\n";

fn repo_with(files: &[(&str, &str)]) -> TempDir {
    let root = TempDir::new().unwrap();
    git(root.path(), &["init", "-q"]);
    for (path, content) in files {
        write(root.path(), path, content.as_bytes());
    }
    root
}

fn snapshot(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let dir = root.join("docs/adr");
    fs::read_dir(&dir)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().path())
                .map(|path| {
                    (
                        path.file_name().unwrap().to_string_lossy().into_owned(),
                        fs::read(&path).unwrap(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// 移行できる1件、移行済み1件、移行できない2件（並びの先頭と末尾）。
fn mixed() -> TempDir {
    repo_with(&[
        ("docs/adr/0000-bad.md", FAILING),
        ("docs/adr/0001-a.md", MIGRATABLE),
        ("docs/adr/0002-b.md", MIGRATED),
        ("docs/adr/0009-bad.md", FAILING),
    ])
}

#[test]
fn dry_run_inv1_failures_block_all_writes() {
    let repo = mixed();
    let before = snapshot(repo.path());
    let result = migrate(repo.path(), &["--write"]);
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert_eq!(snapshot(repo.path()), before);
}

#[test]
fn dry_run_inv2_counts_every_failure() {
    for mode in ["--dry-run", "--write"] {
        let repo = mixed();
        let result = migrate(repo.path(), &[mode]);
        let text = stderr(&result);
        assert_eq!(result.status.code(), Some(1), "{mode}: {text}");
        let failures: Vec<&str> = text.lines().filter(|l| l.contains(".md:")).collect();
        assert_eq!(failures.len(), 2, "{mode}: {text}");
        assert!(failures[0].contains("0000-bad.md"), "{mode}: {text}");
        assert!(failures[1].contains("0009-bad.md"), "{mode}: {text}");
        assert!(
            text.contains("移行できる: 1件、移行済み: 1件、移行できない: 2件"),
            "{mode}: {text}"
        );
    }
}

/// ADR 0024 の境界の表の各行（N、K、M）。
fn table() -> Vec<(Vec<(&'static str, &'static str)>, bool)> {
    vec![
        (vec![], true),
        (vec![("docs/adr/0002-b.md", MIGRATED)], true),
        (vec![("docs/adr/0001-a.md", MIGRATABLE)], true),
        (
            vec![
                ("docs/adr/0001-a.md", MIGRATABLE),
                ("docs/adr/0009-bad.md", FAILING),
            ],
            false,
        ),
        (
            vec![
                ("docs/adr/0000-bad.md", FAILING),
                ("docs/adr/0001-a.md", MIGRATABLE),
                ("docs/adr/0009-bad.md", FAILING),
            ],
            false,
        ),
        (
            vec![
                ("docs/adr/0000-bad.md", FAILING),
                ("docs/adr/0009-bad.md", FAILING),
            ],
            false,
        ),
    ]
}

#[test]
fn dry_run_inv3_never_writes() {
    for (files, success) in table() {
        let repo = repo_with(&files);
        let before = snapshot(repo.path());
        let result = migrate(repo.path(), &["--dry-run"]);
        assert_eq!(result.status.success(), success, "{files:?}: {result:?}");
        assert_eq!(snapshot(repo.path()), before, "{files:?}");
        let diffs = String::from_utf8_lossy(&result.stdout)
            .matches("--- a/")
            .count();
        let expected = files.iter().filter(|(_, c)| *c == MIGRATABLE).count();
        assert_eq!(diffs, expected, "{files:?}");
    }
}

#[test]
fn dry_run_inv4_write_matches_preview() {
    let repo = repo_with(&[
        ("docs/adr/0001-a.md", MIGRATABLE),
        ("docs/adr/0002-b.md", MIGRATED),
        (
            "docs/adr/0003-c.md",
            "# c\r\n\r\nステータス: 採択（未実装）\r\n",
        ),
    ]);
    let root = repo.path();
    let before = snapshot(root);
    let preview = migrate(root, &["--dry-run"]);
    assert!(preview.status.success(), "{preview:?}");
    let stdout = String::from_utf8_lossy(&preview.stdout).into_owned();
    let written = migrate(root, &["--write"]);
    assert!(written.status.success(), "{written:?}");
    let after = snapshot(root);

    let changed: Vec<&String> = after.keys().filter(|k| after[*k] != before[*k]).collect();
    let previewed: Vec<String> = stdout
        .lines()
        .filter_map(|l| l.strip_prefix("--- a/docs/adr/"))
        .map(str::to_owned)
        .collect();
    assert_eq!(changed, previewed.iter().collect::<Vec<_>>());
    let added: Vec<&str> = stdout
        .lines()
        .filter_map(|l| l.strip_prefix('+'))
        .filter(|l| !l.starts_with("++"))
        .collect();
    for (name, line) in previewed.iter().zip(&added) {
        let content = String::from_utf8_lossy(&after[name]).into_owned();
        assert!(content.lines().any(|l| l == *line), "{name}: {content:?}");
    }
}

#[test]
fn dry_run_inv5_usage() {
    for args in [&[][..], &["--dry-run", "--write"][..], &["--foo"][..]] {
        let repo = mixed();
        let before = snapshot(repo.path());
        let result = migrate(repo.path(), args);
        assert_eq!(result.status.code(), Some(1), "{args:?}");
        assert!(stderr(&result).contains("--dry-run|--write"), "{args:?}");
        assert_eq!(snapshot(repo.path()), before, "{args:?}");
    }
}

// ADR 0025: 実装待ちの修正と、URL の修正先の移行。

#[test]
fn amend_migrate_pending_and_url() {
    let repo = repo_with(&[
        (
            "docs/adr/0001-a.md",
            "# a\n\nステータス: 採択（未実装）→丸めは [ADR 0002](0002-b.md)\n",
        ),
        ("docs/adr/0002-b.md", MIGRATED),
        (
            "docs/adr/0003-c.md",
            "# c\n\nステータス: 採択→言語は [other ADR 0001](https://example.com/docs/adr/0001-x.md)\n",
        ),
    ]);
    let root = repo.path();
    let first = migrate(root, &["--dry-run"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let second = migrate(root, &["--dry-run"]);
    assert_eq!(first.stdout, second.stdout);
    let written = migrate(root, &["--write"]);
    assert!(written.status.success(), "{}", stderr(&written));
    let a = fs::read_to_string(root.join("docs/adr/0001-a.md")).unwrap();
    assert!(
        a.contains(r#"Status: superseded-pending [{"by":["docs/adr/0002-b.md"],"part":"丸め"}]"#)
            || a.contains(
                r#"Status: superseded-pending [{"part":"丸め","by":["docs/adr/0002-b.md"]}]"#
            ),
        "{a}"
    );
    let c = fs::read_to_string(root.join("docs/adr/0003-c.md")).unwrap();
    assert!(
        c.contains(r#""by":["https://example.com/docs/adr/0001-x.md"]"#),
        "{c}"
    );
    assert!(c.contains("Status: superseded ["), "{c}");
}
