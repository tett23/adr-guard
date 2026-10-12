//! コミット済みのADR（docs/adr/、docs/<モジュール>/adr/）を、ステータスの行を除いて変更できないようにするガード。
//!
//! - `adr-guard pre-commit`: gitのpre-commit hookとして、ステージされた変更を検査する。
//! - `adr-guard claude-hook`: Claude CodeのPreToolUse hookとして、Write/Edit/MultiEditを検査する。
//! - `adr-guard check-range <base> <head>`: CIで、範囲内の各コミットを検査する。
//! - `adr-guard list [--status]`、`adr-guard proposed [--status]`: ADRと、ステータスが提案のADRを一覧する（ADR 0005、0006、0007）。
//! - `adr-guard --version`、`adr-guard --help`: バージョンとヘルプを表示する（ADR 0003）。

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

const STATUS_PREFIX: &str = "ステータス:";
const PROPOSED: &str = "提案";
/// 意図したADRの書きかえを許可するトレーラーのキー（CI）と環境変数（pre-commit）（ADR 0008）。
const ALLOW_TRAILER: &str = "ADR-Guard-Allow";
const ALLOW_ENV: &str = "ADR_GUARD_ALLOW";
const ADVICE: &str = "仕様を変更する場合は、新しいADRを作成してください。";
const USAGE: &str = "usage: adr-guard <pre-commit|claude-hook|check-range <base> <head>|list [--status]|proposed [--status]|--version|--help>";
const HELP: &str = "\
コミット済みのADR（docs/adr/、docs/<モジュール>/adr/）を、ステータスの行を除いて変更できないようにする。

使い方:
  adr-guard pre-commit                 gitのpre-commit hookとして、ステージされた変更を検査する
  adr-guard claude-hook                Claude CodeのPreToolUse hookとして、Write・Edit・MultiEditを検査する
  adr-guard check-range <base> <head>  CIで、<base>..<head> の各コミットを検査する
  adr-guard list [--status]            ADRのパスを一覧する
                                       --status ではステータスをパスの前に付ける
  adr-guard proposed [--status]        ステータスが提案のADRのパスを一覧する
                                       --status ではステータスをパスの前に付ける
  adr-guard --version, -V              バージョンを表示する
  adr-guard --help, -h                 このヘルプを表示する";

fn version() -> String {
    format!("adr-guard {}", env!("CARGO_PKG_VERSION"))
}

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
        ["list"] => list(false),
        ["list", "--status"] => list(true),
        ["proposed"] => proposed(false),
        ["proposed", "--status"] => proposed(true),
        ["--version" | "-V"] => {
            println!("{}", version());
            ExitCode::SUCCESS
        }
        ["--help" | "-h"] => {
            println!("{}\n{HELP}", version());
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}

/// リポジトリのルートからの相対パスが、`docs/adr/` か `docs/<モジュール>/adr/` 以下にあるか（ADR 0002）。
fn is_adr_path(path: &str) -> bool {
    match path.split('/').collect::<Vec<_>>().as_slice() {
        ["docs", "adr", _, ..] => true,
        ["docs", module, "adr", _, ..] => !module.is_empty(),
        _ => false,
    }
}

/// 最初のステータスの行の、`ステータス:` より後ろの前後の空白を除いた値。
fn status(content: &str) -> Option<&str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix(STATUS_PREFIX))
        .map(str::trim)
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
/// ADR以外のパスは無視する。
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
        if !is_adr_path(path) {
            continue;
        }
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
        ],
        cwd,
    ) else {
        eprintln!("pre-commit: ステージされた変更を取得できませんでした");
        return ExitCode::FAILURE;
    };

    let violations = violations(&diff, "HEAD", "", cwd);
    if violations.is_empty() {
        return ExitCode::SUCCESS;
    }
    let allowed = std::env::var(ALLOW_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(reason) = allowed {
        eprintln!("pre-commit: {ALLOW_ENV} により許可しました: {reason}");
        return ExitCode::SUCCESS;
    }
    for violation in &violations {
        eprintln!("pre-commit: {violation}");
    }
    eprintln!("{ADVICE}");
    ExitCode::FAILURE
}

