//! One-time migration of legacy ADR status lines (ADR 0022).
#![allow(dead_code)]

#[path = "../src/links.rs"]
mod links;
#[path = "../src/revisions.rs"]
mod revisions;
#[path = "../src/status.rs"]
mod status;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

struct Rewrite {
    path: String,
    line: usize,
    before: String,
    after: String,
    original: Vec<u8>,
    content: Vec<u8>,
}

fn is_regular_adr(root: &Path, path: &str) -> bool {
    let mut current = root.to_path_buf();
    for component in path.split('/') {
        current.push(component);
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            return false;
        };
        if metadata.file_type().is_symlink() {
            return false;
        }
    }
    current.is_file()
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("git {args:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn adr_path(path: &str) -> bool {
    if !path.ends_with(".md") {
        return false;
    }
    let components: Vec<&str> = path.split('/').collect();
    components
        .windows(3)
        .any(|parts| parts[0] == "docs" && links::ADR_DIR_NAMES.contains(&parts[1]))
        || components.windows(4).any(|parts| {
            parts[0] == "docs" && !parts[1].is_empty() && links::ADR_DIR_NAMES.contains(&parts[2])
        })
}

fn files(root: &Path) -> Result<Vec<String>, String> {
    let listed = git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    let mut paths: Vec<String> = listed
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .filter(|path| adr_path(path) && is_regular_adr(root, path))
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn simple(raw: &str) -> Option<&'static str> {
    let exact = [
        ("提案", "proposed"),
        ("採択", "accepted"),
        ("採択（未実装）", "accepted-pending"),
        ("採択(未実装)", "accepted-pending"),
        ("却下", "rejected"),
        ("破棄", "discarded"),
        ("廃止", "deprecated"),
    ];
    exact
        .into_iter()
        .find_map(|(old, new)| (raw == old).then_some(new))
        .or_else(|| {
            [
                ("Proposed", "proposed"),
                ("Accepted", "accepted"),
                ("Accepted (not implemented)", "accepted-pending"),
                ("Rejected", "rejected"),
                ("Discarded", "discarded"),
                ("Deprecated", "deprecated"),
            ]
            .into_iter()
            .find_map(|(old, new)| raw.eq_ignore_ascii_case(old).then_some(new))
        })
}

fn english_superseded_has_only_references(raw: &str) -> bool {
    let text = links::link_texts(raw);
    let separated = text.replace([',', '、'], " ");
    let words: Vec<&str> = separated.split_whitespace().collect();
    let Some(head) = words.first() else {
        return false;
    };
    if !head.eq_ignore_ascii_case("superseded") {
        return true;
    }
    let mut index = 1;
    if words
        .get(index)
        .is_some_and(|word| word.eq_ignore_ascii_case("by"))
    {
        index += 1;
    }
    let mut references = 0;
    while index < words.len() {
        if words[index] != "ADR" && !words[index].starts_with("ADR-") {
            if words
                .get(index + 1)
                .is_some_and(|word| *word == "ADR" || word.starts_with("ADR-"))
            {
                index += 1;
            } else if references > 0
                && words[index].len() == 4
                && words[index].bytes().all(|byte| byte.is_ascii_digit())
            {
                index += 1;
                continue;
            } else {
                return false;
            }
        }
        if words[index] == "ADR" {
            index += 1;
            if !words.get(index).is_some_and(|word| {
                word.len() == 4 && word.bytes().all(|byte| byte.is_ascii_digit())
            }) {
                return false;
            }
        } else if words[index].len() != 8
            || !words[index][4..].bytes().all(|byte| byte.is_ascii_digit())
        {
            return false;
        }
        index += 1;
        references += 1;
    }
    references > 0
}

