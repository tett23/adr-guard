use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_adr-guard");
const ADR: &str = "docs/adr/0001-x.md";
const MODULE_ADR: &str = "docs/server/adr/0001-x.md";
const CONTENT: &str = "# ADR 0001: x\n\nステータス: 提案\n\n本文\n";

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn write(dir: &Path, path: &str, content: &str) {
    let path = dir.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// ADRを1件コミットした一時リポジトリを作る。
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    git(p, &["config", "user.name", "test"]);
    git(p, &["config", "user.email", "test@example.com"]);
    git(p, &["config", "commit.gpgsign", "false"]);
    write(p, ADR, "# ADR 0001: x\n\nステータス: 提案\n\n本文\n");
    git(p, &["add", "."]);
    git(p, &["commit", "-qm", "init"]);
    dir
}

/// `repo()` に加えて、モジュールのADRと、ADRではない `docs/` 以下のファイルをコミットした一時リポジトリを作る（ADR 0002）。
fn module_repo() -> TempDir {
    let dir = repo();
    let p = dir.path();
    for path in [
        MODULE_ADR,
        "docs/a/b/adr/0001-x.md",
        "docs/server/notes/a.md",
    ] {
        write(p, path, CONTENT);
    }
    git(p, &["add", "."]);
    git(p, &["commit", "-qm", "module"]);
    dir
}

/// 従来の既定（ステータスの行の変更を許可する）で `pre-commit` を実行する（ADR 0010）。
fn pre_commit(dir: &Path) -> bool {
    Command::new(BIN)
        .args(["pre-commit", "--allow-status"])
        .env_remove("ADR_GUARD_ALLOW")
        .current_dir(dir)
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

/// 従来の既定（ステータスの行の変更を許可する）で `claude-hook` を実行する（ADR 0010）。
fn claude_hook(dir: &Path, payload: serde_json::Value) -> i32 {
    claude_hook_with(dir, payload, &["--allow-status"])
}

fn claude_hook_with(dir: &Path, payload: serde_json::Value, options: &[&str]) -> i32 {
    let mut child = Command::new(BIN)
        .arg("claude-hook")
        .args(options)
        .env("CLAUDE_PROJECT_DIR", dir)
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    child.wait().unwrap().code().unwrap()
}

mod pre_commit {
    use super::*;

    #[test]
    fn rejects_missing_status_without_head() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), ADR, "x\n");
        git(dir.path(), &["add", "."]);
        assert!(!pre_commit(dir.path()));
    }

    #[test]
    fn allows_status_change() {
        let dir = repo();
        write(
            dir.path(),
            ADR,
            "# ADR 0001: x\n\nStatus: accepted\n\n本文\n",
        );
        git(dir.path(), &["add", "."]);
        assert!(pre_commit(dir.path()));
    }

    #[test]
    fn rejects_body_change() {
        let dir = repo();
        write(
            dir.path(),
            ADR,
            "# ADR 0001: x\n\nステータス: 提案\n\n改変\n",
        );
        git(dir.path(), &["add", "."]);
        assert!(!pre_commit(dir.path()));
    }

    #[test]
    fn rejects_delete() {
        let dir = repo();
        git(dir.path(), &["rm", "-q", ADR]);
        assert!(!pre_commit(dir.path()));
    }

    #[test]
    fn rejects_rename() {
        let dir = repo();
        git(dir.path(), &["mv", ADR, "docs/adr/0002-x.md"]);
        assert!(!pre_commit(dir.path()));
    }

    #[test]
    fn allows_new_adr() {
        let dir = repo();
        // ADRにはステータスの行がちょうど1つ要る（ADR 0012）
        write(dir.path(), "docs/adr/0002-y.md", "Status: proposed\n\ny\n");
        git(dir.path(), &["add", "."]);
        assert!(pre_commit(dir.path()));
    }
}

mod pre_commit_module {
    use super::*;

    fn staged(dir: &Path, path: &str, content: &str) -> bool {
        write(dir, path, content);
        git(dir, &["add", "-A"]);
        pre_commit(dir)
    }

    #[test]
    fn allows_status_change() {
        let dir = module_repo();
        assert!(staged(
            dir.path(),
            MODULE_ADR,
            &CONTENT.replace("ステータス: 提案", "Status: accepted")
        ));
    }

    #[test]
    fn rejects_body_change() {
        let dir = module_repo();
        assert!(!staged(
            dir.path(),
            MODULE_ADR,
            &CONTENT.replace("本文", "改変")
        ));
    }

    #[test]
    fn rejects_delete_and_rename() {
        for args in [
            vec!["rm", "-q", MODULE_ADR],
            vec!["mv", MODULE_ADR, "docs/server/adr/0002-x.md"],
        ] {
            let dir = module_repo();
            git(dir.path(), &args);
            assert!(!pre_commit(dir.path()), "{args:?}");
        }
    }

    #[test]
    fn allows_new_adr() {
        let dir = module_repo();
        // ADRにはステータスの行がちょうど1つ要る（ADR 0012）
        assert!(staged(
            dir.path(),
            "docs/client/adr/0001-y.md",
            "Status: proposed\n\ny\n"
        ));
    }

    #[test]
    fn rejects_move_from_docs_adr_to_module() {
        let dir = module_repo();
        git(dir.path(), &["mv", ADR, "docs/server/adr/0002-x.md"]);
        assert!(!pre_commit(dir.path()));
    }

    #[test]
    fn ignores_non_adr_files_under_docs() {
        for path in ["docs/a/b/adr/0001-x.md", "docs/server/notes/a.md"] {
            let dir = module_repo();
            assert!(staged(dir.path(), path, "改変\n"), "{path}");
        }
    }

    #[test]
    fn reports_violations_in_both_directories() {
        let dir = module_repo();
        let p = dir.path();
        write(p, ADR, "改変\n");
        write(p, MODULE_ADR, "改変\n");
        git(p, &["add", "-A"]);
        let output = Command::new(BIN)
            .arg("pre-commit")
            .current_dir(p)
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!output.status.success());
        assert!(stderr.contains(ADR), "{stderr}");
        assert!(stderr.contains(MODULE_ADR), "{stderr}");
    }
}

mod claude_hook {
    use super::*;
    use serde_json::json;

