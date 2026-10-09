//! コミット済みのADR（docs/adr/）を、ステータスの行を除いて変更できないようにするガード。
//!
//! - `adr-guard pre-commit`: gitのpre-commit hookとして、ステージされた変更を検査する。
//! - `adr-guard claude-hook`: Claude CodeのPreToolUse hookとして、Write/Edit/MultiEditを検査する。
//! - `adr-guard check-range <base> <head>`: CIで、範囲内の各コミットを検査する。

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

const ADR_DIR: &str = "docs/adr/";
const STATUS_PREFIX: &str = "ステータス:";
const ADVICE: &str = "仕様を変更する場合は、新しいADRを作成してください。";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["pre-commit"] => pre_commit(),
        ["claude-hook"] => claude_hook(),
        ["check-range", base, head] => check_range(base, head),
        _ => {
            eprintln!("usage: adr-guard <pre-commit|claude-hook|check-range <base> <head>>");
            ExitCode::from(64)
        }
    }
}

/// ステータスの行を固定文字列に置きかえる。
fn normalize(content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len());
    for line in content.split_inclusive(|&b| b == b'\n') {
        if line.starts_with(STATUS_PREFIX.as_bytes()) {
            out.extend_from_slice(STATUS_PREFIX.as_bytes());
            if line.ends_with(b"\n") {
                out.push(b'\n');
            }
        } else {
            out.extend_from_slice(line);
        }
    }
    out
}

fn only_status_changed(before: &[u8], after: &[u8]) -> bool {
    normalize(before) == normalize(after)
}

fn git(args: &[&str], cwd: &Path) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// `git diff --name-status --no-renames -z` の出力から、ADRの規則に違反する変更を列挙する。
///
/// * `old_ref` - 変更前の内容を `git show <old_ref>:<path>` で読むための参照（`HEAD` やコミット）
/// * `new_ref` - 変更後の内容を `git show <new_ref>:<path>` で読むための参照（インデックスなら空文字）
fn violations(diff: &[u8], old_ref: &str, new_ref: &str, cwd: &Path) -> Vec<String> {
    let fields: Vec<String> = diff
        .split(|&b| b == 0)
        .filter(|f| !f.is_empty())
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();

    let mut violations = Vec::new();
    for pair in fields.chunks(2) {
        let [status, path] = pair else { continue };
        match status.as_str() {
            "A" => {}
            "M" => {
                let old = git(&["show", &format!("{old_ref}:{path}")], cwd).unwrap_or_default();
                let new = git(&["show", &format!("{new_ref}:{path}")], cwd).unwrap_or_default();
                if !only_status_changed(&old, &new) {
                    violations.push(format!(
                        "コミット済みのADRはステータスの行以外を変更できません: {path}"
                    ));
                }
            }
            _ => violations.push(format!("コミット済みのADRは削除・移動できません: {path}")),
        }
    }
    violations
}

