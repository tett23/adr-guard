//! インデックスまたはコミットの内容からADRの状態遷移を検査する。
use crate::status::{self, AdrState, Classification, Status};
use crate::{Allow, diff_entries, git, is_adr_path, renumber_pairs, revisions};
use std::path::Path;

fn document(tree: &str, path: &str, cwd: &Path) -> Result<String, String> {
    git(&["show", &format!("{tree}:{path}")], cwd)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .ok_or_else(|| format!("{path}: Gitから文書を取得できません"))
}

fn old_paths(tree: &str, cwd: &Path) -> Result<Vec<String>, String> {
    let listing = git(&["ls-tree", "-rz", "--name-only", tree], cwd)
        .ok_or_else(|| format!("{tree}: ADRの一覧を取得できません"))?;
    Ok(listing
        .split(|b| *b == 0)
        .filter_map(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|path| path.ends_with(".md") && is_adr_path(path))
        .map(str::to_owned)
        .collect())
}

fn check_document(
    path: &str,
    content: &str,
    old: Option<(&str, String)>,
    tree: &str,
    cwd: &Path,
) -> Vec<String> {
    let findings = status::count_violations(path, content);
    if !findings.is_empty() {
        return findings;
    }
    let after = status::status(content).expect("validated status line");
    let line = content.lines().nth(after.line - 1).unwrap();
    let mut before = AdrState::Absent;
    let mut previous = "Absent";
    if let Some((old_path, ref old_content)) = old {
        if status::status_lines(old_content).is_empty() {
            // 既存文書の欠落した行を修復する場合も、新しく書く値は検査する。
            return status::parse_canonical_line(line)
                .err()
                .into_iter()
                .map(|error| format!("{path}:{}: {error}", after.line))
                .collect();
        }
        let Some(old_status) = status::status(old_content) else {
            return vec![format!(
                "{path}:{}: 変更前のステータス行を一意に特定できません",
                after.line
            )];
        };
        let old_line = old_content.lines().nth(old_status.line - 1).unwrap();
        if old_line == line {
            return vec![];
        }
        previous = old_status.raw;
        let mut state = old_status.status;
        if matches!(state, Status::Superseded(_))
            && matches!(after.status, Status::Superseded(_))
            && status::parse_canonical_line(old_line).is_err()
        {
            match old_paths(tree, cwd)
                .and_then(|paths| revisions::resolve_legacy(old_path, previous, &paths))
            {
                Ok(changes) => state = Status::Superseded(changes),
                Err(message) => return vec![format!("{path}:{}: {message}", after.line)],
            }
        }
        before = AdrState::Present(state);
    }
    let result = status::parse_canonical_line(line).and_then(|next| {
        match status::classify(before, AdrState::Present(next)) {
            Classification::Unclassified => Err(format!(
                "許可されていない状態遷移: {previous} -> {}",
                after.raw
            )),
            _ => Ok(()),
        }
    });
    result
        .err()
        .into_iter()
        .map(|error| format!("{path}:{}: {error}", after.line))
        .collect()
}

pub(super) fn check(
    diff: &[u8],
    old_tree: &str,
    new_tree: &str,
    allow: Allow,
    cwd: &Path,
) -> Vec<String> {
    let entries = diff_entries(diff);
    let renames = if allow.renumber {
        renumber_pairs(&entries, old_tree, new_tree, allow, cwd)
    } else {
        vec![]
    };
    let mut messages = Vec::new();
    for (kind, path) in entries {
        if !matches!(kind, "A" | "M" | "T") || !path.ends_with(".md") || !is_adr_path(path) {
            continue;
        }
        let source = if kind == "A" {
            renames
                .iter()
                .find(|(_, to, ..)| to == path)
                .map(|(from, ..)| from.as_str())
        } else {
            Some(path)
        };
        let result = document(new_tree, path, cwd).and_then(|content| {
            let old = source
                .map(|from| document(old_tree, from, cwd).map(|text| (from, text)))
                .transpose()?;
            Ok(check_document(path, &content, old, old_tree, cwd))
        });
        match result {
            Ok(findings) => messages.extend(findings),
            Err(error) => messages.push(error),
        }
    }
    messages
}
