//! ADR 0025: 実装待ちの修正、補足、注記、別のリポジトリの修正先。

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_adr-guard");
const B: &str = "docs/adr/0002-b.md";

fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

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
    dir
}

fn write(root: &Path, path: &str, status: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, format!("# ADR\n\n{status}\n")).unwrap();
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn amend_inv3_kind_and_by() {
    let valid = [
        r#"Status: superseded-pending [{"part":null,"by":["docs/adr/0002-b.md"]}]"#,
        r#"Status: superseded [{"part":"a","by":[],"kind":"note","note":"n"}]"#,
        r#"Status: superseded [{"part":"a","by":["https://example.com/adr/0001.md"]}]"#,
        r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"kind":"supplement","note":"補足"}]"#,
    ];
    let invalid = [
        r#"Status: superseded-pending []"#,
        r#"Status: superseded [{"part":"a","by":[],"kind":"note"}]"#,
        r#"Status: superseded [{"part":"a","by":[]}]"#,
        r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"kind":"note","note":"n"}]"#,
        r#"Status: superseded [{"part":"a","by":["http://example.com/adr/0001.md"]}]"#,
        r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"kind":"other"}]"#,
        r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"note":""}]"#,
        r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"note":"a\nb"}]"#,
        r#"Status: superseded-pending [{"part":"a","by":"docs/adr/0002-b.md"}]"#,
    ];
    for (line, ok) in valid
        .iter()
        .map(|l| (l, true))
        .chain(invalid.iter().map(|l| (l, false)))
    {
        let dir = repo();
        let p = dir.path();
        write(p, "docs/adr/0001-a.md", line);
        write(p, B, "Status: accepted");
        let output = run(p, &["check-status"]);
        assert_eq!(
            output.status.success(),
            ok,
            "{line}: {}",
            text(&output.stderr)
        );
        assert!(git(p, &["add", "."]).status.success());
        let output = run(p, &["pre-commit", "--allow-status"]);
        // 新規ADRは proposed しか追加できないので、構文の誤りだけを確かめる
        if !ok {
            assert!(!output.status.success(), "{line}");
            assert!(
                text(&output.stderr).contains("docs/adr/0001-a.md"),
                "{line}: {}",
                text(&output.stderr)
            );
        }
    }
}

#[test]
fn amend_inv4_unimplemented() {
    let dir = repo();
    let p = dir.path();
    write(p, "docs/adr/0001-a.md", "Status: accepted-pending");
    write(
        p,
        "docs/adr/0003-c.md",
        r#"Status: superseded-pending [{"part":null,"by":["docs/adr/0002-b.md"]}]"#,
    );
    write(
        p,
        "docs/adr/0004-d.md",
        r#"Status: superseded [{"part":null,"by":["docs/adr/0002-b.md"]}]"#,
    );
    write(p, B, "Status: accepted");
    let output = run(p, &["unimplemented"]);
    assert!(output.status.success());
    assert_eq!(
        text(&output.stdout),
        "docs/adr/0001-a.md\ndocs/adr/0003-c.md\n"
    );
}

#[test]
fn amend_inv5_transitions() {
    let dir = repo();
    let p = dir.path();
    let hook = p.join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\nexec '{BIN}' pre-commit --allow-status\n"),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let commit = |path: &str, status: &str| {
        write(p, path, status);
        assert!(git(p, &["add", "."]).status.success());
        git(p, &["commit", "-qm", status])
    };
    let pending = r#"Status: superseded-pending [{"part":"a","by":["docs/adr/0002-b.md"]}]"#;
    let done = r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"]}]"#;
    assert!(commit(B, "Status: proposed").status.success());
    for status in [
        "Status: proposed",
        "Status: accepted-pending",
        pending,
        done,
    ] {
        let output = commit("docs/adr/0001-a.md", status);
        assert!(
            output.status.success(),
            "{status}: {}",
            text(&output.stderr)
        );
    }

    write(p, "docs/adr/0003-c.md", "Status: proposed");
    assert!(git(p, &["add", "."]).status.success());
    assert!(git(p, &["commit", "-qm", "c"]).status.success());
    assert!(
        commit("docs/adr/0003-c.md", "Status: accepted")
            .status
            .success()
    );
    let head = text(&git(p, &["rev-parse", "HEAD"]).stdout);
    let output = commit("docs/adr/0003-c.md", pending);
    assert!(!output.status.success());
    assert_eq!(text(&git(p, &["rev-parse", "HEAD"]).stdout), head);
}

#[test]
fn amend_inv6_trace_columns() {
    let dir = repo();
    let p = dir.path();
    write(
        p,
        "docs/adr/0001-a.md",
        r#"Status: superseded-pending [{"part":"入力","by":["docs/adr/0002-b.md"],"note":"消した"},{"part":"出力","by":["docs/adr/0003-c.md"],"kind":"supplement"},{"part":"既定","by":[],"kind":"note","note":"U+23CE とした"},{"part":null,"by":["https://example.com/adr/0001.md"]}]"#,
    );
    write(p, B, "Status: accepted");
    write(p, "docs/adr/0003-c.md", "Status: accepted");
    let output = run(p, &["trace", "docs/adr/0001-a.md"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let expected = "root\t\tdocs/adr/0001-a.md\t\t\t\n\
edge\tdocs/adr/0001-a.md\tdocs/adr/0002-b.md\t入力\treplace\t消した\n\
edge\tdocs/adr/0001-a.md\tdocs/adr/0003-c.md\t出力\tsupplement\t\n\
edge\tdocs/adr/0001-a.md\t\t既定\tnote\tU+23CE とした\n\
edge\tdocs/adr/0001-a.md\thttps://example.com/adr/0001.md\t\treplace\t\n";
    assert_eq!(text(&output.stdout), expected);
    assert!(
        text(&output.stdout)
            .lines()
            .all(|l| l.split('\t').count() == 6)
    );

    let output = run(p, &["trace", "--graph", "docs/adr/0001-a.md"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let expected = "docs/adr/0001-a.md\n\
├── 入力 -> docs/adr/0002-b.md (消した)\n\
├── 出力 -> docs/adr/0003-c.md (補足)\n\
├── 既定: U+23CE とした\n\
└── (範囲指定なし) -> https://example.com/adr/0001.md\n";
    assert_eq!(text(&output.stdout), expected);
}

#[test]
fn amend_inv7_trace_url() {
    let dir = repo();
    let p = dir.path();
    write(
        p,
        "docs/adr/0001-a.md",
        r#"Status: superseded [{"part":"a","by":["https://example.com/x.md"]}]"#,
    );
    for graph in [false, true] {
        let args: &[&str] = if graph {
            &["trace", "--graph", "docs/adr/0001-a.md"]
        } else {
            &["trace", "docs/adr/0001-a.md"]
        };
        let output = run(p, args);
        assert!(output.status.success(), "{}", text(&output.stderr));
        assert!(text(&output.stdout).contains("https://example.com/x.md"));
        assert_eq!(text(&output.stdout).lines().count(), 2);
    }
}
