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
    fn skips_without_head() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), ADR, "x\n");
        git(dir.path(), &["add", "."]);
        assert!(pre_commit(dir.path()));
    }

    #[test]
    fn allows_status_change() {
        let dir = repo();
        write(
            dir.path(),
            ADR,
            "# ADR 0001: x\n\nステータス: 採択\n\n本文\n",
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
        write(dir.path(), "docs/adr/0002-y.md", "y\n");
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
            &CONTENT.replace("提案", "採択")
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
        assert!(staged(dir.path(), "docs/client/adr/0001-y.md", "y\n"));
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
        let head = commit(dir.path(), "docs/adr/0002-y.md", "y\n");
        assert!(check_range(dir.path(), &base, &head));
    }

    #[test]
    fn allows_status_change() {
        let dir = repo();
        let base = rev(dir.path(), "HEAD");
        let head = commit(dir.path(), ADR, &ORIGINAL.replace("提案", "採択"));
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
        commit(dir.path(), MODULE_ADR, &CONTENT.replace("提案", "採択"));
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

    const ADOPTED: &str = "# ADR 0001: x\n\nステータス: 採択\n\n本文\n";

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