/// コミットメッセージのトレーラー `ADR-Guard-Allow` の、空でない最初の値（ADR 0008）。
fn allow_reason(commit: &str, cwd: &Path) -> Option<String> {
    let format = format!("--format=%(trailers:key={ALLOW_TRAILER},valueonly,separator=%x00)");
    let values = git(&["log", "-1", &format, commit], cwd)?;
    String::from_utf8_lossy(&values)
        .split('\0')
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_owned)
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
            ],
            cwd,
        ) else {
            eprintln!("check-range: {commit}: 差分を取得できませんでした");
            failed = true;
            continue;
        };
        let short = &commit[..commit.len().min(12)];
        let violations = violations(&diff, &parent, commit, cwd);
        if violations.is_empty() {
            continue;
        }
        if let Some(reason) = allow_reason(commit, cwd) {
            eprintln!("check-range: {short}: {ALLOW_TRAILER} により許可しました: {reason}");
            continue;
        }
        for violation in violations {
            eprintln!("check-range: {short}: {violation}");
            failed = true;
        }
    }

    if failed {
        eprintln!("{ADVICE}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// 作業ツリーにあるADR（`.md`）の、リポジトリのルートとルートからの相対パスの昇順の一覧。
/// 追跡されているファイルと、追跡されていないが無視もされていないファイルを対象にする。
fn adr_files() -> Option<(PathBuf, Vec<String>)> {
    let top = git(&["rev-parse", "--show-toplevel"], Path::new("."))?;
    let root = PathBuf::from(String::from_utf8_lossy(&top).trim());
    let files = git(
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        &root,
    )?;
    let mut paths: Vec<String> = files
        .split(|&b| b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .filter(|path| is_adr_path(path) && path.ends_with(".md") && root.join(path).is_file())
        .collect();
    paths.sort();
    paths.dedup();
    Some((root, paths))
}

/// 行を標準出力に書く。パイプの先が閉じていても失敗にしない。
fn print_lines(lines: impl Iterator<Item = String>) -> ExitCode {
    let out: String = lines.map(|line| line + "\n").collect();
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    ExitCode::SUCCESS
}

/// `include` を満たすステータスのADRを出力する。`with_status` なら `<ステータス><タブ><パス>`、
/// そうでなければパスだけを出力する。ステータスの行が無ければステータスを空にする（ADR 0007）。
fn print_adrs(
    command: &str,
    with_status: bool,
    include: impl Fn(Option<&str>) -> bool,
) -> ExitCode {
    let Some((root, paths)) = adr_files() else {
        eprintln!("{command}: ADRを列挙できませんでした");
        return ExitCode::FAILURE;
    };
    print_lines(paths.into_iter().filter_map(|path| {
        let content = std::fs::read(root.join(&path))
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        let status = status(&content);
        include(status).then(|| {
            if with_status {
                format!("{}\t{path}", status.unwrap_or_default())
            } else {
                path
            }
        })
    }))
}

fn list(with_status: bool) -> ExitCode {
    print_adrs("list", with_status, |_| true)
}

fn proposed(with_status: bool) -> ExitCode {
    print_adrs("proposed", with_status, |status| {
        status.is_some_and(|s| s.starts_with(PROPOSED))
    })
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
    if !is_adr_path(&rel_path) {
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
    fn adr_paths() {
        let adr = [
            "docs/adr/0001-a.md",
            "docs/adr/adr/a.md",
            "docs/server/adr/0001-a.md",
            "docs/server/adr/images/a.png",
        ];
        for path in adr {
            assert!(is_adr_path(path), "{path}");
        }
    }

    #[test]
    fn non_adr_paths() {
        let non_adr = [
            "docs/adr",
            "docs/adr.md",
            "docs/server/adr",
            "docs/server/adr.md",
            "docs/server/adrs/0001-a.md",
            "docs/server/other/a.md",
            "docs/a/b/adr/0001-a.md",
            "docs//adr/0001-a.md",
            "docs/specifications.md",
            "src/docs/server/adr/a.md",
            "packages/server/docs/adr/0001-a.md",
            "README.md",
        ];
        for path in non_adr {
            assert!(!is_adr_path(path), "{path}");
        }
    }

    #[test]
    fn status_values() {
        let cases = [
            ("# t\n\nステータス: 提案\n\n本文\n", Some("提案")),
            ("ステータス:提案\n", Some("提案")),
            (
                "ステータス: 提案（レビュー中） \n",
                Some("提案（レビュー中）"),
            ),
            ("ステータス: 提案\r\n", Some("提案")),
            ("ステータス: 採択\nステータス: 提案\n", Some("採択")),
            ("本文\n", None),
            ("ステータス：提案\n", None),
            ("前置き ステータス: 提案\n", None),
        ];
        for (content, expected) in cases {
            assert_eq!(status(content), expected, "{content:?}");
        }
    }

    #[test]
    fn non_proposed_statuses() {
        for content in [
            "ステータス: 採択\n",
            "ステータス: 採択→ADRのディレクトリはADR 0002\n",
            "ステータス: 却下\n",
            "ステータス: 破棄\n",
        ] {
            assert!(!status(content).unwrap().starts_with(PROPOSED), "{content}");
        }
    }

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
