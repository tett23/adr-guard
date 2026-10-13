//! コミット済みのADR（docs/adr/、docs/<モジュール>/adr/）を変更できないようにするガード。ステータスの行とリンクの変更はオプションで許可する（ADR 0010）。
//!
//! - `adr-guard pre-commit [--allow-status] [--allow-links]`: gitのpre-commit hookとして、ステージされた変更を検査する。
//! - `adr-guard claude-hook [--allow-status]`: Claude CodeのPreToolUse hookとして、Write/Edit/MultiEditを検査する。
//! - `adr-guard check-range [--allow-status] [--allow-links] <base> <head>`: CIで、範囲内の各コミットを検査する。
//! - `adr-guard list [--status]`、`adr-guard proposed [--status]`: ADRと、ステータスが提案のADRを一覧する（ADR 0005、0006、0007）。
//! - `adr-guard links [--fix [--include-committed]]`: リンクの無いADRの参照を検査・修正する（ADR 0009）。
//! - `adr-guard --version`、`adr-guard --help`: バージョンとヘルプを表示する（ADR 0003、0009）。

mod links;

use std::collections::HashSet;
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
const LINKS_ADVICE: &str = "adr-guard links --fix で修正できます。コミット済みのADRは --fix --include-committed で修正し、検査に --allow-links を付けてコミットしてください。";
const USAGE: &str = "usage: adr-guard <pre-commit [--allow-status] [--allow-links]|claude-hook [--allow-status]|check-range [--allow-status] [--allow-links] <base> <head>|list [--status]|proposed [--status]|links [--fix [--include-committed]]|--version|--help>";
const HELP: &str = "\
コミット済みのADR（Architecture Decision Record）を変更できないようにするツール。
ステータスの行の変更（--allow-status）と、ADRの参照のリンクだけの変更（--allow-links）は、オプションで許可できる。
gitのpre-commit hook、Claude CodeのPreToolUse hook、CI（GitHub Actions）の3か所で検査する。

ADRとステータス:
  ADRは、リポジトリのルートからの相対パスが docs/adr/ または docs/<モジュール>/adr/ 以下にあるファイル。
  <モジュール> は1段だけ（docs/a/b/adr/ は対象外）。一覧や参照の検査では、拡張子が .md のものを扱う。
  行頭が「ステータス:」の最初の行をステータスの行とする。値の例:
    提案                               レビュー待ち。コミットせずに置く
    採択                               採択済み
    採択→<変わった部分>は[ADR NNNN](...)  一部が後のADRで変更された
    却下、破棄                         採択されなかった、取りやめた

規則:
  - コミット済みのADRは変更できない。削除・移動（リネーム）もできない。
    ただし、検査に --allow-status を付けるとステータスの行の変更を、--allow-links を付けると
    ADRの参照のリンクだけの変更を許可する。ADRの採択などでステータスを変えるため、通常は両方を付ける。
  - 新しいADRの追加と、未コミットのADRの編集は自由にできる。
  - 仕様を変える場合は、元のADRを書きかえずに、新しいADRを作成する。
  - 文書（.md）の中のADRの参照（ADR 0001 の形）には、ADRのファイルへのリンクを付ける。
    例: [ADR 0001](docs/adr/0001-xxx.md)、続く番号は [ADR 0005](...)、[0006](...)
    インラインのコード、コードブロック、リンクのテキストの中と、ADR自身の番号は対象外。
    参照先は、docs/<モジュール>/ 以下の文書なら docs/<モジュール>/adr/、ほかは docs/adr/ から探す。
    参照先のADRが見つからない番号は検査しない。

