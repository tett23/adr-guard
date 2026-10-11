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

fn pre_commit(dir: &Path) -> bool {
    Command::new(BIN)
        .arg("pre-commit")
        .current_dir(dir)
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

fn claude_hook(dir: &Path, payload: serde_json::Value) -> i32 {
    let mut child = Command::new(BIN)
        .arg("claude-hook")
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

    fn check_range(dir: &Path, base: &str, head: &str) -> bool {
        Command::new(BIN)
            .args(["check-range", base, head])
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
