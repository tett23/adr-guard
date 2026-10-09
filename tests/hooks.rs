use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_adr-guard");
const ADR: &str = "docs/adr/0001-x.md";

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
        write(dir.path(), ADR, "# ADR 0001: x\n\nステータス: 採択\n\n本文\n");
        git(dir.path(), &["add", "."]);
        assert!(pre_commit(dir.path()));
    }

    #[test]
    fn rejects_body_change() {
        let dir = repo();
        write(dir.path(), ADR, "# ADR 0001: x\n\nステータス: 提案\n\n改変\n");
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
        let payload = json!({"tool_name": "Write", "tool_input": {"file_path": ADR, "content": "x"}});
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
        let payload = json!({"tool_name": "Write", "tool_input": {"file_path": "README.md", "content": "x"}});
        assert_eq!(claude_hook(dir.path(), payload), 0);
    }
}