fn pre_commit() -> ExitCode {
    let cwd = Path::new(".");
    if git(&["rev-parse", "--verify", "-q", "HEAD"], cwd).is_none() {
        // 初回コミットでは比較対象がない
        return ExitCode::SUCCESS;
    }

    let Some(diff) = git(
        &[
            "diff",
            "--cached",
            "--name-status",
            "--no-renames",
            "-z",
            "HEAD",
            "--",
            ADR_DIR,
        ],
        cwd,
    ) else {
        eprintln!("pre-commit: ステージされた変更を取得できませんでした");
        return ExitCode::FAILURE;
    };

    let violations = violations(&diff, "HEAD", "", cwd);
    for violation in &violations {
        eprintln!("pre-commit: {violation}");
    }
    if !violations.is_empty() {
        eprintln!("{ADVICE}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// `<base>..<head>` のマージ以外の各コミットを、親コミットとの差分で検査する。
/// `<base>` がコミットとして存在しなければ（ブランチの新規作成など）、ルートコミットから検査する。
fn check_range(base: &str, head: &str) -> ExitCode {
    let cwd = Path::new(".");
    let base_exists = git(
        &["rev-parse", "--verify", "-q", &format!("{base}^{{commit}}")],
        cwd,
    )
    .is_some();
    let range = if base_exists {
        format!("{base}..{head}")
    } else {
        head.to_owned()
    };
    let Some(commits) = git(&["rev-list", "--no-merges", "--reverse", &range], cwd) else {
        eprintln!("check-range: コミットの範囲を取得できませんでした: {range}");
        return ExitCode::FAILURE;
    };

    let mut failed = false;
    for commit in String::from_utf8_lossy(&commits).lines() {
        let parent = format!("{commit}^");
        if git(&["rev-parse", "--verify", "-q", &parent], cwd).is_none() {
            // ルートコミットはすべて新規追加
            continue;
        }
        let Some(diff) = git(
            &[
                "diff",
                "--name-status",
                "--no-renames",
                "-z",
                &parent,
                commit,
                "--",
                ADR_DIR,
            ],
            cwd,
        ) else {
            eprintln!("check-range: {commit}: 差分を取得できませんでした");
            failed = true;
            continue;
        };
        for violation in violations(&diff, &parent, commit, cwd) {
            eprintln!(
                "check-range: {}: {violation}",
                &commit[..commit.len().min(12)]
            );
            failed = true;
        }
    }

    if failed {
        eprintln!("{ADVICE}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// `..` や `.` を字句的に取り除く。存在しないパスも扱えるようにcanonicalizeは使わない。
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn resolve(path: &Path) -> PathBuf {
    let path = lexical_normalize(path);
    // シンボリックリンク（macOSの/tmpなど）を解決するため、存在する最も深い祖先をcanonicalizeする
    let mut existing = path.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            return rest.iter().rev().fold(canonical, |acc, c| acc.join(c));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return path,
        }
    }
}

fn apply_edit(text: &str, edit: &Value) -> String {
    let old = edit["old_string"].as_str().unwrap_or_default();
    let new = edit["new_string"].as_str().unwrap_or_default();
    if edit["replace_all"].as_bool().unwrap_or(false) {
        text.replace(old, new)
    } else {
        text.replacen(old, new, 1)
    }
}

/// ツール実行後のファイル内容を計算する。対象外のツールならNone。
fn simulate(tool: &str, input: &Value, current: &str) -> Option<String> {
    match tool {
        "Write" => Some(input["content"].as_str().unwrap_or_default().to_owned()),
        "Edit" => Some(apply_edit(current, input)),
        "MultiEdit" => Some(
            input["edits"]
                .as_array()
                .into_iter()
                .flatten()
                .fold(current.to_owned(), |acc, edit| apply_edit(&acc, edit)),
        ),
        _ => None,
    }
}

fn claude_hook() -> ExitCode {
    let mut stdin = String::new();
    if std::io::stdin().read_to_string(&mut stdin).is_err() {
        return ExitCode::SUCCESS;
    }
    let Ok(payload) = serde_json::from_str::<Value>(&stdin) else {
        return ExitCode::SUCCESS;
    };
    let tool = payload["tool_name"].as_str().unwrap_or_default();
    let input = &payload["tool_input"];
    let Some(file_path) = input["file_path"].as_str() else {
        return ExitCode::SUCCESS;
    };

    let project_dir = std::env::var_os("CLAUDE_PROJECT_DIR")
        .map(PathBuf::from)
        .or_else(|| payload["cwd"].as_str().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let Some(top) = git(&["rev-parse", "--show-toplevel"], &project_dir) else {
        return ExitCode::SUCCESS;
    };
    let root = resolve(Path::new(String::from_utf8_lossy(&top).trim()));

    let abs_path = resolve(&project_dir.join(file_path));
    let Ok(rel_path) = abs_path.strip_prefix(&root) else {
        return ExitCode::SUCCESS;
    };
    let rel_path = rel_path.to_string_lossy();
    if !rel_path.starts_with(ADR_DIR) {
        return ExitCode::SUCCESS;
    }

    let Some(committed) = git(&["show", &format!("HEAD:{rel_path}")], &root) else {
        // 未コミットのADRは自由に編集できる
        return ExitCode::SUCCESS;
    };

    let current = std::fs::read_to_string(&abs_path).unwrap_or_default();
    match simulate(tool, input, &current) {
        Some(updated) if only_status_changed(&committed, updated.as_bytes()) => ExitCode::SUCCESS,
        _ => {
            eprintln!(
                "コミット済みのADRはステータスの行以外を変更できません: {rel_path}\n{ADVICE}"
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_line_only_change_is_allowed() {
        assert!(only_status_changed(
            "# t\n\nステータス: 提案\n\n本文\n".as_bytes(),
            "# t\n\nステータス: 採択\n\n本文\n".as_bytes(),
        ));
    }

    #[test]
    fn body_change_is_rejected() {
        assert!(!only_status_changed(
            "ステータス: 提案\n本文\n".as_bytes(),
            "ステータス: 提案\n改変\n".as_bytes(),
        ));
    }

    #[test]
    fn trailing_newline_change_is_rejected() {
        assert!(!only_status_changed(b"a\n", b"a"));
    }

    #[test]
    fn simulate_edit_replaces_first_occurrence() {
        let input = json!({"old_string": "a", "new_string": "b"});
        assert_eq!(simulate("Edit", &input, "aa").as_deref(), Some("ba"));
    }

    #[test]
    fn simulate_edit_replace_all() {
        let input = json!({"old_string": "a", "new_string": "b", "replace_all": true});
        assert_eq!(simulate("Edit", &input, "aa").as_deref(), Some("bb"));
    }

    #[test]
    fn simulate_multi_edit_applies_sequentially() {
        let input = json!({"edits": [
            {"old_string": "a", "new_string": "b"},
            {"old_string": "b", "new_string": "c"},
        ]});
        assert_eq!(simulate("MultiEdit", &input, "a").as_deref(), Some("c"));
    }

    #[test]
    fn lexical_normalize_removes_dot_components() {
        assert_eq!(
            lexical_normalize(Path::new("/r/./x/../docs/adr/a.md")),
            PathBuf::from("/r/docs/adr/a.md"),
        );
    }
}