サブコマンド:
  adr-guard pre-commit [--allow-status] [--allow-links]
    gitのpre-commit hookから呼ぶ。ステージされた変更を検査する。
      - コミット済みのADRの変更（オプションで許可したものを除く）・削除・移動を拒否する。
      - ステージされた .md のファイルの、リンクの無いADRの参照を拒否する。
    --allow-status  コミット済みのADRで、ステータスの行の変更を許可する。
    --allow-links   コミット済みのADRで、ADRの参照のリンクだけが異なる変更（リンクを付ける・外す・
                    リンク先を変える）を許可する。
    終了コード: 0 問題なし、1 違反あり

  adr-guard claude-hook [--allow-status]
    Claude CodeのPreToolUse hook（matcher: Write|Edit|MultiEdit）から呼ぶ。標準入力のJSONを読み、
    コミット済みのADRを変える編集を拒否する。
    --allow-status  ステータスの行だけを変える編集を許可する。
    終了コード: 0 許可、2 拒否（理由を標準エラーに表示）

  adr-guard check-range <base> <head> [--allow-status] [--allow-links]
    CIから呼ぶ。<base>..<head> のマージ以外の各コミットを、親コミットとの差分で検査する。
    <base> がコミットとして存在しなければ（0000... など）、<head> から辿れるすべてのコミットを検査する。
    --allow-status, --allow-links  pre-commit と同じ。
    終了コード: 0 問題なし、1 違反あり

  adr-guard list [--status]
    作業ツリーにあるADR（未コミットのものも含む）のパスを、1行に1つずつ標準出力に出力する。
    --status  「<ステータス><タブ><パス>」の形で出力する。ステータスの行が無ければステータスは空。
    終了コード: 0 成功、1 gitのリポジトリの外など

  adr-guard proposed [--status]
    ステータスが「提案」で始まるADRのパスを、1行に1つずつ標準出力に出力する。レビュー待ちのADRを探すのに使う。
    --status  「<ステータス><タブ><パス>」の形で出力する。
    終了コード: 0 成功（該当なしでも0）、1 gitのリポジトリの外など

  adr-guard links [--fix [--include-committed]]
    作業ツリーの .md のファイルにある、リンクの無いADRの参照を「<パス>:<行>: ...」の形で標準エラーに表示する。
    --fix                リンクの無い参照をリンクに置きかえてファイルを書きかえる。コミット済みのADRは書きかえない。
    --include-committed  --fix と一緒に使う。コミット済みのADRも書きかえる。
                         書きかえたADRのコミットには、検査の --allow-links が必要。
    終了コード: 0 リンクの無い参照なし（--fix では、修正後に残っていない）、1 残っている

  adr-guard --version, -V   「adr-guard <バージョン>」を表示する
  adr-guard --help, -h      このヘルプを表示する

  オプションは、サブコマンドの後ろならどの位置に置いてもよい。

意図してADRを書きかえる場合:
  --allow-status         ステータスの行の変更を許可する。hook、Claude Codeのhook、CIで常に付けてよい。
  --allow-links          ADRの参照にリンクを付けるだけの変更を許可する。hookとCIで常に付けてよい。
  ADR_GUARD_ALLOW=<理由>  環境変数。pre-commit で、ADRのどんな書きかえも許可する。
                         同じhookのほかの検査は迂回しない。リンクの検査は許可しない。
  ADR-Guard-Allow: <理由>  コミットメッセージの最後の段落（トレーラー）。check-range で、そのコミットの
                         ADRのどんな書きかえも許可する。緊急用であり、通常は使わない。
  ADR_GUARD_ALLOW とトレーラーは、書きかえをほかの変更と分けた1つのコミットに付ける。

終了コード:
  0   成功
  1   違反あり、または実行の失敗（gitのリポジトリの外など）
  2   claude-hook での拒否
  64  引数の誤り（使い方の1行を標準エラーに表示する）

