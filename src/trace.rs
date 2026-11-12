//! ADRのステータスにある修正先を順にたどる（ADR 0023）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::revisions;
use crate::status::{self, Change, Status};

struct Edge {
    part: Option<String>,
    target: String,
}

struct Frame {
    path: String,
    edges: Vec<Edge>,
    next: usize,
    /// 各祖先の後に兄弟の枝があるか。
    later_siblings: Vec<bool>,
}

fn checked_file(root: &Path, path: &str) -> Result<PathBuf, String> {
    let mut file = root.to_path_buf();
    for component in Path::new(path).components() {
        file.push(component);
        let metadata = std::fs::symlink_metadata(&file)
            .map_err(|error| format!("{path}: 読み込みに失敗しました: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{path}: シンボリックリンクをたどれません"));
        }
    }
    Ok(file)
}

fn edges(root: &Path, path: &str, adrs: &[String]) -> Result<Vec<Edge>, String> {
    let file = checked_file(root, path)?;
    let content = std::fs::read_to_string(file)
        .map_err(|error| format!("{path}: 読み込みに失敗しました: {error}"))?;
    let Some(line) = status::status(&content) else {
        return Err(format!("{path}: ステータスの行がちょうど1つ必要です"));
    };
    if line.raw.starts_with("superseded [") {
        status::parse_canonical_line(&format!("Status: {}", line.raw))
            .map_err(|error| format!("{path}: {error}"))?;
    }
    let changes = match line.status {
        Status::Superseded(changes) if line.raw.starts_with("superseded [") => changes,
        Status::Superseded(_) => revisions::resolve_legacy(path, line.raw, adrs)
            .map_err(|error| format!("{path}: {error}"))?,
        _ => Vec::<Change>::new(),
    };
    changes
        .into_iter()
        .flat_map(|change| {
            change.by.into_iter().map(move |target| Edge {
                part: change.part.clone(),
                target,
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|edge| {
            if adrs.contains(&edge.target) {
                Ok(edge)
            } else {
                Err(format!("{path}: 修正先 {} がありません", edge.target))
            }
        })
        .collect()
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn render(root: &Path, start: &str, adrs: &[String], graph: bool) -> Result<String, String> {
    let mut output = if graph {
        format!("{}\n", escape(start))
    } else {
        format!("root\t\t{}\t\n", escape(start))
    };
    let mut seen = HashSet::from([start.to_owned()]);
    let mut ancestors = seen.clone();
    let mut stack = vec![Frame {
        path: start.to_owned(),
        edges: edges(root, start, adrs)?,
        next: 0,
        later_siblings: Vec::new(),
    }];
    while let Some(frame) = stack.last_mut() {
        if frame.next == frame.edges.len() {
            ancestors.remove(&frame.path);
            stack.pop();
            continue;
        }
        let edge = &frame.edges[frame.next];
        let last = frame.next + 1 == frame.edges.len();
        frame.next += 1;
        if ancestors.contains(&edge.target) {
            return Err(format!(
                "{}: {} への参照が循環しています",
                frame.path, edge.target
            ));
        }
        let already_seen = !seen.insert(edge.target.clone());
        if graph {
            for later in &frame.later_siblings {
                output.push_str(if *later { "│   " } else { "    " });
            }
            output.push_str(if last { "└── " } else { "├── " });
            output.push_str(&escape(edge.part.as_deref().unwrap_or("(範囲指定なし)")));
            output.push_str(" -> ");
            output.push_str(&escape(&edge.target));
            if already_seen {
                output.push_str(" [表示済み]");
            }
            output.push('\n');
        } else {
            output.push_str(&format!(
                "edge\t{}\t{}\t{}\n",
                escape(&frame.path),
                escape(&edge.target),
                escape(edge.part.as_deref().unwrap_or("")),
            ));
        }
        if !already_seen {
            let next_path = edge.target.clone();
            let mut later_siblings = frame.later_siblings.clone();
            later_siblings.push(!last);
            stack.push(Frame {
                edges: edges(root, &next_path, adrs)?,
                path: next_path,
                next: 0,
                later_siblings,
            });
        }
    }
    Ok(output)
}

/// 作業ツリーにあるADRの修正関係を表示する。
pub fn run(path: &str, graph: bool) -> ExitCode {
    let Some((root, adrs)) = crate::adr_files() else {
        eprintln!("trace: ADRを列挙できませんでした");
        return ExitCode::FAILURE;
    };
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        eprintln!("trace: {path}: シンボリックリンクをたどれません");
        return ExitCode::FAILURE;
    }
    let absolute = Path::new(path).canonicalize();
    let relative = absolute
        .as_ref()
        .ok()
        .and_then(|path| path.strip_prefix(&root).ok());
    let Some(relative) = relative.and_then(|path| path.to_str()) else {
        eprintln!("trace: {path}: ADRのパスを解決できませんでした");
        return ExitCode::FAILURE;
    };
    if !adrs.iter().any(|adr| adr == relative) {
        eprintln!("trace: {path}: ADRの一覧にありません");
        return ExitCode::FAILURE;
    }
    match render(&root, relative, &adrs, graph) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("trace: {error}");
            ExitCode::FAILURE
        }
    }
}
