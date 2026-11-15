//! ADR 0026: 旧形式のステータス行の修正先を、リンク先と裸の番号からも読む。

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_adr-guard");
const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/migrate-adr-status");
const A: &str = "docs/adr/0001-a.md";

fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

fn write(root: &Path, path: &str, status: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, format!("# ADR\n\n{status}\n")).unwrap();
}

/// 修正先になる3つのADRを置いた一時リポジトリ。
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    for args in [
        &["init", "-q"][..],
        &["config", "user.name", "test"],
        &["config", "user.email", "test@example.com"],
        &["config", "commit.gpgsign", "false"],
    ] {
        assert!(git(p, args).status.success());
    }
    for path in [
        "docs/adr/0002-b.md",
        "docs/adr/0003-c.md",
        "docs/adr/0004-d.md",
    ] {
        write(p, path, "Status: accepted");
    }
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `trace --list` の edge の行の（修正先、部分名）。
fn edges(root: &Path, path: &str) -> Result<Vec<(String, String)>, String> {
    let output = Command::new(BIN)
        .args(["trace", path])
        .current_dir(root)
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(text(&output.stderr));
    }
    Ok(text(&output.stdout)
        .lines()
        .filter(|line| line.starts_with("edge\t"))
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            (fields[2].to_owned(), fields[3].to_owned())
        })
        .collect())
}

fn resolve(status: &str) -> Result<Vec<(String, String)>, String> {
    let dir = repo();
    write(dir.path(), A, &format!("ステータス: {status}"));
    edges(dir.path(), A)
}

fn edge(target: &str, part: &str) -> (String, String) {
    (format!("docs/adr/{target}"), part.to_owned())
}

#[test]
fn legacy_inv1_unchanged() {
    let cases = [
        (
            "採択→a は [ADR 0002](0002-b.md)",
            vec![edge("0002-b.md", "a")],
        ),
        (
            "採択→a は ADR 0002、0003",
            vec![edge("0002-b.md", "a"), edge("0003-c.md", "a")],
        ),
        (
            "採択→a は [ADR 0002](0002-b.md)、b は [ADR 0003](0003-c.md)",
            vec![edge("0002-b.md", "a"), edge("0003-c.md", "b")],
        ),
        (
            "採択→0002、0003",
            vec![edge("0002-b.md", ""), edge("0003-c.md", "")],
        ),
    ];
    for (status, expected) in cases {
        assert_eq!(resolve(status), Ok(expected), "{status}");
    }
}

#[test]
fn legacy_inv2_link_destinations() {
    let cases = [
        (
            "採択→a は [docs/adr/0002](0002-b.md)",
            vec![edge("0002-b.md", "a")],
        ),
        (
            "採択→a は [docs/adr 0002](0002-b.md)",
            vec![edge("0002-b.md", "a")],
        ),
        (
            "採択→a は [0002](0002-b.md)、[0003](0003-c.md)",
            vec![edge("0002-b.md", "a"), edge("0003-c.md", "a")],
        ),
        (
            "採択→[0002](0002-b.md)、[0003](0003-c.md)",
            vec![edge("0002-b.md", ""), edge("0003-c.md", "")],
        ),
    ];
    for (status, expected) in cases {
        assert_eq!(resolve(status), Ok(expected), "{status}");
    }

    let error = resolve("採択→a は [docs/adr/0003](0002-b.md)").unwrap_err();
    assert!(error.contains("番号が違います"), "{error}");

    // モジュールのADRのディレクトリから、横断のADRへのリンク
    let dir = repo();
    let p = dir.path();
    let module = "docs/server/adr/0001-a.md";
    write(
        p,
        module,
        "ステータス: 採択→a は [docs/adr/0002](../../adr/0002-b.md)",
    );
    assert_eq!(edges(p, module), Ok(vec![edge("0002-b.md", "a")]));
}

#[test]
fn legacy_inv3_separators() {
    for separator in ["、", ",", " と ", " および ", "、 "] {
        let status =
            format!("採択→a は [ADR 0002](0002-b.md){separator}[docs/adr/0003](0003-c.md)");
        assert_eq!(
            resolve(&status),
            Ok(vec![edge("0002-b.md", "a"), edge("0003-c.md", "a")]),
            "{status}"
        );
    }
    for status in [
        "採択→a は ADR 0002、ADR 0003",
        "採択→a は ADR 0002 と ADR 0003",
    ] {
        assert_eq!(
            resolve(status),
            Ok(vec![edge("0002-b.md", "a"), edge("0003-c.md", "a")]),
            "{status}"
        );
    }
    assert_eq!(
        resolve("採択→a は ADR 0002 や ADR 0003"),
        Ok(vec![edge("0002-b.md", "a"), edge("0003-c.md", "や")])
    );
}

#[test]
fn legacy_inv4_every_change_has_targets() {
    let cases = [
        "採択→a は [ADR 0002](0002-b.md) で消した",
        "採択→a は [ADR 0002](0002-b.md)（新しい規則）",
        "採択→a は [ADR 0002](0002-b.md) (新しい規則)",
        "採択→a は [ADR 0002](0002-b.md)）",
    ];
    for status in cases {
        assert_eq!(
            resolve(status),
            Ok(vec![edge("0002-b.md", "a")]),
            "{status}"
        );
    }
}

#[test]
fn legacy_inv5_missing_target() {
    for status in [
        "採択→a は [ADR 0002](0002-b.md)、b は",
        "採択→a は [ADR 0002](0002-b.md)（b は）",
    ] {
        let error = resolve(status).unwrap_err();
        assert!(
            error.contains("修正先の番号がありません"),
            "{status}: {error}"
        );
    }
}