    #[test]
    fn allows_status_edit() {
        let dir = repo();
        let payload = json!({"tool_name": "Edit", "tool_input": {
            "file_path": ADR, "old_string": "ステータス: 提案", "new_string": "ステータス: 採択",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 0);
    }

    #[test]
    fn rejects_body_edit_by_absolute_path() {
        let dir = repo();
        let payload = json!({"tool_name": "Edit", "tool_input": {
            "file_path": dir.path().join(ADR), "old_string": "本文", "new_string": "改変",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 2);
    }

    #[test]
    fn rejects_write() {
        let dir = repo();
        let payload =
            json!({"tool_name": "Write", "tool_input": {"file_path": ADR, "content": "x"}});
        assert_eq!(claude_hook(dir.path(), payload), 2);
    }

    #[test]
    fn rejects_multi_edit_touching_body() {
        let dir = repo();
        let payload = json!({"tool_name": "MultiEdit", "tool_input": {"file_path": ADR, "edits": [
            {"old_string": "提案", "new_string": "採択"},
            {"old_string": "本文", "new_string": "改変"},
        ]}});
        assert_eq!(claude_hook(dir.path(), payload), 2);
    }

    #[test]
    fn allows_uncommitted_adr() {
        let dir = repo();
        let payload = json!({"tool_name": "Write", "tool_input": {
            "file_path": "docs/adr/0002-new.md", "content": "x",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 0);
    }

    #[test]
    fn allows_other_files() {
        let dir = repo();
        let payload =
            json!({"tool_name": "Write", "tool_input": {"file_path": "README.md", "content": "x"}});
        assert_eq!(claude_hook(dir.path(), payload), 0);
    }
}

mod claude_hook_module {
    use super::*;
    use serde_json::json;

    #[test]
    fn allows_status_edit() {
        let dir = module_repo();
        let payload = json!({"tool_name": "Edit", "tool_input": {
            "file_path": MODULE_ADR, "old_string": "ステータス: 提案", "new_string": "ステータス: 採択",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 0);
    }

    #[test]
    fn rejects_body_edit() {
        let dir = module_repo();
        let payload = json!({"tool_name": "Edit", "tool_input": {
            "file_path": dir.path().join(MODULE_ADR), "old_string": "本文", "new_string": "改変",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 2);
    }

    #[test]
    fn allows_non_adr_files_under_docs() {
        let dir = module_repo();
        let payload = json!({"tool_name": "Write", "tool_input": {
            "file_path": "docs/a/b/adr/0001-x.md", "content": "x",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 0);
    }
}

mod check_range {
    use super::*;

    fn commit(dir: &Path, path: &str, content: &str) -> String {
        write(dir, path, content);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-qm", "change"]);
        rev(dir, "HEAD")
    }

    fn rev(dir: &Path, name: &str) -> String {
        let output = Command::new("git")
            .args(["rev-parse", name])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// 従来の既定（ステータスの行の変更を許可する）で `check-range` を実行する（ADR 0010）。
    fn check_range(dir: &Path, base: &str, head: &str) -> bool {
        Command::new(BIN)
            .args(["check-range", "--allow-status", base, head])
            .current_dir(dir)
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    const ORIGINAL: &str = "# ADR 0001: x\n\nステータス: 提案\n\n本文\n";

    #[test]
    fn allows_adding_adr() {
        let dir = repo();
        let base = rev(dir.path(), "HEAD");
        let head = commit(dir.path(), "docs/adr/0002-y.md", "Status: proposed\n\ny\n");
        assert!(check_range(dir.path(), &base, &head));
    }

    #[test]
    fn allows_status_change() {
        let dir = repo();
        let base = rev(dir.path(), "HEAD");
        let head = commit(
            dir.path(),
            ADR,
            &ORIGINAL.replace("ステータス: 提案", "Status: accepted"),
        );
        assert!(check_range(dir.path(), &base, &head));
    }

    #[test]
    fn rejects_body_change_even_if_reverted_later() {
        let dir = repo();
        let base = rev(dir.path(), "HEAD");
        commit(dir.path(), ADR, &ORIGINAL.replace("本文", "改変"));
        let head = commit(dir.path(), ADR, ORIGINAL);
        assert!(!check_range(dir.path(), &base, &head));
    }

    #[test]
    fn rejects_delete_and_rename() {
        for args in [vec!["rm", "-q", ADR], vec!["mv", ADR, "docs/adr/0002-x.md"]] {
            let dir = repo();
            let base = rev(dir.path(), "HEAD");
            git(dir.path(), &args);
            git(dir.path(), &["commit", "-qm", "change"]);
            let head = rev(dir.path(), "HEAD");
            assert!(!check_range(dir.path(), &base, &head), "{args:?}");
        }
    }

    #[test]
    fn rejects_rewriting_adr_added_in_same_range() {
        let dir = repo();
        let base = rev(dir.path(), "HEAD");
        commit(dir.path(), "docs/adr/0002-y.md", "y\n");
        let head = commit(dir.path(), "docs/adr/0002-y.md", "z\n");
        assert!(!check_range(dir.path(), &base, &head));
    }

    #[test]
    fn ignores_commits_before_base() {
        let dir = repo();
        let base = commit(dir.path(), ADR, &ORIGINAL.replace("本文", "改変"));
        let head = commit(dir.path(), "README.md", "x\n");
        assert!(check_range(dir.path(), &base, &head));
    }

    #[test]
    fn checks_from_root_when_base_is_missing() {
        let dir = repo();
        let head = commit(dir.path(), ADR, &ORIGINAL.replace("本文", "改変"));
        let zero = "0000000000000000000000000000000000000000";
        assert!(!check_range(dir.path(), zero, &head));
    }

    #[test]
    fn rejects_module_adr_body_change() {
        let dir = module_repo();
        let base = rev(dir.path(), "HEAD");
        let head = commit(dir.path(), MODULE_ADR, &CONTENT.replace("本文", "改変"));
        assert!(!check_range(dir.path(), &base, &head));
    }

    #[test]
    fn allows_module_adr_status_change_and_non_adr_changes() {
        let dir = module_repo();
        let base = rev(dir.path(), "HEAD");
        commit(
            dir.path(),
            MODULE_ADR,
            &CONTENT.replace("ステータス: 提案", "Status: accepted"),
        );
        let head = commit(dir.path(), "docs/a/b/adr/0001-x.md", "改変\n");
        assert!(check_range(dir.path(), &base, &head));
    }

    #[test]
    fn ignores_merge_commits() {
        // 範囲外のコミットで持ち込まれた変更は、マージコミットの差分に現れても検査しない
        let dir = repo();
        let p = dir.path();
        git(p, &["switch", "-qc", "side"]);
        let base = commit(p, ADR, &ORIGINAL.replace("本文", "改変"));
        git(p, &["switch", "-q", "-"]);
        commit(p, "README.md", "x\n");
        git(p, &["merge", "-q", "--no-edit", "side"]);
        let head = rev(p, "HEAD");
        assert!(check_range(p, &base, &head));
    }
}

mod options {
    use super::*;

    fn run(args: &[&str]) -> (i32, String, String) {
        let output = Command::new(BIN).args(args).output().unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn version() -> String {
        format!("adr-guard {}", env!("CARGO_PKG_VERSION"))
    }

    #[test]
    fn prints_version() {
        for flag in ["--version", "-V"] {
            assert_eq!(run(&[flag]), (0, format!("{}\n", version()), String::new()));
        }
    }

    #[test]
    fn prints_help() {
        let (code, stdout, stderr) = run(&["--help"]);
        assert_eq!((code, stderr.as_str()), (0, ""));
        assert_eq!(stdout.lines().next(), Some(version().as_str()));
        for word in [
            "pre-commit",
            "claude-hook",
            "check-range <base> <head>",
            "--version",
            "-V",
            "--help",
            "-h",
        ] {
            assert!(stdout.contains(word), "{word}: {stdout}");
        }
        assert_eq!(run(&["-h"]), (code, stdout, stderr));
    }

    #[test]
    fn rejects_extra_arguments() {
        for args in [["--version", "x"], ["--help", "x"]] {
            let (code, stdout, _) = run(&args);
            assert_eq!((code, stdout.as_str()), (64, ""), "{args:?}");
        }
    }

    #[test]
    fn usage_mentions_options() {
        let (code, _, stderr) = run(&[]);
        assert_eq!(code, 64);
        assert!(stderr.contains("--version"), "{stderr}");
        assert!(stderr.contains("--help"), "{stderr}");
    }
}

mod listing {
    use super::*;

    fn adr(status: &str) -> String {
        format!("# ADR\n\n{status}\n\n本文\n")
    }

    /// ADR 0005 のテスト設計のファイルを置いた一時リポジトリを作る。
    fn listing_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        let committed = [
            ("docs/adr/0001-a.md", adr("ステータス: 採択")),
            ("docs/adr/0002-b.md", adr("ステータス: 提案")),
            ("docs/adr/0003-c.md", adr("ステータス: 採択→0004")),
            ("docs/adr/0004-d.md", adr("ステータス: 却下")),
            ("docs/adr/0005-e.md", adr("ステータス: 破棄")),
            ("docs/adr/0006-f.md", "# ADR\n\n本文\n".to_owned()),
            (
                "docs/server/adr/0001-a.md",
                adr("ステータス: 提案（レビュー中）"),
            ),
            ("docs/notes.md", adr("ステータス: 提案")),
            ("docs/a/b/adr/0001-a.md", adr("ステータス: 提案")),
            ("docs/adr/0009-i.txt", adr("ステータス: 提案")),
            ("docs/adr/0010-j.md", adr("ステータス: 提案")),
            (".gitignore", "docs/adr/0008-h.md\n".to_owned()),
        ];
        for (path, content) in &committed {
            write(p, path, content);
        }
        git(p, &["add", "."]);
        git(p, &["commit", "-qm", "init"]);
        write(p, "docs/adr/0007-g.md", &adr("ステータス: 提案"));
        write(p, "docs/adr/0008-h.md", &adr("ステータス: 提案"));
        std::fs::remove_file(p.join("docs/adr/0010-j.md")).unwrap();
        dir
    }

    fn run(dir: &Path, args: &[&str]) -> (i32, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .stderr(Stdio::null())
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout).unwrap(),
        )
    }

    fn lines(lines: &[&str]) -> String {
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    #[test]
    fn lists_all_adrs() {
        let dir = listing_repo();
        let expected = lines(&[
            "docs/adr/0001-a.md",
            "docs/adr/0002-b.md",
            "docs/adr/0003-c.md",
            "docs/adr/0004-d.md",
            "docs/adr/0005-e.md",
            "docs/adr/0006-f.md",
            "docs/adr/0007-g.md",
            "docs/server/adr/0001-a.md",
        ]);
        assert_eq!(run(dir.path(), &["list"]), (0, expected.clone()));
        assert_eq!(run(&dir.path().join("docs"), &["list"]), (0, expected));
    }

    #[test]
    fn lists_all_adrs_with_status_before_path() {
        let dir = listing_repo();
        let expected = lines(&[
            "採択\tdocs/adr/0001-a.md",
            "提案\tdocs/adr/0002-b.md",
            "採択→0004\tdocs/adr/0003-c.md",
            "却下\tdocs/adr/0004-d.md",
            "破棄\tdocs/adr/0005-e.md",
            "\tdocs/adr/0006-f.md",
            "提案\tdocs/adr/0007-g.md",
            "提案（レビュー中）\tdocs/server/adr/0001-a.md",
        ]);
        let args = ["list", "--status"];
        assert_eq!(run(dir.path(), &args), (0, expected.clone()));
        assert_eq!(run(&dir.path().join("docs"), &args), (0, expected));
    }

    #[test]
    fn lists_proposed_adrs() {
        let dir = listing_repo();
        let expected = lines(&[
            "docs/adr/0002-b.md",
            "docs/adr/0007-g.md",
            "docs/server/adr/0001-a.md",
        ]);
        assert_eq!(run(dir.path(), &["proposed"]), (0, expected.clone()));
        assert_eq!(run(&dir.path().join("docs"), &["proposed"]), (0, expected));
    }

    #[test]
    fn lists_proposed_adrs_with_status_before_path() {
        let dir = listing_repo();
        let expected = lines(&[
            "提案\tdocs/adr/0002-b.md",
            "提案\tdocs/adr/0007-g.md",
            "提案（レビュー中）\tdocs/server/adr/0001-a.md",
        ]);
        let args = ["proposed", "--status"];
        assert_eq!(run(dir.path(), &args), (0, expected.clone()));
        assert_eq!(run(&dir.path().join("docs"), &args), (0, expected));
    }

    #[test]
    fn prints_nothing_without_adrs() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), "README.md", "x\n");
        for args in [
            &["list"][..],
            &["list", "--status"],
            &["proposed"],
            &["proposed", "--status"],
        ] {
            assert_eq!(run(dir.path(), args), (0, String::new()), "{args:?}");
        }
    }

    #[test]
    fn fails_outside_repository() {
        let dir = TempDir::new().unwrap();
        for args in [
            &["list"][..],
            &["list", "--status"],
            &["proposed"],
            &["proposed", "--status"],
        ] {
            assert_eq!(run(dir.path(), args), (1, String::new()), "{args:?}");
        }
    }

    #[test]
    fn rejects_extra_arguments() {
        let dir = listing_repo();
        for args in [
            &["list", "x"][..],
            &["list", "--no-status"],
            &["proposed", "x"],
            &["proposed", "--no-status"],
            &["proposed", "--status", "x"],
            &["proposed", "--status", "--status"],
            &["--status", "proposed"],
            &["list", "--status", "x"],
            &["list", "--status", "--status"],
            &["--status", "list"],
            &["--no-status", "proposed"],
        ] {
            assert_eq!(run(dir.path(), args), (64, String::new()), "{args:?}");
        }
    }

    #[test]
    fn help_mentions_subcommands() {
        let (code, stdout) = run(Path::new("."), &["--help"]);
        assert_eq!(code, 0);
        for word in ["list", "proposed", "--status"] {
            assert!(stdout.contains(word), "{word}: {stdout}");
        }
        assert!(!stdout.contains("--no-status"), "{stdout}");
    }
}

mod allow {
    use super::*;

    const REWRITTEN: &str = "# ADR 0001: x\n\nステータス: 提案\n\n改変\n";

    fn rev(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// ステージした変更を、pre-commit hookを通さずに `message` でコミットする。
    fn commit_staged(dir: &Path, message: &str) -> String {
        git(dir, &["commit", "-q", "--no-verify", "-m", message]);
        rev(dir)
    }

    fn commit_rewrite(dir: &Path, path: &str, content: &str, message: &str) -> String {
        write(dir, path, content);
        git(dir, &["add", "-A"]);
        commit_staged(dir, message)
    }

    fn check_range(dir: &Path, base: &str, head: &str) -> (bool, String) {
        let output = Command::new(BIN)
            .args(["check-range", base, head])
            .current_dir(dir)
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn allows(message: &str) -> bool {
        let dir = repo();
        let base = rev(dir.path());
        let head = commit_rewrite(dir.path(), ADR, REWRITTEN, message);
        check_range(dir.path(), &base, &head).0
    }

    #[test]
    fn allows_rewrite_with_trailer() {
        let dir = repo();
        let base = rev(dir.path());
        let head = commit_rewrite(
            dir.path(),
            ADR,
            REWRITTEN,
            "rewrite\n\nADR-Guard-Allow: リンクにするため",
        );
        let (success, stderr) = check_range(dir.path(), &base, &head);
        assert!(success, "{stderr}");
        assert!(stderr.contains("リンクにするため"), "{stderr}");
    }

    #[test]
    fn allows_delete_and_rename_with_trailer() {
        for args in [vec!["rm", "-q", ADR], vec!["mv", ADR, "docs/adr/0002-x.md"]] {
            let dir = repo();
            commit_rewrite(
                dir.path(),
                ADR,
                &CONTENT.replace("ステータス: 提案", "Status: proposed"),
                "migrate status",
            );
            let base = rev(dir.path());
            git(dir.path(), &args);
            let head = commit_staged(dir.path(), "change\n\nADR-Guard-Allow: 理由");
            assert!(check_range(dir.path(), &base, &head).0, "{args:?}");
        }
    }

    #[test]
    fn trailer_key_is_case_insensitive() {
        assert!(allows("rewrite\n\nadr-guard-allow: 理由"));
    }

    #[test]
    fn rejects_empty_reason() {
        assert!(!allows("rewrite\n\nADR-Guard-Allow:"));
        assert!(!allows("rewrite\n\nADR-Guard-Allow:    "));
    }

    #[test]
    fn rejects_trailer_outside_last_paragraph() {
        assert!(!allows(
            "rewrite\n\nADR-Guard-Allow: 理由\n\n最後の段落はトレーラーではない"
        ));
    }

    #[test]
    fn rejects_other_keys() {
        assert!(!allows("rewrite\n\nADR-Guard-Allowed: 理由"));
    }

    #[test]
    fn allows_only_commits_with_trailer() {
        let dir = repo();
        let p = dir.path();
        let base = rev(p);
        commit_rewrite(p, ADR, REWRITTEN, "allowed\n\nADR-Guard-Allow: 理由");
        let head = commit_rewrite(p, ADR, "改変2\n", "not allowed");
        let (success, stderr) = check_range(p, &base, &head);
        assert!(!success);
        let violations: Vec<&str> = stderr
            .lines()
            .filter(|line| line.contains("変更できません"))
            .collect();
        assert_eq!(violations.len(), 1, "{stderr}");
        assert!(violations[0].contains(&head[..12]), "{stderr}");
    }

    #[test]
    fn prints_nothing_for_commits_without_violations() {
        let dir = repo();
        let base = rev(dir.path());
        let head = commit_rewrite(
            dir.path(),
            "README.md",
            "x\n",
            "readme\n\nADR-Guard-Allow: 理由",
        );
        assert_eq!(check_range(dir.path(), &base, &head), (true, String::new()));
    }

    fn pre_commit_with(dir: &Path, value: &str) -> (bool, String) {
        let output = Command::new(BIN)
            .arg("pre-commit")
            .env("ADR_GUARD_ALLOW", value)
            .current_dir(dir)
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    #[test]
    fn pre_commit_allows_with_env() {
        let dir = repo();
        write(dir.path(), ADR, REWRITTEN);
        git(dir.path(), &["add", "-A"]);
        let (success, stderr) = pre_commit_with(dir.path(), "リンクにするため");
        assert!(success, "{stderr}");
        assert!(stderr.contains("リンクにするため"), "{stderr}");
    }

    #[test]
    fn pre_commit_rejects_empty_env() {
        for value in ["", "   "] {
            let dir = repo();
            write(dir.path(), ADR, REWRITTEN);
            git(dir.path(), &["add", "-A"]);
            assert!(!pre_commit_with(dir.path(), value).0, "{value:?}");
        }
    }
}

mod links {
    use super::*;

    const ADR_A: &str = "# ADR 0001: a\n\nステータス: 採択\n\n本文\n";
    const ADR_B: &str = "# ADR 0002: b\n\nステータス: 採択\n\nADR 0001 を参照\n";

    /// ADR 0009 の結合テストの一時リポジトリ。
    fn links_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        for (path, content) in [
            ("docs/adr/0001-a.md", ADR_A),
            ("docs/adr/0002-b.md", ADR_B),
            ("docs/server/adr/0001-a.md", ADR_A),
            ("README.md", "ADR 0001 と ADR 0099\n"),
            ("docs/server/notes.md", "ADR 0001\n"),
            (
                "docs/specifications.md",
                "`ADR 0001` と [ADR 0002](adr/0002-b.md)\n",
            ),
        ] {
            write(p, path, content);
        }
        git(p, &["add", "."]);
        git(p, &["commit", "-qm", "init"]);
        dir
    }

    fn run(dir: &Path, args: &[&str], allow: Option<&str>) -> (i32, String) {
        let mut command = Command::new(BIN);
        command
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW");
        if let Some(value) = allow {
            command.env("ADR_GUARD_ALLOW", value);
        }
        let output = command.output().unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn read(dir: &Path, path: &str) -> String {
        std::fs::read_to_string(dir.join(path)).unwrap()
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn reports_unlinked_references() {
        let dir = links_repo();
        let (code, stderr) = run(dir.path(), &["links"], None);
        assert_eq!(code, 1, "{stderr}");
        for expected in [
            "README.md:1: ADR 0001 にリンクがありません",
            "docs/adr/0002-b.md:5: ADR 0001 にリンクがありません",
            "docs/server/notes.md:1: ADR 0001 にリンクがありません",
            "adr-guard links --fix",
        ] {
            assert!(stderr.contains(expected), "{expected}: {stderr}");
        }
        assert!(!stderr.contains("0099"), "{stderr}");
        assert!(!stderr.contains("specifications.md"), "{stderr}");
    }

    #[test]
    fn succeeds_without_unlinked_references() {
        let dir = repo();
        assert_eq!(run(dir.path(), &["links"], None), (0, String::new()));
    }

    #[test]
    fn fixes_documents_except_committed_adrs() {
        let dir = links_repo();
        let p = dir.path();
        let (code, stderr) = run(p, &["links", "--fix"], None);
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("--include-committed"), "{stderr}");
        assert_eq!(
            read(p, "README.md"),
            "[ADR 0001](docs/adr/0001-a.md) と ADR 0099\n"
        );
        assert_eq!(
            read(p, "docs/server/notes.md"),
            "[ADR 0001](adr/0001-a.md)\n"
        );
        assert_eq!(
            read(p, "docs/specifications.md"),
            "`ADR 0001` と [ADR 0002](adr/0002-b.md)\n"
        );
        assert_eq!(read(p, "docs/adr/0002-b.md"), ADR_B);
    }

    #[test]
    fn fixes_committed_adrs_with_include_committed() {
        for args in [
            ["links", "--fix", "--include-committed"],
            ["links", "--include-committed", "--fix"],
        ] {
            let dir = links_repo();
            let p = dir.path();
            assert_eq!(run(p, &args, None).0, 0, "{args:?}");
            assert_eq!(
                read(p, "docs/adr/0002-b.md"),
                ADR_B.replace("ADR 0001 を", "[ADR 0001](0001-a.md) を")
            );
            assert_eq!(run(p, &["links"], None), (0, String::new()), "{args:?}");
        }
    }

    #[test]
    fn rejects_wrong_arguments() {
        let dir = links_repo();
        for args in [
            &["links", "x"][..],
            &["links", "--include-committed"],
            &["links", "--fix", "--fix"],
            &["links", "--status"],
            &["pre-commit", "--fix"],
            &["check-range", "--allow-links", "a"],
        ] {
            assert_eq!(run(dir.path(), args, None).0, 64, "{args:?}");
        }
        let outside = TempDir::new().unwrap();
        assert_eq!(run(outside.path(), &["links"], None).0, 1);
    }

    #[test]
    fn allow_links_permits_link_only_changes() {
        let dir = links_repo();
        let p = dir.path();
        let base = head(p);
        run(p, &["links", "--fix", "--include-committed"], None);
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit"], None).0, 1);
        let (code, stderr) = run(p, &["pre-commit", "--allow-links"], None);
        assert_eq!(code, 0, "{stderr}");

        git(p, &["commit", "-q", "--no-verify", "-m", "link"]);
        let head = head(p);
        assert_eq!(run(p, &["check-range", &base, &head], None).0, 1);
        for args in [
            ["check-range", "--allow-links", &base, &head],
            ["check-range", &base, &head, "--allow-links"],
        ] {
            assert_eq!(run(p, &args, None).0, 0, "{args:?}");
        }
    }

    #[test]
    fn allow_links_rejects_other_changes() {
        for content in [
            ADR_B.replace("ADR 0001 を", "[ADR 0002](0001-a.md) を"),
            ADR_B.replace("ADR 0001 を参照", "[ADR 0001](0001-a.md) を改変"),
        ] {
            let dir = links_repo();
            let p = dir.path();
            let base = head(p);
            write(p, "docs/adr/0002-b.md", &content);
            git(p, &["add", "-A"]);
            assert_eq!(run(p, &["pre-commit", "--allow-links"], None).0, 1);
            git(p, &["commit", "-q", "--no-verify", "-m", "change"]);
            let head = head(p);
            let args = ["check-range", "--allow-links", &base, &head];
            assert_eq!(run(p, &args, None).0, 1, "{content}");
        }
    }

    #[test]
    fn pre_commit_checks_staged_documents() {
        let dir = links_repo();
        let p = dir.path();
        write(p, "docs/new.md", "ADR 0002 を参照\n");
        git(p, &["add", "docs/new.md"]);
        let (code, stderr) = run(p, &["pre-commit", "--allow-links"], None);
        assert_eq!(code, 1);
        assert!(
            stderr.contains("pre-commit: docs/new.md:1: ADR 0002 にリンクがありません"),
            "{stderr}"
        );
        assert_eq!(run(p, &["pre-commit"], Some("理由")).0, 1);

        write(p, "docs/new.md", "[ADR 0002](adr/0002-b.md) を参照\n");
        git(p, &["add", "docs/new.md"]);
        assert_eq!(run(p, &["pre-commit"], None).0, 0);
    }

    #[test]
    fn pre_commit_ignores_unstaged_documents() {
        let dir = links_repo();
        let p = dir.path();
        write(p, "docs/unstaged.md", "ADR 0002\n");
        write(p, "README.md", "ADR 0001 と ADR 0002\n");
        assert_eq!(run(p, &["pre-commit"], None).0, 0);
    }

    #[test]
    fn help_describes_everything() {
        let output = Command::new(BIN).arg("--help").output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        for word in [
            "pre-commit",
            "claude-hook",
            "check-range",
            "list",
            "proposed",
            "links",
            "--allow-links",
            "--fix",
            "--include-committed",
            "--status",
            "ADR_GUARD_ALLOW",
            "ADR-Guard-Allow",
            "64",
            "  2 ",
            "  1 ",
            "  0 ",
        ] {
            assert!(stdout.contains(word), "{word}: {stdout}");
        }
    }
}

mod allow_status {
    use super::*;
    use serde_json::json;

    const ADOPTED: &str = "# ADR 0001: x\n\nStatus: accepted\n\n本文\n";

    fn run(dir: &Path, args: &[&str]) -> (i32, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn pre_commit_requires_allow_status_for_status_changes() {
        let dir = repo();
        let p = dir.path();
        write(p, ADR, ADOPTED);
        git(p, &["add", "-A"]);
        let (code, stderr) = run(p, &["pre-commit"]);
        assert_eq!(code, 1);
        assert!(stderr.contains("--allow-status"), "{stderr}");
        assert_eq!(run(p, &["pre-commit", "--allow-status"]).0, 0);
        assert_eq!(run(p, &["pre-commit", "--allow-links"]).0, 1);
        assert_eq!(
            run(p, &["pre-commit", "--allow-links", "--allow-status"]).0,
            0
        );
    }

    #[test]
    fn check_range_requires_allow_status_for_status_changes() {
        let dir = repo();
        let p = dir.path();
        let base = head(p);
        write(p, ADR, ADOPTED);
        git(p, &["add", "-A"]);
        git(p, &["commit", "-q", "--no-verify", "-m", "adopt"]);
        let head = head(p);
        let (code, stderr) = run(p, &["check-range", &base, &head]);
        assert_eq!(code, 1);
        assert!(stderr.contains("--allow-status"), "{stderr}");
        for args in [
            ["check-range", "--allow-status", &base, &head],
            ["check-range", &base, &head, "--allow-status"],
        ] {
            assert_eq!(run(p, &args).0, 0, "{args:?}");
        }
    }

    #[test]
    fn claude_hook_requires_allow_status_for_status_edits() {
        let dir = repo();
        let payload = || {
            json!({"tool_name": "Edit", "tool_input": {
                "file_path": ADR, "old_string": "ステータス: 提案", "new_string": "ステータス: 採択",
            }})
        };
        assert_eq!(claude_hook_with(dir.path(), payload(), &[]), 2);
        assert_eq!(
            claude_hook_with(dir.path(), payload(), &["--allow-status"]),
            0
        );
    }

    #[test]
    fn rejects_wrong_options() {
        let dir = repo();
        for args in [
            &["claude-hook", "--allow-links"][..],
            &["pre-commit", "--allow-status", "--allow-status"],
            &["list", "--allow-status"],
        ] {
            assert_eq!(run(dir.path(), args).0, 64, "{args:?}");
        }
    }

    #[test]
    fn help_mentions_allow_status() {
        let output = Command::new(BIN).arg("--help").output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("--allow-status"), "{stdout}");
    }
}

mod nested {
    use super::*;
    use serde_json::json;

    const PACKAGE_ADR: &str = "packages/server/docs/adr/0001-a.md";

    /// `repo()` に加えて、パッケージの中のADRをコミットした一時リポジトリ（ADR 0011）。
    fn nested_repo() -> TempDir {
        let dir = repo();
        let p = dir.path();
        write(p, PACKAGE_ADR, CONTENT);
        git(p, &["add", "."]);
        git(p, &["commit", "-qm", "package"]);
        dir
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn run(dir: &Path, args: &[&str]) -> (i32, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout).unwrap(),
        )
    }

    #[test]
    fn pre_commit_protects_package_adrs() {
        let dir = nested_repo();
        let p = dir.path();
        write(p, PACKAGE_ADR, &CONTENT.replace("本文", "改変"));
        git(p, &["add", "-A"]);
        assert!(!pre_commit(p));

        write(
            p,
            PACKAGE_ADR,
            &CONTENT.replace("ステータス: 提案", "Status: accepted"),
        );
        git(p, &["add", "-A"]);
        assert!(pre_commit(p));
    }

    #[test]
    fn check_range_protects_package_adrs() {
        let dir = nested_repo();
        let p = dir.path();
        let base = head(p);
        write(p, PACKAGE_ADR, &CONTENT.replace("本文", "改変"));
        git(p, &["add", "-A"]);
        git(p, &["commit", "-q", "--no-verify", "-m", "change"]);
        let head = head(p);
        let args = ["check-range", "--allow-status", &base, &head];
        assert_eq!(run(p, &args).0, 1);
    }

    #[test]
    fn claude_hook_protects_package_adrs() {
        let dir = nested_repo();
        let payload = json!({"tool_name": "Edit", "tool_input": {
            "file_path": PACKAGE_ADR, "old_string": "本文", "new_string": "改変",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 2);
    }

    #[test]
    fn list_includes_package_adrs() {
        let dir = nested_repo();
        assert_eq!(
            run(dir.path(), &["list"]),
            (0, format!("{ADR}\n{PACKAGE_ADR}\n"))
        );
    }

    #[test]
    fn links_resolve_from_the_nearest_docs() {
        let dir = nested_repo();
        let p = dir.path();
        write(p, "packages/server/README.md", "ADR 0001\n");
        write(p, "README.md", "ADR 0001\n");
        assert_eq!(run(p, &["links", "--fix"]).0, 0);
        assert_eq!(
            std::fs::read_to_string(p.join("packages/server/README.md")).unwrap(),
            "[ADR 0001](docs/adr/0001-a.md)\n"
        );
        assert_eq!(
            std::fs::read_to_string(p.join("README.md")).unwrap(),
            format!("[ADR 0001]({ADR})\n")
        );
    }
}

mod status_lines {
    use super::*;
    use serde_json::json;

    const ENGLISH: &str = "# ADR 0002: b\n\nStatus: Proposed\n\nbody\n";

    fn run(dir: &Path, args: &[&str], allow: Option<&str>) -> (i32, String, String) {
        let mut command = Command::new(BIN);
        command
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW");
        if let Some(value) = allow {
            command.env("ADR_GUARD_ALLOW", value);
        }
        let output = command.output().unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// `repo()` に、英語のステータスの行のADRと、ステータスの行が無いADRをコミットしたもの。
    fn status_repo() -> TempDir {
        let dir = repo();
        let p = dir.path();
        write(p, "docs/adr/0002-b.md", ENGLISH);
        write(
            p,
            "docs/adr/0003-c.md",
            "# ADR 0003: c\n\n- Status: Accepted\n\nbody\n",
        );
        write(p, "docs/notes.md", "no status\n");
        git(p, &["add", "."]);
        git(p, &["commit", "-q", "--no-verify", "-m", "statuses"]);
        dir
    }

    #[test]
    fn check_status_reports_missing_and_duplicated_lines() {
        let dir = status_repo();
        let p = dir.path();
        write(
            p,
            "docs/adr/0004-d.md",
            "ステータス: 採択\n\nStatus: Accepted\n",
        );
        let (code, _, stderr) = run(p, &["check-status"], None);
        assert_eq!(code, 1);
        for expected in [
            "check-status: docs/adr/0003-c.md: ステータスの行がありません",
            "check-status: docs/adr/0004-d.md:3: ステータスの行が2つ以上あります（最初は1行目）",
        ] {
            assert!(stderr.contains(expected), "{expected}: {stderr}");
        }
        assert!(!stderr.contains("0001-x.md"), "{stderr}");
        assert!(!stderr.contains("0002-b.md"), "{stderr}");
        assert!(!stderr.contains("notes.md"), "{stderr}");
    }

    #[test]
    fn check_status_succeeds_with_one_line_each() {
        let dir = repo();
        assert_eq!(run(dir.path(), &["check-status"], None).0, 0);
        assert_eq!(run(dir.path(), &["check-status", "x"], None).0, 64);
    }

    #[test]
    fn pre_commit_checks_status_lines_of_staged_adrs() {
        let dir = repo();
        let p = dir.path();
        write(p, "docs/adr/0002-b.md", "# ADR 0002\n\nbody\n");
        git(p, &["add", "-A"]);
        let (code, _, stderr) = run(p, &["pre-commit", "--allow-status"], Some("理由"));
        assert_eq!(code, 1);
        assert!(stderr.contains("ステータスの行がありません"), "{stderr}");

        write(
            p,
            "docs/adr/0002-b.md",
            "ステータス: 提案\nStatus: Proposed\n",
        );
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit", "--allow-status"], None).0, 1);

        write(p, "docs/adr/0002-b.md", "Status: proposed\n");
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit", "--allow-status"], None).0, 0);
    }

    #[test]
    fn pre_commit_ignores_unstaged_adrs() {
        let dir = repo();
        write(dir.path(), "docs/adr/0002-b.md", "no status\n");
        assert_eq!(
            run(dir.path(), &["pre-commit", "--allow-status"], None).0,
            0
        );
    }

    #[test]
    fn allow_status_permits_english_status_changes() {
        let dir = status_repo();
        let p = dir.path();
        let base = head(p);
        write(
            p,
            "docs/adr/0002-b.md",
            &ENGLISH.replace("Proposed", "accepted"),
        );
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit"], None).0, 1);
        assert_eq!(run(p, &["pre-commit", "--allow-status"], None).0, 0);
        git(p, &["commit", "-q", "--no-verify", "-m", "accept"]);
        let head = head(p);
        assert_eq!(run(p, &["check-range", &base, &head], None).0, 1);
        let args = ["check-range", "--allow-status", &base, &head];
        assert_eq!(run(p, &args, None).0, 0);
    }

    #[test]
    fn allow_status_permits_adding_a_missing_status_line() {
        let dir = status_repo();
        let p = dir.path();
        write(
            p,
            "docs/adr/0003-c.md",
            // 空行などを足さず、ステータスの行1行だけを加える
            "# ADR 0003: c\n\nStatus: accepted\n- Status: Accepted\n\nbody\n",
        );
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit"], None).0, 1);
        assert_eq!(run(p, &["pre-commit", "--allow-status"], None).0, 0);
    }

    #[test]
    fn claude_hook_permits_english_status_edits() {
        let dir = status_repo();
        let payload = json!({"tool_name": "Edit", "tool_input": {
            "file_path": "docs/adr/0002-b.md", "old_string": "Proposed", "new_string": "Accepted",
        }});
        assert_eq!(claude_hook(dir.path(), payload), 0);
    }

    #[test]
    fn proposed_and_list_use_the_status_adt() {
        let dir = status_repo();
        let p = dir.path();
        write(p, "docs/adr/0004-d.md", "Status: Draft\n");
        assert_eq!(
            run(p, &["proposed"], None).1,
            "docs/adr/0001-x.md\ndocs/adr/0002-b.md\n"
        );
        assert_eq!(
            run(p, &["list", "--status"], None).1,
            "提案\tdocs/adr/0001-x.md\nProposed\tdocs/adr/0002-b.md\n\tdocs/adr/0003-c.md\nDraft\tdocs/adr/0004-d.md\n"
        );
    }

    #[test]
    fn help_describes_status_lines() {
        let (_, stdout, _) = run(Path::new("."), &["--help"], None);
        for word in ["Status", "Proposed", "廃止", "check-status"] {
            assert!(stdout.contains(word), "{word}: {stdout}");
        }
    }
}

mod records {
    use super::*;
    use serde_json::json;

    const AUDIT: &str = "docs/audit/2026-10-07.md";
    const BENCHMARK: &str = "docs/benchmarks/v0.1.0.json";

    /// `repo()` に、監査と計測の記録をコミットした一時リポジトリ（ADR 0013）。
    fn records_repo() -> TempDir {
        let dir = repo();
        let p = dir.path();
        write(p, AUDIT, "# 監査\n\n結果\n");
        write(p, BENCHMARK, "{\"ms\": 10}\n");
        git(p, &["add", "."]);
        git(p, &["commit", "-qm", "records"]);
        dir
    }

    fn run(dir: &Path, args: &[&str], allow: Option<&str>) -> (i32, String) {
        let mut command = Command::new(BIN);
        command
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW");
        if let Some(value) = allow {
            command.env("ADR_GUARD_ALLOW", value);
        }
        let output = command.output().unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    const PRE_COMMIT: [&str; 3] = ["pre-commit", "--allow-status", "--allow-links"];

    #[test]
    fn pre_commit_rejects_changing_records() {
        for path in [AUDIT, BENCHMARK] {
            let dir = records_repo();
            let p = dir.path();
            write(p, path, "changed\n");
            git(p, &["add", "-A"]);
            let (code, stderr) = run(p, &PRE_COMMIT, None);
            assert_eq!(code, 1, "{path}");
            assert!(stderr.contains("コミット済みの記録"), "{stderr}");
            assert!(stderr.contains("新しい記録を作成してください"), "{stderr}");
            assert_eq!(run(p, &PRE_COMMIT, Some("誤記の修正")).0, 0, "{path}");
        }
    }

    #[test]
    fn pre_commit_rejects_deleting_and_moving_records() {
        for args in [
            vec!["rm", "-q", AUDIT],
            vec!["mv", AUDIT, "docs/audit/moved.md"],
        ] {
            let dir = records_repo();
            git(dir.path(), &args);
            let (code, stderr) = run(dir.path(), &PRE_COMMIT, None);
            assert_eq!(code, 1, "{args:?}");
            assert!(stderr.contains("削除・移動できません"), "{stderr}");
        }
    }

    #[test]
    fn pre_commit_allows_new_records() {
        let dir = records_repo();
        write(dir.path(), "docs/audit/2026-10-08.md", "# 監査\n");
        git(dir.path(), &["add", "-A"]);
        assert_eq!(run(dir.path(), &PRE_COMMIT, None).0, 0);
    }

    #[test]
    fn check_range_allows_records_only_with_trailer() {
        let dir = records_repo();
        let p = dir.path();
        let base = head(p);
        write(p, AUDIT, "changed\n");
        git(p, &["add", "-A"]);
        git(p, &["commit", "-q", "--no-verify", "-m", "change"]);
        let changed = head(p);
        let args = [
            "check-range",
            "--allow-status",
            "--allow-links",
            &base,
            &changed,
        ];
        assert_eq!(run(p, &args, None).0, 1);

        git(
            p,
            &[
                "commit",
                "-q",
                "--amend",
                "--no-verify",
                "-m",
                "change\n\nADR-Guard-Allow: 誤記の修正",
            ],
        );
        let allowed = head(p);
        let args = [
            "check-range",
            "--allow-status",
            "--allow-links",
            &base,
            &allowed,
        ];
        assert_eq!(run(p, &args, None).0, 0);
    }

    #[test]
    fn claude_hook_rejects_editing_committed_records() {
        let dir = records_repo();
        let edit = |path: &str| {
            json!({"tool_name": "Edit", "tool_input": {
                "file_path": path, "old_string": "結果", "new_string": "改変",
            }})
        };
        assert_eq!(claude_hook(dir.path(), edit(AUDIT)), 2);
        write(dir.path(), "docs/audit/draft.md", "結果\n");
        assert_eq!(claude_hook(dir.path(), edit("docs/audit/draft.md")), 0);
    }

    #[test]
    fn links_fix_skips_committed_records_by_default() {
        let dir = records_repo();
        let p = dir.path();
        write(p, AUDIT, "ADR 0001 を参照\n");
        git(p, &["add", "-A"]);
        git(p, &["commit", "-q", "--no-verify", "-m", "audit"]);

        let (code, stderr) = run(p, &["links", "--fix"], None);
        assert_eq!(code, 1);
        assert!(stderr.contains("コミット済みの記録"), "{stderr}");
        assert_eq!(
            std::fs::read_to_string(p.join(AUDIT)).unwrap(),
            "ADR 0001 を参照\n"
        );

        let (code, stderr) = run(p, &["links", "--fix", "--include-committed"], None);
        assert_eq!(code, 0);
        assert!(
            stderr.contains("ADR-Guard-Allow のトレーラーが要ります"),
            "{stderr}"
        );
        assert_eq!(
            std::fs::read_to_string(p.join(AUDIT)).unwrap(),
            "[ADR 0001](../adr/0001-x.md) を参照\n"
        );
    }

    #[test]
    fn reports_record_and_adr_violations_together() {
        let dir = records_repo();
        let p = dir.path();
        write(p, AUDIT, "changed\n");
        write(p, ADR, &CONTENT.replace("本文", "改変"));
        git(p, &["add", "-A"]);
        let (code, stderr) = run(p, &PRE_COMMIT, None);
        assert_eq!(code, 1);
        assert!(stderr.contains(AUDIT), "{stderr}");
        assert!(stderr.contains(ADR), "{stderr}");
    }
}

mod directory_name_variants {
    use super::*;

    /// ADR 0014 のディレクトリの名前の揺れだけを持つ一時リポジトリ（`docs/adr/` は作らない）。
    fn variants_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        write(
            p,
            "docs/adrs/0001-a.md",
            "# ADR 0001: a\n\nステータス: 採択\n\n本文\n",
        );
        write(
            p,
            "docs/decisions/0002-b.md",
            "# ADR 0002: b\n\nステータス: 採択\n\n本文\n",
        );
        write(p, "docs/audits/a.md", "# 監査\n\n結果\n");
        write(p, "docs/benchmark/v1.json", "{\"ms\": 10}\n");
        git(p, &["add", "."]);
        git(p, &["commit", "-q", "--no-verify", "-m", "init"]);
        dir
    }

    fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    const PRE_COMMIT: [&str; 3] = ["pre-commit", "--allow-status", "--allow-links"];

    #[test]
    fn pre_commit_rejects_body_changes_of_adrs_in_variant_directories() {
        let dir = variants_repo();
        let p = dir.path();
        write(
            p,
            "docs/adrs/0001-a.md",
            "# ADR 0001: a\n\nステータス: 採択\n\n改変\n",
        );
        write(
            p,
            "docs/decisions/0002-b.md",
            "# ADR 0002: b\n\nステータス: 採択\n\n改変\n",
        );
        git(p, &["add", "-A"]);
        let (code, _, stderr) = run(p, &PRE_COMMIT);
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("docs/adrs/0001-a.md"), "{stderr}");
        assert!(stderr.contains("docs/decisions/0002-b.md"), "{stderr}");
    }

    #[test]
    fn list_includes_adrs_in_variant_directories() {
        let dir = variants_repo();
        let (code, stdout, stderr) = run(dir.path(), &["list"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stdout.contains("docs/adrs/0001-a.md"), "{stdout}");
        assert!(stdout.contains("docs/decisions/0002-b.md"), "{stdout}");
        assert!(!stdout.contains("docs/audits/"), "{stdout}");
    }

    #[test]
    fn check_status_checks_adrs_in_variant_directories() {
        let dir = variants_repo();
        let p = dir.path();
        assert_eq!(run(p, &["check-status"]).0, 0);
        write(p, "docs/decisions/0003-c.md", "# ADR 0003: c\n\n本文\n");
        git(p, &["add", "-A"]);
        git(p, &["commit", "-q", "--no-verify", "-m", "no status"]);
        let (code, _, stderr) = run(p, &["check-status"]);
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("docs/decisions/0003-c.md"), "{stderr}");
    }

    #[test]
    fn links_fix_links_to_adrs_in_variant_directories() {
        let dir = variants_repo();
        let p = dir.path();
        write(p, "README.md", "ADR 0001 と ADR 0002 を参照する\n");
        let (code, _, stderr) = run(p, &["links", "--fix"]);
        assert_eq!(code, 0, "{stderr}");
        let readme = std::fs::read_to_string(p.join("README.md")).unwrap();
        assert_eq!(
            readme,
            "[ADR 0001](docs/adrs/0001-a.md) と [ADR 0002](docs/decisions/0002-b.md) を参照する\n"
        );
    }

    #[test]
    fn pre_commit_rejects_changes_of_records_in_variant_directories() {
        let dir = variants_repo();
        let p = dir.path();
        write(p, "docs/audits/a.md", "changed\n");
        write(p, "docs/benchmark/v1.json", "{\"ms\": 20}\n");
        git(p, &["add", "-A"]);
        let (code, _, stderr) = run(p, &PRE_COMMIT);
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("コミット済みの記録"), "{stderr}");
        assert!(stderr.contains("docs/audits/"), "{stderr}");
        assert!(stderr.contains("docs/benchmark/"), "{stderr}");
        assert!(stderr.contains("docs/audits/a.md"), "{stderr}");
        assert!(stderr.contains("docs/benchmark/v1.json"), "{stderr}");
    }
}

mod link_changes {
    use super::*;

    const DOC: &str = "# ADR 0001: x\n\nステータス: 採択\n\n[仕様](../specifications.md) と 本家\n";

    fn run(dir: &Path, args: &[&str]) -> i32 {
        Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .code()
            .unwrap()
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// `repo()` のADRを、リンクを含む内容にしてコミットしたもの（ADR 0015）。
    fn links_repo() -> TempDir {
        let dir = repo();
        let p = dir.path();
        write(p, ADR, DOC);
        write(p, "docs/audit/a.md", "[仕様](../specifications.md)\n");
        git(p, &["add", "-A"]);
        git(p, &["commit", "-q", "--no-verify", "-m", "links"]);
        dir
    }

    #[test]
    fn allow_links_permits_destination_changes() {
        let dir = links_repo();
        let p = dir.path();
        let base = head(p);
        write(
            p,
            ADR,
            &DOC.replace("../specifications.md", "../../specifications.md"),
        );
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit"]), 1);
        assert_eq!(run(p, &["pre-commit", "--allow-links"]), 0);
        git(p, &["commit", "-q", "--no-verify", "-m", "move"]);
        let head = head(p);
        assert_eq!(run(p, &["check-range", &base, &head]), 1);
        assert_eq!(run(p, &["check-range", "--allow-links", &base, &head]), 0);
    }

    #[test]
    fn allow_links_permits_adding_links() {
        let dir = links_repo();
        let p = dir.path();
        write(
            p,
            ADR,
            &DOC.replace("と 本家", "と [本家](https://example.com)"),
        );
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit"]), 1);
        assert_eq!(run(p, &["pre-commit", "--allow-links"]), 0);
    }

    #[test]
    fn allow_links_rejects_link_text_changes() {
        let dir = links_repo();
        let p = dir.path();
        write(p, ADR, &DOC.replace("[仕様]", "[仕様書]"));
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit", "--allow-links"]), 1);
    }

    #[test]
    fn allow_links_does_not_apply_to_records() {
        let dir = links_repo();
        let p = dir.path();
        write(p, "docs/audit/a.md", "[仕様](../../specifications.md)\n");
        git(p, &["add", "-A"]);
        assert_eq!(
            run(p, &["pre-commit", "--allow-status", "--allow-links"]),
            1
        );
    }
}

mod unimplemented {
    use super::*;

    /// ADR 0016 の結合テストの一時リポジトリ。
    fn unimplemented_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        for (path, status) in [
            ("docs/adr/0001-a.md", "ステータス: 採択（未実装）"),
            ("docs/adr/0002-b.md", "ステータス: 採択(未実装)"),
            ("docs/adr/0003-c.md", "Status: Accepted (not implemented)"),
            ("docs/adr/0004-d.md", "ステータス: 採択"),
            ("docs/adr/0005-e.md", "ステータス: 提案"),
        ] {
            write(p, path, &format!("# ADR\n\n{status}\n\n本文\n"));
        }
        git(p, &["add", "."]);
        git(p, &["commit", "-q", "--no-verify", "-m", "init"]);
        dir
    }

    fn run(dir: &Path, args: &[&str]) -> (i32, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .stderr(Stdio::null())
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout).unwrap(),
        )
    }

    #[test]
    fn lists_unimplemented_adrs() {
        let dir = unimplemented_repo();
        assert_eq!(
            run(dir.path(), &["unimplemented"]),
            (
                0,
                "docs/adr/0001-a.md\ndocs/adr/0002-b.md\ndocs/adr/0003-c.md\n".to_owned()
            )
        );
        assert_eq!(
            run(dir.path(), &["unimplemented", "--status"]),
            (
                0,
                "採択（未実装）\tdocs/adr/0001-a.md\n採択(未実装)\tdocs/adr/0002-b.md\nAccepted (not implemented)\tdocs/adr/0003-c.md\n"
                    .to_owned()
            )
        );
        assert_eq!(
            run(dir.path(), &["proposed"]),
            (0, "docs/adr/0005-e.md\n".to_owned())
        );
    }

    #[test]
    fn prints_nothing_without_unimplemented_adrs() {
        let dir = repo();
        assert_eq!(run(dir.path(), &["unimplemented"]), (0, String::new()));
        let outside = TempDir::new().unwrap();
        assert_eq!(run(outside.path(), &["unimplemented"]).0, 1);
    }

    #[test]
    fn rejects_wrong_arguments() {
        let dir = repo();
        for args in [&["unimplemented", "x"][..], &["unimplemented", "--fix"]] {
            assert_eq!(run(dir.path(), args).0, 64, "{args:?}");
        }
    }

    #[test]
    fn help_mentions_unimplemented() {
        let (_, stdout) = run(Path::new("."), &["--help"]);
        for word in ["unimplemented", "採択（未実装）"] {
            assert!(stdout.contains(word), "{word}: {stdout}");
        }
    }

    #[test]
    fn allow_status_permits_marking_as_implemented() {
        let dir = unimplemented_repo();
        let p = dir.path();
        write(
            p,
            "docs/adr/0001-a.md",
            "# ADR\n\nStatus: accepted\n\n本文\n",
        );
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &["pre-commit", "--allow-status"]).0, 0);
    }
}

mod renumber {
    use super::*;

    const A: &str = "# ADR 0017: a\n\nステータス: 採択\n\nA の本文\n";
    const B: &str = "# ADR 0017: b\n\nステータス: 採択\n\nB の本文\n";

    fn run(dir: &Path, args: &[&str]) -> (i32, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn read(dir: &Path, path: &str) -> String {
        std::fs::read_to_string(dir.join(path)).unwrap()
    }

    /// ADR 0018 の結合テストの一時リポジトリ。`0017-a.md` と `0017-b.md` を別々のコミットで追加する。
    fn duplicate_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        write(
            p,
            "docs/adr/0016-x.md",
            "# ADR 0016: x\n\nステータス: 採択\n\n[ADR 0017](0017-b.md) を参照\n",
        );
        write(p, "docs/adr/0017-a.md", A);
        git(p, &["add", "."]);
        commit_at(p, "a", "2026-01-01T00:00:00");
        write(p, "docs/adr/0017-b.md", B);
        write(
            p,
            "README.md",
            "[ADR 0017](docs/adr/0017-b.md) と [A](docs/adr/0017-a.md)\n",
        );
        git(p, &["add", "."]);
        commit_at(p, "b", "2026-01-02T00:00:00");
        dir
    }

    /// 作者とコミットの日付を `date` にしてコミットする（追加された順を決めるため）。
    fn commit_at(dir: &Path, message: &str, date: &str) {
        let status = Command::new("git")
            .args(["commit", "-q", "--no-verify", "-m", message])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn check_numbers_reports_duplicates() {
        let dir = duplicate_repo();
        let (code, stderr) = run(dir.path(), &["check-numbers"]);
        assert_eq!(code, 1);
        assert!(
            stderr.contains(
                "check-numbers: docs/adr: ADR 0017 が重複しています: 0017-a.md, 0017-b.md"
            ),
            "{stderr}"
        );
        assert!(stderr.contains("adr-guard renumber"), "{stderr}");
        assert_eq!(run(repo().path(), &["check-numbers"]).0, 0);
    }

    #[test]
    fn check_numbers_ignores_other_directories() {
        let dir = repo();
        let p = dir.path();
        write(
            p,
            "docs/server/adr/0001-x.md",
            "# ADR 0001: x\n\nステータス: 採択\n",
        );
        assert_eq!(run(p, &["check-numbers"]).0, 0);
    }

    #[test]
    fn dry_run_shows_the_plan_without_changing_files() {
        let dir = duplicate_repo();
        let p = dir.path();
        let (code, stderr) = run(p, &["renumber", "--dry-run"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(
            stderr.contains("docs/adr: 0017-b.md -> 0018-b.md"),
            "{stderr}"
        );
        assert!(p.join("docs/adr/0017-b.md").exists());
        assert!(!p.join("docs/adr/0018-b.md").exists());
    }

    #[test]
    fn renumber_renames_and_fixes_links() {
        let dir = duplicate_repo();
        let p = dir.path();
        let (code, stderr) = run(p, &["renumber"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(!p.join("docs/adr/0017-b.md").exists());
        assert_eq!(
            read(p, "docs/adr/0018-b.md"),
            B.replace("# ADR 0017:", "# ADR 0018:")
        );
        assert_eq!(read(p, "docs/adr/0017-a.md"), A);
        assert_eq!(
            read(p, "README.md"),
            "[ADR 0018](docs/adr/0018-b.md) と [A](docs/adr/0017-a.md)\n"
        );
        assert!(read(p, "docs/adr/0016-x.md").contains("[ADR 0018](0018-b.md) を参照"));
        assert_eq!(run(p, &["check-numbers"]).0, 0);
        assert_eq!(run(p, &["links"]).0, 0);
    }

    #[test]
    fn renumber_does_not_warn_about_titles_of_adrs() {
        // ADR自身のタイトルの行（# ADR 0017: a）は参照ではない
        let dir = duplicate_repo();
        let p = dir.path();
        write(
            p,
            "docs/adr/0016-x.md",
            "# ADR 0016: x\n\nステータス: 採択\n\nADR 0017 を参照\n",
        );
        let (code, stderr) = run(p, &["renumber"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(!stderr.contains("0017-a.md:1:"), "{stderr}");
        assert!(
            stderr.contains(
                "docs/adr/0016-x.md:5: ADR 0017 はどちらのADRを指すか分からないため、直していません"
            ),
            "{stderr}"
        );
    }

    #[test]
    fn allow_renumber_permits_the_renumbering_commit() {
        let dir = duplicate_repo();
        let p = dir.path();
        let base = head(p);
        run(p, &["renumber"]);
        git(p, &["add", "-A"]);
        let checks = ["pre-commit", "--allow-status", "--allow-links"];
        assert_eq!(run(p, &checks).0, 1);
        let (code, stderr) = run(
            p,
            &[
                "pre-commit",
                "--allow-status",
                "--allow-links",
                "--allow-renumber",
            ],
        );
        assert_eq!(code, 0, "{stderr}");
        git(p, &["commit", "-q", "--no-verify", "-m", "renumber"]);
        let head = head(p);
        let args = [
            "check-range",
            "--allow-status",
            "--allow-links",
            "--allow-renumber",
            &base,
            &head,
        ];
        assert_eq!(run(p, &args).0, 0);
        let args = [
            "check-range",
            "--allow-status",
            "--allow-links",
            &base,
            &head,
        ];
        assert_eq!(run(p, &args).0, 1);
    }

    #[test]
    fn allow_renumber_rejects_other_renames() {
        // 重複していないADRのファイル名を変える
        let dir = repo();
        let p = dir.path();
        git(p, &["mv", ADR, "docs/adr/0002-x.md"]);
        let args = [
            "pre-commit",
            "--allow-status",
            "--allow-links",
            "--allow-renumber",
        ];
        assert_eq!(run(p, &args).0, 1);

        // 振りなおしたADRの本文も変える
        let dir = duplicate_repo();
        let p = dir.path();
        run(p, &["renumber"]);
        let changed = read(p, "docs/adr/0018-b.md").replace("B の本文", "改変");
        write(p, "docs/adr/0018-b.md", &changed);
        git(p, &["add", "-A"]);
        assert_eq!(run(p, &args).0, 1);
    }

    #[test]
    fn rejects_wrong_arguments() {
        let dir = repo();
        for args in [
            &["check-numbers", "x"][..],
            &["renumber", "x"],
            &["renumber", "--fix"],
        ] {
            assert_eq!(run(dir.path(), args).0, 64, "{args:?}");
        }
    }

    #[test]
    fn help_mentions_renumbering() {
        let output = Command::new(BIN).arg("--help").output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        for word in ["check-numbers", "renumber", "--allow-renumber"] {
            assert!(stdout.contains(word), "{word}: {stdout}");
        }
    }
}

/// ADR 0019: 修飾子の付いたADRの参照。
mod qualified_links {
    use super::*;

    fn adr(number: &str, body: &str) -> String {
        format!("# ADR {number}: x\n\nステータス: 採択\n\n{body}\n")
    }

    fn run(dir: &Path, args: &[&str]) -> (i32, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn read(dir: &Path, path: &str) -> String {
        std::fs::read_to_string(dir.join(path)).unwrap()
    }

    const WRONG_0172: &str = "（golem-driver [ADR 0015](0015-regex-styles.md)）";
    const WRONG_0003: &str =
        "golem-doc [ADR 0008](../../adr/0008-first-release-verification.md) が";

    /// tett23/golem と同じ配置のADRと、誤ったリンクをコミットした一時リポジトリ。
    fn golem_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        for (path, number, body) in [
            (
                "docs/adr/0008-first-release-verification.md",
                "0008",
                "本文",
            ),
            ("docs/adr/0015-regex-styles.md", "0015", "本文"),
            ("docs/adr/0187-x.md", "0187", "本文"),
            (
                "docs/golem-driver/adr/0015-vertical-dashes.md",
                "0015",
                "本文",
            ),
            (
                "docs/golem-doc/adr/0008-multiple-index-and-bibliography.md",
                "0008",
                "本文",
            ),
            (
                "docs/adr/0172-continuous-double-dashes.md",
                "0172",
                WRONG_0172,
            ),
            (
                "docs/golem-html/adr/0003-cite-links-across-files.md",
                "0003",
                WRONG_0003,
            ),
        ] {
            write(p, path, &adr(number, body));
        }
        write(
            p,
            "docs/specifications.md",
            "docs/adr ADR 0187\n\ngolem-driver ADR 0015\n\ngolem-nope ADR 0001\n",
        );
        git(p, &["add", "."]);
        git(p, &["commit", "-qm", "init"]);
        dir
    }

    #[test]
    fn links_reports_qualified_references() {
        let dir = golem_repo();
        let (code, stderr) = run(dir.path(), &["links"]);
        assert_eq!(code, 1, "{stderr}");
        for expected in [
            "docs/adr/0172-continuous-double-dashes.md:5: golem-driver ADR 0015 のリンク先 0015-regex-styles.md は、docs/golem-driver/adr/0015-vertical-dashes.md ではありません",
            "docs/golem-html/adr/0003-cite-links-across-files.md:5: golem-doc ADR 0008 のリンク先 ../../adr/0008-first-release-verification.md は、docs/golem-doc/adr/0008-multiple-index-and-bibliography.md ではありません",
            "docs/specifications.md:1: docs/adr ADR 0187 にリンクがありません",
            "docs/specifications.md:3: golem-driver ADR 0015 にリンクがありません",
            "docs/specifications.md:5: golem-nope ADR 0001: golem-nope のADRのディレクトリがありません",
        ] {
            assert!(stderr.contains(expected), "{expected}: {stderr}");
        }
    }

    #[test]
    fn links_fix_rewrites_qualified_references() {
        let dir = golem_repo();
        let p = dir.path();
        let (code, stderr) = run(p, &["links", "--fix", "--include-committed"]);
        assert_eq!(code, 1, "{stderr}");
        assert!(
            stderr.contains("docs/specifications.md:5: golem-nope ADR 0001: golem-nope のADRのディレクトリがありません"),
            "{stderr}"
        );
        assert_eq!(
            read(p, "docs/adr/0172-continuous-double-dashes.md"),
            adr(
                "0172",
                "（[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)）"
            )
        );
        assert_eq!(
            read(p, "docs/golem-html/adr/0003-cite-links-across-files.md"),
            adr(
                "0003",
                "[golem-doc ADR 0008](../../golem-doc/adr/0008-multiple-index-and-bibliography.md) が"
            )
        );
        assert_eq!(
            read(p, "docs/specifications.md"),
            "[docs/adr ADR 0187](adr/0187-x.md)\n\n[golem-driver ADR 0015](golem-driver/adr/0015-vertical-dashes.md)\n\ngolem-nope ADR 0001\n"
        );

        // 直したコミット済みのADRは --allow-links で許可される
        git(p, &["add", "docs/adr", "docs/golem-html"]);
        let (code, stderr) = run(p, &["pre-commit", "--allow-links"]);
        assert_eq!(code, 0, "{stderr}");
        let (code, _) = run(p, &["pre-commit"]);
        assert_eq!(code, 1);

        write(
            p,
            "docs/specifications.md",
            &read(p, "docs/specifications.md").replace("golem-nope ADR 0001\n", ""),
        );
        let (code, stderr) = run(p, &["links"]);
        assert_eq!((code, stderr.as_str()), (0, ""));
    }

    #[test]
    fn pre_commit_rejects_wrong_qualified_links() {
        let dir = golem_repo();
        let p = dir.path();
        write(
            p,
            "notes.md",
            "golem-driver [ADR 0015](docs/adr/0015-regex-styles.md)\n",
        );
        git(p, &["add", "notes.md"]);
        let (code, stderr) = run(p, &["pre-commit", "--allow-links"]);
        assert_eq!(code, 1, "{stderr}");
        assert!(
            stderr.contains("notes.md:1: golem-driver ADR 0015 のリンク先 docs/adr/0015-regex-styles.md は、docs/golem-driver/adr/0015-vertical-dashes.md ではありません"),
            "{stderr}"
        );
    }
}

/// ADR 0020: シンボリックリンクの文書。
#[cfg(unix)]
mod symlinked_documents {
    use super::*;

    const DIAGNOSTICS: &str = "[golem-cli ADR 0005](golem-cli/adr/0005-x.md) を参照\n";

    fn run(dir: &Path, args: &[&str]) -> (i32, String) {
        let output = Command::new(BIN)
            .args(args)
            .current_dir(dir)
            .env_remove("ADR_GUARD_ALLOW")
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn read(dir: &Path, path: &str) -> String {
        std::fs::read_to_string(dir.join(path)).unwrap()
    }

    fn symlink(dir: &Path, target: &str, path: &str) {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, path).unwrap();
    }

    fn commit_at(dir: &Path, message: &str, date: &str) {
        let status = Command::new("git")
            .args(["commit", "-qm", message])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .current_dir(dir)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// `docs/diagnostics.md` と、それへのシンボリックリンク `modules/m/diagnostics.md` をコミットした一時リポジトリ。
    fn symlink_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        write(
            p,
            "docs/golem-cli/adr/0005-x.md",
            "# ADR 0005: x\n\nステータス: 採択\n\n本文\n",
        );
        write(p, "docs/diagnostics.md", DIAGNOSTICS);
        symlink(p, "../../docs/diagnostics.md", "modules/m/diagnostics.md");
        git(p, &["add", "."]);
        commit_at(p, "init", "2026-01-02T00:00:00");
        dir
    }

    #[test]
    fn links_skips_symlinks() {
        let dir = symlink_repo();
        assert_eq!(run(dir.path(), &["links"]), (0, String::new()));
    }

    #[test]
    fn links_fix_writes_only_the_real_file() {
        let dir = symlink_repo();
        let p = dir.path();
        write(
            p,
            "docs/diagnostics.md",
            &format!("{DIAGNOSTICS}golem-cli ADR 0005\n"),
        );
        let (code, stderr) = run(p, &["links"]);
        assert_eq!(code, 1, "{stderr}");
        assert!(
            stderr.contains("docs/diagnostics.md:2: golem-cli ADR 0005 にリンクがありません"),
            "{stderr}"
        );
        assert!(!stderr.contains("modules/m"), "{stderr}");

        let (code, stderr) = run(p, &["links", "--fix"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(!stderr.contains("modules/m"), "{stderr}");
        assert_eq!(
            read(p, "docs/diagnostics.md"),
            format!("{DIAGNOSTICS}[golem-cli ADR 0005](golem-cli/adr/0005-x.md)\n")
        );
        assert_eq!(run(p, &["links"]), (0, String::new()));
    }

    #[test]
    fn pre_commit_skips_staged_symlinks() {
        let dir = symlink_repo();
        let p = dir.path();
        // インデックスのシンボリックリンクの内容はリンク先のパス。参照に見える文字列でも検査しない
        symlink(p, "golem-cli ADR 0005", "modules/n/diagnostics.md");
        git(p, &["add", "modules/n/diagnostics.md"]);
        let (code, stderr) = run(p, &["pre-commit", "--allow-status", "--allow-links"]);
        assert_eq!(code, 0, "{stderr}");
    }

    #[test]
    fn renumber_skips_symlinks() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.name", "test"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        write(
            p,
            "docs/golem-cli/adr/0005-a.md",
            "# ADR 0005: a\n\nステータス: 採択\n\n本文\n",
        );
        git(p, &["add", "."]);
        commit_at(p, "a", "2026-01-01T00:00:00");
        write(
            p,
            "docs/golem-cli/adr/0005-x.md",
            "# ADR 0005: x\n\nステータス: 採択\n\n本文\n",
        );
        // 2行めのリンクは、シンボリックリンクの場所（docs/golem-cli）からだけ解決できる
        write(
            p,
            "docs/diagnostics.md",
            &format!("{DIAGNOSTICS}[b](adr/0005-x.md)\n"),
        );
        symlink(p, "../diagnostics.md", "docs/golem-cli/d.md");
        git(p, &["add", "."]);
        commit_at(p, "x", "2026-01-02T00:00:00");

        let (code, stderr) = run(p, &["renumber"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("1件の参照を直しました"), "{stderr}");
        assert_eq!(
            read(p, "docs/diagnostics.md"),
            "[golem-cli ADR 0005](golem-cli/adr/0006-x.md) を参照\n[b](adr/0005-x.md)\n"
        );
    }
}

/// ADR 0021: Codex の PreToolUse hook。
mod codex_hook {
    use super::*;
    use serde_json::{Value, json};

    const AUDIT: &str = "docs/audit/a.md";

    fn codex_repo() -> TempDir {
        let dir = repo();
        let p = dir.path();
        write(p, AUDIT, "ステータス: 提案\n結果\n");
        git(p, &["add", "."]);
        git(p, &["commit", "-qm", "audit"]);
        dir
    }

    fn run_hook(payload: &Value, options: &[&str], cwd: &Path) -> (i32, String) {
        let mut child = Command::new(BIN)
            .arg("codex-hook")
            .args(options)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn patch(body: &str) -> String {
        format!("*** Begin Patch\n{body}*** End Patch\n")
    }

    fn apply_patch(dir: &Path, cwd: &Path, body: &str) -> Value {
        let _ = dir;
        json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "apply_patch",
            "tool_input": {"command": patch(body)},
            "cwd": cwd,
        })
    }

    fn hook(dir: &Path, body: &str, options: &[&str]) -> (i32, String) {
        run_hook(&apply_patch(dir, dir, body), options, dir)
    }

    fn update(path: &str, lines: &str) -> String {
        format!("*** Update File: {path}\n@@\n{lines}")
    }

    const STATUS: &str = " # ADR 0001: x\n \n-ステータス: 提案\n+ステータス: 採択\n";
    const BODY: &str = "-本文\n+改変\n";

    #[test]
    fn status_edit_needs_allow_status() {
        let dir = codex_repo();
        let p = dir.path();
        assert_eq!(hook(p, &update(ADR, STATUS), &["--allow-status"]).0, 0);
        assert_eq!(hook(p, &update(ADR, STATUS), &[]).0, 2);
    }

    #[test]
    fn rejects_body_edit() {
        let dir = codex_repo();
        let (code, stderr) = hook(dir.path(), &update(ADR, BODY), &["--allow-status"]);
        assert_eq!(code, 2);
        assert!(stderr.contains(ADR), "{stderr}");
        assert!(
            stderr.contains("仕様を変更する場合は、新しいADRを作成してください。"),
            "{stderr}"
        );
    }

    #[test]
    fn add_file_compares_whole_content() {
        let dir = codex_repo();
        let p = dir.path();
        let add = |content: &str| {
            let lines: String = content.lines().map(|l| format!("+{l}\n")).collect();
            format!("*** Add File: {ADR}\n{lines}")
        };
        assert_eq!(hook(p, &add(CONTENT), &[]).0, 0);
        assert_eq!(
            hook(
                p,
                &add(&CONTENT.replace("ステータス: 提案", "Status: accepted")),
                &["--allow-status"]
            )
            .0,
            0
        );
        assert_eq!(
            hook(
                p,
                &add(&CONTENT.replace("本文", "改変")),
                &["--allow-status"]
            )
            .0,
            2
        );
    }

    #[test]
    fn rejects_delete() {
        let dir = codex_repo();
        let (code, stderr) = hook(
            dir.path(),
            &format!("*** Delete File: {ADR}\n"),
            &["--allow-status"],
        );
        assert_eq!(code, 2);
        assert!(
            stderr.contains(&format!("コミット済みのADRは削除・移動できません: {ADR}")),
            "{stderr}"
        );
    }

    #[test]
    fn rejects_moves_from_and_to_committed_adrs() {
        let dir = codex_repo();
        let p = dir.path();
        write(p, "notes.md", "x\n");
        let from = format!("*** Update File: {ADR}\n*** Move to: docs/adr/0009-y.md\n");
        let to = format!("*** Update File: notes.md\n*** Move to: {ADR}\n");
        assert_eq!(hook(p, &from, &["--allow-status"]).0, 2);
        assert_eq!(hook(p, &to, &["--allow-status"]).0, 2);
    }

    #[test]
    fn allows_uncommitted_adrs_and_other_files() {
        let dir = codex_repo();
        let p = dir.path();
        write(p, "docs/adr/0002-b.md", "本文\n");
        write(p, "README.md", "本文\n");
        assert_eq!(hook(p, &update("docs/adr/0002-b.md", BODY), &[]).0, 0);
        assert_eq!(hook(p, &update("README.md", BODY), &[]).0, 0);
    }

    #[test]
    fn rejects_record_status_edit() {
        let dir = codex_repo();
        let lines = "-ステータス: 提案\n+ステータス: 採択\n 結果\n";
        let (code, stderr) = hook(dir.path(), &update(AUDIT, lines), &["--allow-status"]);
        assert_eq!(code, 2);
        assert!(stderr.contains(AUDIT), "{stderr}");
        assert!(
            stderr.contains("記録を直す場合は、新しい記録を作成してください。"),
            "{stderr}"
        );
    }

    #[test]
    fn reports_only_violating_files() {
        let dir = codex_repo();
        let p = dir.path();
        write(p, "README.md", "本文\n");
        let body = format!("{}{}", update("README.md", BODY), update(ADR, BODY));
        let (code, stderr) = hook(p, &body, &["--allow-status"]);
        assert_eq!(code, 2);
        assert!(stderr.contains(ADR), "{stderr}");
        assert!(!stderr.contains("README.md"), "{stderr}");
    }

    #[test]
    fn resolves_paths_from_cwd_and_absolute_paths() {
        let dir = codex_repo();
        let p = dir.path();
        let docs = p.join("docs");
        let relative = apply_patch(p, &docs, &update("adr/0001-x.md", BODY));
        assert_eq!(run_hook(&relative, &["--allow-status"], p).0, 2);
        let absolute = p.join(ADR).to_string_lossy().into_owned();
        assert_eq!(hook(p, &update(&absolute, BODY), &["--allow-status"]).0, 2);
    }

    #[test]
    fn reads_patches_in_bash_heredocs() {
        let dir = codex_repo();
        let p = dir.path();
        let bash = |command: String| json!({"tool_name": "Bash", "tool_input": {"command": command}, "cwd": p});
        let heredoc = format!("apply_patch <<'EOF'\n{}EOF\n", patch(&update(ADR, BODY)));
        assert_eq!(run_hook(&bash(heredoc), &["--allow-status"], p).0, 2);
        assert_eq!(
            run_hook(&bash("ls".to_owned()), &["--allow-status"], p).0,
            0
        );
    }

    #[test]
    fn ignores_unreadable_input() {
        let dir = codex_repo();
        let p = dir.path();
        let mut child = Command::new(BIN)
            .arg("codex-hook")
            .current_dir(p)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"{").unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(0));

        let read = json!({"tool_name": "Read", "tool_input": {"command": patch(&update(ADR, BODY))}, "cwd": p});
        assert_eq!(run_hook(&read, &[], p).0, 0);

        let unterminated = json!({"tool_name": "apply_patch",
            "tool_input": {"command": format!("*** Begin Patch\n{}", update(ADR, BODY))}, "cwd": p});
        assert_eq!(run_hook(&unterminated, &[], p).0, 0);

        let outside = TempDir::new().unwrap();
        let elsewhere = apply_patch(p, outside.path(), &update(ADR, BODY));
        assert_eq!(run_hook(&elsewhere, &[], outside.path()).0, 0);
    }
}

#[test]
fn inv6_pre_commit_transition() {
    use std::os::unix::fs::PermissionsExt;
    let repo = repo();
    let p = repo.path();
    write(
        p,
        ADR,
        &CONTENT.replace("ステータス: 提案", "Status: proposed"),
    );
    git(p, &["add", ADR]);
    let hook = p.join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\nexec '{}' pre-commit --allow-status\n", BIN),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    git(p, &["commit", "-qm", "migrate"]);
    for state in ["accepted-pending", "accepted"] {
        write(
            p,
            ADR,
            &CONTENT.replace("ステータス: 提案", &format!("Status: {state}")),
        );
        git(p, &["add", ADR]);
        git(p, &["commit", "-qm", state]);
    }
    let head = || {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(p)
            .output()
            .unwrap()
            .stdout
    };
    let accepted = head();
    write(
        p,
        ADR,
        &CONTENT.replace("ステータス: 提案", "Status: proposed"),
    );
    git(p, &["add", ADR]);
    write(
        p,
        ADR,
        &CONTENT.replace("ステータス: 提案", "Status: accepted"),
    );
    let rejected = Command::new("git")
        .args(["commit", "-qm", "invalid"])
        .current_dir(p)
        .env("ADR_GUARD_ALLOW", "intentional")
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(head(), accepted);
    let message = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        message.contains("accepted") && message.contains("proposed") && message.contains(ADR),
        "{message}"
    );
    git(
        p,
        &[
            "commit",
            "--no-verify",
            "-qm",
            "invalid\n\nADR-Guard-Allow: intentional",
        ],
    );
    assert!(
        !Command::new(BIN)
            .args(["check-range", "--allow-status", "HEAD^", "HEAD"])
            .current_dir(p)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn inv1_propose_new_adr() {
    for initialized in [false, true] {
        for (line, valid) in [
            ("Status: proposed", true),
            ("Status: accepted", false),
            ("Status: absent", false),
            ("ステータス: 提案", false),
            ("body", false),
        ] {
            let dir = TempDir::new().unwrap();
            let p = dir.path();
            git(p, &["init", "-q"]);
            git(p, &["config", "user.email", "test@example.com"]);
            git(p, &["config", "user.name", "test"]);
            if initialized {
                git(
                    p,
                    &[
                        "-c",
                        "commit.gpgSign=false",
                        "commit",
                        "--allow-empty",
                        "-qm",
                        "base",
                    ],
                );
            }
            write(p, ADR, line);
            git(p, &["add", ADR]);
            assert_eq!(pre_commit(p), valid, "{line}");
            git(
                p,
                &["-c", "commit.gpgSign=false", "commit", "-qm", "initial ADR"],
            );
            let base = if initialized {
                "HEAD^"
            } else {
                "0000000000000000000000000000000000000000"
            };
            assert_eq!(
                Command::new(BIN)
                    .args(["check-range", base, "HEAD"])
                    .current_dir(p)
                    .output()
                    .unwrap()
                    .status
                    .success(),
                valid,
                "{line}"
            );
        }
    }
}

#[test]
fn inv4_new_syntax() {
    let dir = repo();
    let p = dir.path();
    for (line, valid) in [
        ("ステータス: 提案", true),
        ("Status: 提案", false),
        ("ステータス: 採択", false),
        ("Status: Proposed", false),
        ("Status: proposed", true),
    ] {
        write(p, ADR, &format!("{line}\n\n本文を編集\n"));
        git(p, &["add", ADR]);
        let result = Command::new(BIN)
            .arg("pre-commit")
            .arg("--allow-status")
            .current_dir(p)
            .env("ADR_GUARD_ALLOW", "本文の訂正")
            .output()
            .unwrap();
        assert_eq!(result.status.success(), valid, "{line}");
    }
}

#[cfg(unix)]
#[test]
fn invalid_utf8_adr_name_cannot_hide_deletion() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = repo();
    let p = dir.path();
    let path = OsStr::from_bytes(b"docs/adr/0002-\xff.md");
    let blob = Command::new("git")
        .args(["rev-parse", &format!("HEAD:{ADR}")])
        .current_dir(p)
        .output()
        .unwrap();
    let blob = std::str::from_utf8(&blob.stdout).unwrap().trim();
    let update = |args: &[&str]| {
        Command::new("git")
            .arg("update-index")
            .args(args)
            .arg(path)
            .current_dir(p)
            .status()
            .unwrap()
    };
    assert!(update(&["--add", "--cacheinfo", "100644", blob]).success());
    git(p, &["commit", "-qm", "existing ADR"]);
    assert!(update(&["--force-remove", "--"]).success());
    assert!(!pre_commit(p));
}