例:
  # .githooks/pre-commit（git config core.hooksPath .githooks）
  exec adr-guard pre-commit --allow-status --allow-links

  # Claude Codeの .claude/settings.json の PreToolUse hook の command
  adr-guard claude-hook --allow-status

  # CI（GitHub Actions）。actionを使う場合は uses: tett23/adr-guard@<タグ> と
  # with: の allow-status: true、allow-links: true
  adr-guard check-range --allow-status --allow-links \"$BASE_SHA\" \"$HEAD_SHA\"
  adr-guard links

  # レビュー待ちのADRを開く
  adr-guard proposed | xargs \"$EDITOR\"

  # リンクの無いADRの参照を修正する
  adr-guard links --fix
  adr-guard links --fix --include-committed
  git commit  # pre-commit hookに --allow-links が必要";

fn version() -> String {
    format!("adr-guard {}", env!("CARGO_PKG_VERSION"))
}

/// コミット済みのADRで許可する変更（ADR 0009、0010）。
#[derive(Clone, Copy)]
struct Allow {
    /// ステータスの行の変更（`--allow-status`）
    status: bool,
    /// ADRの参照のリンクだけの変更（`--allow-links`）
    links: bool,
}

enum Subcommand<'a> {
    PreCommit {
        allow: Allow,
    },
    ClaudeHook {
        allow: Allow,
    },
    CheckRange {
        base: &'a str,
        head: &'a str,
        allow: Allow,
    },
    List {
        status: bool,
    },
    Proposed {
        status: bool,
    },
    Links {
        fix: bool,
        include_committed: bool,
    },
    Version,
    Help,
}

/// 引数を解釈する。誤りがあればNone。オプション（`--` で始まるもの）はサブコマンドの後ろのどこにあってもよい。
fn parse_args(args: &[String]) -> Option<Subcommand<'_>> {
    let (command, rest) = args.split_first()?;
    let (options, positional): (Vec<&str>, Vec<&str>) = rest
        .iter()
        .map(String::as_str)
        .partition(|arg| arg.starts_with("--"));
    let accepted: &[&str] = match command.as_str() {
        "pre-commit" | "check-range" => &["--allow-status", "--allow-links"],
        "claude-hook" => &["--allow-status"],
        "list" | "proposed" => &["--status"],
        "links" => &["--fix", "--include-committed"],
        _ => &[],
    };
    let unique: HashSet<&str> = options.iter().copied().collect();
    if unique.len() != options.len() || !options.iter().all(|o| accepted.contains(o)) {
        return None;
    }
    let has = |option: &str| unique.contains(option);
    let allow = Allow {
        status: has("--allow-status"),
        links: has("--allow-links"),
    };
    match (command.as_str(), positional.as_slice()) {
        ("pre-commit", []) => Some(Subcommand::PreCommit { allow }),
        ("claude-hook", []) => Some(Subcommand::ClaudeHook { allow }),
        ("check-range", [base, head]) => Some(Subcommand::CheckRange { base, head, allow }),
        ("list", []) => Some(Subcommand::List {
            status: has("--status"),
        }),
        ("proposed", []) => Some(Subcommand::Proposed {
            status: has("--status"),
        }),
        ("links", []) if has("--fix") || !has("--include-committed") => Some(Subcommand::Links {
            fix: has("--fix"),
            include_committed: has("--include-committed"),
        }),
        ("--version" | "-V", []) => Some(Subcommand::Version),
        ("--help" | "-h", []) => Some(Subcommand::Help),
        _ => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Some(Subcommand::PreCommit { allow }) => pre_commit(allow),
        Some(Subcommand::ClaudeHook { allow }) => claude_hook(allow),
        Some(Subcommand::CheckRange { base, head, allow }) => check_range(base, head, allow),
        Some(Subcommand::List { status }) => list(status),
        Some(Subcommand::Proposed { status }) => proposed(status),
        Some(Subcommand::Links {
            fix,
            include_committed,
        }) => links(fix, include_committed),
        Some(Subcommand::Version) => {
            println!("{}", version());
            ExitCode::SUCCESS
        }
        Some(Subcommand::Help) => {
            println!("{}\n{HELP}", version());
            ExitCode::SUCCESS
        }
        None => {
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

#[cfg(test)]
fn only_status_changed(before: &[u8], after: &[u8]) -> bool {
    normalize(before) == normalize(after)
}

/// 変更が許可されるか。`allow.status` ならステータスの行の違いを、`allow.links` ならADRの参照の
/// リンクだけの違いを許可する。どちらも無ければ、前後がまったく同じときだけ許可する（ADR 0009、0010）。
fn allowed_change(before: &[u8], after: &[u8], allow: Allow) -> bool {
    let prepare = |content: &[u8]| {
        let content = if allow.links {
            links::unlink(&String::from_utf8_lossy(content)).into_bytes()
        } else {
            content.to_vec()
        };
        if allow.status {
            normalize(&content)
        } else {
            content
        }
    };
    prepare(before) == prepare(after)
}

/// コミット済みのADRを許可されない形で変更したときのメッセージ。
fn change_violation(path: &str, allow: Allow) -> String {
    if allow.status {
        format!("コミット済みのADRはステータスの行以外を変更できません: {path}")
    } else {
        format!(
            "コミット済みのADRは変更できません（ステータスの行の変更は --allow-status で許可できます）: {path}"
        )
    }
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
fn violations(diff: &[u8], old_ref: &str, new_ref: &str, allow: Allow, cwd: &Path) -> Vec<String> {
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
                if !allowed_change(&old, &new, allow) {
                    violations.push(change_violation(path, allow));
                }
            }
            _ => violations.push(format!("コミット済みのADRは削除・移動できません: {path}")),
        }
    }
    violations
}