#[test]
fn legacy_inv6_migrate_note() {
    let dir = repo();
    let p = dir.path();
    write(
        p,
        A,
        "ステータス: 採択→`x` は [ADR 0002](0002-b.md) で消した",
    );
    write(
        p,
        "docs/adr/0005-e.md",
        "ステータス: 採択→y は [docs/adr/0003](0003-c.md)（位置は単位で返す）",
    );
    let migrate = |mode: &str| {
        Command::new(SCRIPT)
            .arg(mode)
            .current_dir(p)
            .output()
            .unwrap()
    };
    let written = migrate("--write");
    assert!(written.status.success(), "{}", text(&written.stderr));
    let a = std::fs::read_to_string(p.join(A)).unwrap();
    let line = a.lines().find(|l| l.starts_with("Status: ")).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(line.strip_prefix("Status: superseded ").unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!([{"part":"`x`","by":["docs/adr/0002-b.md"],"note":"で消した"}])
    );
    let e = std::fs::read_to_string(p.join("docs/adr/0005-e.md")).unwrap();
    assert!(e.contains(r#""note":"位置は単位で返す""#), "{e}");
    let again = migrate("--dry-run");
    assert!(again.status.success(), "{}", text(&again.stderr));
    assert!(again.stdout.is_empty(), "{}", text(&again.stdout));
}

#[test]
fn legacy_inv4_pre_commit() {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo();
    let p = dir.path();
    let hook = p.join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\nexec '{BIN}' pre-commit --allow-status\n"),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let legacy = [
        (
            "docs/adr/0006-f.md",
            "ステータス: 採択→a は [docs/adr/0002](0002-b.md)",
        ),
        (
            "docs/adr/0007-g.md",
            "ステータス: 採択→a は [ADR 0002](0002-b.md) で消した",
        ),
        (
            "docs/adr/0008-h.md",
            "ステータス: 採択→a は [0002](0002-b.md)、[0003](0003-c.md)",
        ),
    ];
    for (path, status) in legacy {
        write(p, path, status);
    }
    assert!(git(p, &["add", "."]).status.success());
    let output = git(p, &["commit", "-q", "--no-verify", "-m", "legacy"]);
    assert!(output.status.success(), "{}", text(&output.stderr));

    let canonical = [
        (
            "docs/adr/0006-f.md",
            r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"]}]"#,
        ),
        (
            "docs/adr/0007-g.md",
            r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"note":"で消した"}]"#,
        ),
        (
            "docs/adr/0008-h.md",
            r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md","docs/adr/0003-c.md"]}]"#,
        ),
    ];
    for (path, status) in canonical {
        write(p, path, status);
    }
    assert!(git(p, &["add", "."]).status.success());
    let output = git(p, &["commit", "-qm", "migrate"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
}

// ADR 0027: 旧形式の `置換（…）`。

#[test]
fn replaced_inv4_pre_commit() {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo();
    let p = dir.path();
    let path = "docs/adr/0009-i.md";
    write(p, path, "ステータス: 置換（[ADR 0002](0002-b.md)）");
    assert!(git(p, &["add", "."]).status.success());
    assert!(
        git(p, &["commit", "-q", "--no-verify", "-m", "legacy"])
            .status
            .success()
    );
    let hook = p.join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\nexec '{BIN}' pre-commit --allow-status\n"),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    write(
        p,
        path,
        r#"Status: superseded [{"part":null,"by":["docs/adr/0002-b.md"]}]"#,
    );
    assert!(git(p, &["add", "."]).status.success());
    let output = git(p, &["commit", "-qm", "migrate"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let range = Command::new(BIN)
        .args(["check-range", "--allow-status", "HEAD~1", "HEAD"])
        .current_dir(p)
        .output()
        .unwrap();
    assert!(range.status.success(), "{}", text(&range.stderr));

    // 移行スクリプトでも移り、二度目は差分がない
    let dir = repo();
    let p = dir.path();
    write(p, path, "ステータス: 置換（[ADR 0002](0002-b.md)）");
    let migrate = |mode: &str| {
        Command::new(SCRIPT)
            .arg(mode)
            .current_dir(p)
            .output()
            .unwrap()
    };
    let written = migrate("--write");
    assert!(written.status.success(), "{}", text(&written.stderr));
    let content = std::fs::read_to_string(p.join(path)).unwrap();
    assert!(
        content.contains(r#"Status: superseded [{"by":["docs/adr/0002-b.md"],"part":null}]"#),
        "{content}"
    );
    let again = migrate("--dry-run");
    assert!(again.status.success(), "{}", text(&again.stderr));
    assert!(again.stdout.is_empty());
}

#[test]
fn replaced_inv5_migrate_note() {
    let dir = repo();
    let p = dir.path();
    let path = "docs/adr/0009-i.md";
    write(p, path, "ステータス: 置換（[ADR 0002](0002-b.md)）で削った");
    let written = Command::new(SCRIPT)
        .arg("--write")
        .current_dir(p)
        .output()
        .unwrap();
    assert!(written.status.success(), "{}", text(&written.stderr));
    let content = std::fs::read_to_string(p.join(path)).unwrap();
    let line = content.lines().find(|l| l.starts_with("Status: ")).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(line.strip_prefix("Status: superseded ").unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!([{"part":null,"by":["docs/adr/0002-b.md"],"note":"で削った"}])
    );
}