fn replacement(path: &str, line: &str, adrs: &[String]) -> Result<Option<String>, String> {
    if line.starts_with("Status: ") && status::parse_canonical_line(line).is_ok() {
        return Ok(None);
    }
    let raw = line
        .strip_prefix("ステータス:")
        .or_else(|| line.strip_prefix("Status:"))
        .ok_or_else(|| "ステータス行を解釈できません".to_owned())?
        .trim();
    if let Some(value) = simple(raw) {
        return Ok(Some(format!("Status: {value}")));
    }
    if matches!(status::parse(raw), status::Status::Superseded(_)) {
        if !english_superseded_has_only_references(raw) {
            return Err("注記を保全して変換できません".to_owned());
        }
        let changes = revisions::resolve_legacy(path, raw, adrs)?;
        let values: Vec<_> = changes
            .into_iter()
            .map(|change| serde_json::json!({"part": change.part, "by": change.by}))
            .collect();
        return Ok(Some(format!(
            "Status: superseded {}",
            serde_json::to_string(&values).map_err(|error| error.to_string())?
        )));
    }
    Err(format!(
        "注記または未知のステータスを保全して変換できません: {raw}"
    ))
}

fn plan(root: &Path, adrs: &[String]) -> Result<Vec<Rewrite>, String> {
    let mut rewrites = Vec::new();
    for path in adrs {
        let original = fs::read(root.join(path)).map_err(|error| format!("{path}: {error}"))?;
        let content = std::str::from_utf8(&original)
            .map_err(|error| format!("{path}: UTF-8で読めません: {error}"))?;
        let lines = status::status_lines(content);
        if lines.len() != 1 {
            return Err(format!(
                "{path}: ステータス行は1件必要です（{}件）",
                lines.len()
            ));
        }
        let line_number = lines[0].0;
        let mut replacement_line = None;
        let mut new_content = Vec::with_capacity(original.len());
        for (index, line) in content.split_inclusive('\n').enumerate() {
            if index + 1 == line_number {
                let ending = if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                let bare = line.trim_end_matches('\n').trim_end_matches('\r');
                let changed = replacement(path, bare, adrs)
                    .map_err(|error| format!("{path}:{}: {error}", index + 1))?;
                if let Some(new_line) = changed {
                    new_content.extend_from_slice(new_line.as_bytes());
                    new_content.extend_from_slice(ending.as_bytes());
                    replacement_line = Some((bare.to_owned(), new_line));
                } else {
                    new_content.extend_from_slice(line.as_bytes());
                }
            } else {
                new_content.extend_from_slice(line.as_bytes());
            }
        }
        if let Some((before, after)) = replacement_line {
            rewrites.push(Rewrite {
                path: path.clone(),
                line: line_number,
                before,
                after,
                original,
                content: new_content,
            });
        }
    }
    Ok(rewrites)
}

fn preview(rewrites: &[Rewrite]) {
    for rewrite in rewrites {
        println!("--- a/{}", rewrite.path);
        println!("+++ b/{}", rewrite.path);
        println!("@@ -{},1 +{},1 @@", rewrite.line, rewrite.line);
        println!("-{}", rewrite.before);
        println!("+{}", rewrite.after);
    }
}

fn write_rewrite(root: &Path, rewrite: &Rewrite) -> Result<(), String> {
    let target = root.join(&rewrite.path);
    let current = fs::read(&target).map_err(|error| format!("{}: {error}", rewrite.path))?;
    if current != rewrite.original {
        return Err(format!("{}: 検査後に内容が変わりました", rewrite.path));
    }
    let metadata = fs::metadata(&target).map_err(|error| format!("{}: {error}", rewrite.path))?;
    let temporary = target.with_extension(format!("migrate-{}", std::process::id()));
    fs::write(&temporary, &rewrite.content)
        .map_err(|error| format!("{}: {error}", rewrite.path))?;
    let result = fs::set_permissions(&temporary, metadata.permissions())
        .and_then(|_| fs::rename(&temporary, &target));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("{}: {error}", rewrite.path))
}

fn run() -> Result<(), String> {
    let write = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [arg] if arg == "--write" => true,
        _ => return Err("使い方: migrate-adr-status [--write]".to_owned()),
    };
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let top = git(&cwd, &["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(String::from_utf8_lossy(&top).trim());
    if root != cwd {
        return Err("リポジトリのルートで実行してください".to_owned());
    }
    let adrs = files(&root)?;
    let rewrites = plan(&root, &adrs)?;
    if write {
        for rewrite in &rewrites {
            write_rewrite(&root, rewrite)?;
        }
    } else {
        preview(&rewrites);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