fn pre_commit(allow: Allow) -> ExitCode {
    let cwd = Path::new(".");
    let adr_violations = adr_violations_in_index(allow, cwd);
    let link_findings = unlinked_in_index(cwd);
    let (Some(adr_violations), Some(link_findings)) = (adr_violations, link_findings) else {
        eprintln!("pre-commit: ステージされた変更を取得できませんでした");
        return ExitCode::FAILURE;
    };

    let allowed = std::env::var(ALLOW_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let adr_failed = match (&allowed, adr_violations.is_empty()) {
        (_, true) => false,
        (Some(reason), false) => {
            eprintln!("pre-commit: {ALLOW_ENV} により許可しました: {reason}");
            false
        }
        (None, false) => {
            for violation in &adr_violations {
                eprintln!("pre-commit: {violation}");
            }
            eprintln!("{ADVICE}");
            true
        }
    };
    for finding in &link_findings {
        eprintln!("pre-commit: {finding}");
    }
    if !link_findings.is_empty() {
        eprintln!("{LINKS_ADVICE}");
    }
    if adr_failed || !link_findings.is_empty() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// HEADとインデックスの差分のうち、ADRの規則に違反する変更。HEADが無ければ（初回コミット）空。
fn adr_violations_in_index(allow: Allow, cwd: &Path) -> Option<Vec<String>> {
    if git(&["rev-parse", "--verify", "-q", "HEAD"], cwd).is_none() {
        return Some(Vec::new());
    }
    let diff = git(
        &[
            "diff",
            "--cached",
            "--name-status",
            "--no-renames",
            "-z",
            "HEAD",
        ],
        cwd,
    )?;
    Some(violations(&diff, "HEAD", "", allow, cwd))
}

/// ステージされた追加・変更の .md のファイルの、インデックスの内容にあるリンクの無い参照（ADR 0009）。
fn unlinked_in_index(cwd: &Path) -> Option<Vec<String>> {
    let (root, files) = work_tree_files()?;
    let adrs = adr_paths(&files);
    let staged = git(
        &[
            "diff",
            "--cached",
            "--name-only",
            "--no-renames",
            "--diff-filter=AM",
            "-z",
        ],
        &root,
    )?;
    let findings = staged
        .split(|&b| b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .filter(|path| path.ends_with(".md"))
        .flat_map(|path| {
            let content = git(&["show", &format!(":{path}")], cwd).unwrap_or_default();
            let content = String::from_utf8_lossy(&content).into_owned();
            let (_, findings) = scan_document(&path, &content, &adrs);
            findings
                .into_iter()
                .map(move |f| format!("{path}:{}: ADR {} にリンクがありません", f.line, f.number))
                .collect::<Vec<_>>()
        })
        .collect();
    Some(findings)
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
fn check_range(base: &str, head: &str, allow: Allow) -> ExitCode {
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
        let violations = violations(&diff, &parent, commit, allow, cwd);
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

/// 作業ツリーにあるファイルの、リポジトリのルートとルートからの相対パスの昇順の一覧。
/// 追跡されているファイルと、追跡されていないが無視もされていないファイルを対象にする。
fn work_tree_files() -> Option<(PathBuf, Vec<String>)> {
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
        .filter(|path| !path.is_empty() && root.join(path).is_file())
        .collect();
    paths.sort();
    paths.dedup();
    Some((root, paths))
}

/// ADR（`.md`）のパス。
fn adr_paths(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|path| is_adr_path(path) && path.ends_with(".md"))
        .cloned()
        .collect()
}

/// 作業ツリーにあるADR（`.md`）の、リポジトリのルートとルートからの相対パスの昇順の一覧。
fn adr_files() -> Option<(PathBuf, Vec<String>)> {
    let (root, files) = work_tree_files()?;
    Some((root, adr_paths(&files)))
}

/// ADRのファイル名の先頭の番号（`0001-x.md` なら `0001`）。ADRでなければNone。
fn adr_number(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?;
    let number = name.get(..4)?;
    (is_adr_path(path) && number.bytes().all(|b| b.is_ascii_digit()) && name[4..].starts_with('-'))
        .then_some(number)
}

/// 文書のリンクの無い参照を調べる。`adrs` は参照先を探すADRのパスの一覧（ADR 0009）。
fn scan_document(path: &str, content: &str, adrs: &[String]) -> (String, Vec<links::Finding>) {
    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let target = |number: &str| {
        links::candidate_dirs(path)
            .into_iter()
            .find_map(|candidate| {
                let prefix = format!("{candidate}/{number}-");
                let matches: Vec<&String> = adrs
                    .iter()
                    .filter(|adr| adr.starts_with(&prefix) && !adr[prefix.len()..].contains('/'))
                    .collect();
                match matches.as_slice() {
                    [only] => Some(links::relative(dir, only)),
                    _ => None,
                }
            })
    };
    links::scan(content, adr_number(path), target)
}

/// `adr-guard links [--fix [--include-committed]]`（ADR 0009）。
fn links(fix: bool, include_committed: bool) -> ExitCode {
    let Some((root, files)) = work_tree_files() else {
        eprintln!("links: 文書を列挙できませんでした");
        return ExitCode::FAILURE;
    };
    let adrs = adr_paths(&files);
    let committed: HashSet<String> = git(&["ls-tree", "-r", "--name-only", "-z", "HEAD"], &root)
        .map(|out| {
            out.split(|&b| b == 0)
                .map(|f| String::from_utf8_lossy(f).into_owned())
                .collect()
        })
        .unwrap_or_default();

    let mut remaining = 0;
    for path in files.iter().filter(|path| path.ends_with(".md")) {
        let Ok(content) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        let (fixed, findings) = scan_document(path, &content, &adrs);
        if findings.is_empty() {
            continue;
        }
        let protected = is_adr_path(path) && committed.contains(path);
        if fix && (!protected || include_committed) {
            if std::fs::write(root.join(path), fixed).is_err() {
                eprintln!("links: 書きかえられませんでした: {path}");
                return ExitCode::FAILURE;
            }
            eprintln!("{path}: {}件の参照をリンクにしました", findings.len());
            continue;
        }
        let note = if fix {
            "（コミット済みのADR。--include-committed で修正できます）"
        } else {
            ""
        };
        for finding in &findings {
            eprintln!(
                "{path}:{}: ADR {} にリンクがありません{note}",
                finding.line, finding.number
            );
        }
        remaining += findings.len();
    }
    if remaining > 0 {
        eprintln!("{LINKS_ADVICE}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
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

fn claude_hook(allow: Allow) -> ExitCode {
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
        Some(updated) if allowed_change(&committed, updated.as_bytes(), allow) => ExitCode::SUCCESS,
        _ => {
            eprintln!("{}\n{ADVICE}", change_violation(&rel_path, allow));
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

    const NONE: Allow = Allow {
        status: false,
        links: false,
    };
    const STATUS: Allow = Allow {
        status: true,
        links: false,
    };
    const LINKS: Allow = Allow {
        status: false,
        links: true,
    };
    const BOTH: Allow = Allow {
        status: true,
        links: true,
    };

    #[test]
    fn allowed_changes_by_option() {
        let before = "ステータス: 提案\n\nADR 0001 本文\n";
        let status = "ステータス: 採択\n\nADR 0001 本文\n";
        let link = "ステータス: 提案\n\n[ADR 0001](0001-a.md) 本文\n";
        let both = "ステータス: 採択\n\n[ADR 0001](0001-a.md) 本文\n";
        let body = "ステータス: 採択\n\n[ADR 0001](0001-a.md) 改変\n";
        let cases = [
            (before, [true, true, true, true]),
            (status, [false, true, false, true]),
            (link, [false, false, true, true]),
            (both, [false, false, false, true]),
            (body, [false, false, false, false]),
        ];
        for (after, expected) in cases {
            let actual = [NONE, STATUS, LINKS, BOTH]
                .map(|allow| allowed_change(before.as_bytes(), after.as_bytes(), allow));
            assert_eq!(actual, expected, "{after}");
        }
    }

    #[test]
    fn link_only_changes() {
        let plain = "ステータス: 採択\n\nADR 0001 と ADR 0005、0006\n";
        for after in [
            "ステータス: 採択\n\n[ADR 0001](0001-a.md) と ADR 0005、0006\n",
            "ステータス: 採択\n\n[ADR 0001](0001-a.md) と [ADR 0005](0005-e.md)、[0006](0006-f.md)\n",
            "ステータス: 採択→ADR 0002\n\n[ADR 0001](0001-a.md) と ADR 0005、0006\n",
        ] {
            assert!(
                allowed_change(plain.as_bytes(), after.as_bytes(), BOTH),
                "{after}"
            );
            assert!(
                !allowed_change(plain.as_bytes(), after.as_bytes(), STATUS),
                "{after}"
            );
        }
        // リンクを外す、リンク先を変える
        let linked = "[ADR 0001](0001-a.md)\n";
        for after in ["ADR 0001\n", "[ADR 0001](../adr/0001-a.md)\n"] {
            assert!(
                allowed_change(linked.as_bytes(), after.as_bytes(), BOTH),
                "{after}"
            );
            assert!(
                !allowed_change(linked.as_bytes(), after.as_bytes(), STATUS),
                "{after}"
            );
        }
    }

    #[test]
    fn link_text_and_body_changes_are_rejected_even_with_allow_links() {
        let before = "[ADR 0001](0001-a.md) 本文\n";
        for after in [
            "[ADR 0002](0001-a.md) 本文\n",
            "[参照](0001-a.md) 本文\n",
            "[ADR 0001](0001-a.md) 改変\n",
        ] {
            assert!(
                !allowed_change(before.as_bytes(), after.as_bytes(), BOTH),
                "{after}"
            );
        }
    }

    #[test]
    fn adr_numbers() {
        let cases = [
            ("docs/adr/0001-a.md", Some("0001")),
            ("docs/server/adr/0012-b.md", Some("0012")),
            ("docs/adr/notes.md", None),
            ("docs/adr/0001.md", None),
            ("docs/0001-a.md", None),
        ];
        for (path, expected) in cases {
            assert_eq!(adr_number(path), expected, "{path}");
        }
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
