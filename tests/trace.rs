use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_adr-guard");

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success()
    );
    dir
}

fn write(root: &Path, path: &str, status: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, format!("# ADR\n\n{status}\n")).unwrap();
}

fn trace(root: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("trace")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

#[test]
fn inv1_trace_order() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        "ステータス: 採択→入力は[ADR 0002](0002-b.md)、出力は[ADR 0003](0003-c.md)",
    );
    write(
        root,
        "docs/adr/0002-b.md",
        "ステータス: 採択→字句は[ADR 0004](0004-d.md)",
    );
    write(root, "docs/adr/0003-c.md", "Status: accepted");
    write(root, "docs/adr/0004-d.md", "Status: accepted");
    let expected = "docs/adr/0001-a.md\n├── 入力 -> docs/adr/0002-b.md\n│   └── 字句 -> docs/adr/0004-d.md\n└── 出力 -> docs/adr/0003-c.md\n";
    let output = trace(root, &["--graph", "docs/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes());
    let expected = "root\t\tdocs/adr/0001-a.md\t\nedge\tdocs/adr/0001-a.md\tdocs/adr/0002-b.md\t入力\nedge\tdocs/adr/0002-b.md\tdocs/adr/0004-d.md\t字句\nedge\tdocs/adr/0001-a.md\tdocs/adr/0003-c.md\t出力\n";
    let output = trace(root, &["docs/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn inv2_trace_target() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/server/adr/0001-a.md",
        "ステータス: 採択→[server ADR 0002](0002-b.md)",
    );
    write(root, "docs/server/adr/0002-b.md", "Status: accepted");
    write(root, "docs/adr/0002-other.md", "Status: accepted");
    let output = trace(root, &["docs/server/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("docs/server/adr/0002-b.md")
    );
}

#[test]
fn inv3_trace_failure() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        "ステータス: 採択→[ADR 0002](0002-missing.md)",
    );
    let output = trace(root, &["docs/adr/0001-a.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn inv1_join_is_rendered_twice_without_expansion() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        "ステータス: 採択→入力は[ADR 0002](0002-b.md)、出力は[ADR 0003](0003-c.md)",
    );
    write(
        root,
        "docs/adr/0002-b.md",
        "ステータス: 採択→字句は[ADR 0004](0004-d.md)",
    );
    write(
        root,
        "docs/adr/0003-c.md",
        "ステータス: 採択→字句は[ADR 0004](0004-d.md)",
    );
    write(root, "docs/adr/0004-d.md", "Status: accepted");
    let output = trace(root, &["--graph", "docs/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("└── 字句 -> docs/adr/0004-d.md [表示済み]"));
    let output = trace(root, &["--list", "docs/adr/0001-a.md"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .matches("docs/adr/0004-d.md")
            .count(),
        2
    );
}

#[test]
fn inv3_cycle_is_atomic() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        "ステータス: 採択→[ADR 0002](0002-b.md)",
    );
    write(
        root,
        "docs/adr/0002-b.md",
        "ステータス: 採択→[ADR 0001](0001-a.md)",
    );
    for format in ["--list", "--graph"] {
        let output = trace(root, &[format, "docs/adr/0001-a.md"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("循環"));
    }
}

#[test]
fn inv1_list_fields_escape_parts() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        r#"Status: superseded [{"part":"入力\t\\\"\n","by":["docs/adr/0002-b.md"]}]"#,
    );
    write(root, "docs/adr/0002-b.md", "Status: accepted");
    let output = trace(root, &["docs/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.lines().all(|line| line.split('\t').count() == 4));
    assert!(text.contains("入力\\t"), "{text:?}");
    assert!(text.contains("\\n"), "{text:?}");
}

#[test]
fn inv3_invalid_json_has_no_output() {
    let dir = repo();
    write(
        dir.path(),
        "docs/adr/0001-a.md",
        "Status: superseded [{bad}]",
    );
    let output = trace(dir.path(), &["docs/adr/0001-a.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn inv2_explicit_link_crosses_module_without_qualifier() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/server/adr/0001-a.md",
        "ステータス: 採択→[ADR 0002](../../client/adr/0002-b.md)",
    );
    write(root, "docs/server/adr/0002-local.md", "Status: accepted");
    write(root, "docs/client/adr/0002-b.md", "Status: accepted");
    let output = trace(root, &["docs/server/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("docs/client/adr/0002-b.md")
    );
}

#[test]
fn inv2_qualified_unlinked_reference() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/server/adr/0001-a.md",
        "ステータス: 採択→server ADR 0002",
    );
    write(root, "docs/server/adr/0002-b.md", "Status: accepted");
    write(root, "docs/adr/0002-other.md", "Status: accepted");
    let output = trace(root, &["docs/server/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("docs/server/adr/0002-b.md")
    );
}

#[test]
fn inv3_link_number_mismatch_is_atomic() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        "ステータス: 採択→[ADR 0002](0003-b.md)",
    );
    write(root, "docs/adr/0003-b.md", "Status: accepted");
    let output = trace(root, &["docs/adr/0001-a.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("番号が違います")
    );
}

#[cfg(unix)]
#[test]
fn inv3_symlinked_target_is_rejected() {
    use std::os::unix::fs::symlink;

    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        "Status: superseded [{\"part\":null,\"by\":[\"docs/adr/0002-b.md\"]}]",
    );
    let outside = TempDir::new().unwrap();
    write(outside.path(), "0002-b.md", "Status: accepted");
    symlink(
        outside.path().join("0002-b.md"),
        root.join("docs/adr/0002-b.md"),
    )
    .unwrap();
    let output = trace(root, &["docs/adr/0001-a.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("シンボリックリンク")
    );
}

#[test]
fn inv1_deep_trace_uses_bounded_stack() {
    let dir = repo();
    let root = dir.path();
    const DEPTH: usize = 1300;
    for number in 1..=DEPTH {
        let path = format!("docs/adr/{number:04}-n.md");
        let status = if number == DEPTH {
            "Status: accepted".to_owned()
        } else {
            format!(
                "Status: superseded [{{\"part\":null,\"by\":[\"docs/adr/{:04}-n.md\"]}}]",
                number + 1
            )
        };
        write(root, &path, &status);
    }
    let output = trace(root, &["docs/adr/0001-n.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout.iter().filter(|&&byte| byte == b'\n').count(),
        DEPTH
    );
}

#[test]
fn inv2_bare_numbers_and_linked_sequence() {
    let dir = repo();
    let root = dir.path();
    write(
        root,
        "docs/adr/0001-a.md",
        "ステータス: 採択→[ADR 0002](0002-b.md)、[0003](0003-c.md)",
    );
    write(root, "docs/adr/0002-b.md", "Status: accepted");
    write(root, "docs/adr/0003-c.md", "Status: accepted");
    let output = trace(root, &["docs/adr/0001-a.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("docs/adr/0002-b.md"));
    assert!(text.contains("docs/adr/0003-c.md"));
}
