//! Codex の `apply_patch` のパッチを読む（ADR 0021）。
//!
//! 文法は openai/codex の `codex-rs/apply-patch/src/parser.rs` による。
//! 判定に要る部分（ハンクのパスと、消す行・足す行）だけを読み、パッチの適用はしない。

const BEGIN: &str = "*** Begin Patch";
const END: &str = "*** End Patch";
const ADD: &str = "*** Add File: ";
const DELETE: &str = "*** Delete File: ";
const UPDATE: &str = "*** Update File: ";
const MOVE: &str = "*** Move to: ";

/// ファイルごとの塊。
#[derive(Debug, PartialEq, Eq)]
pub enum Hunk {
    /// ファイルを作る（あれば上書きする）。`contents` は新しい内容。
    Add { path: String, contents: String },
    /// ファイルを消す。
    Delete { path: String },
    /// ファイルを変える。`removed` と `added` は、先頭の1文字を除いた消す行と足す行。
    Update {
        path: String,
        move_to: Option<String>,
        removed: Vec<String>,
        added: Vec<String>,
    },
}

/// 文字列の中のパッチを読む。前後の空白を除くと `*** Begin Patch` になる最初の行から、
/// `*** End Patch` になる次の行までをパッチとする。パッチが無ければNone。
pub fn parse(text: &str) -> Option<Vec<Hunk>> {
    let body: Vec<&str> = text
        .lines()
        .skip_while(|line| line.trim() != BEGIN)
        .skip(1)
        .collect();
    let end = body.iter().position(|line| line.trim() == END)?;
    Some(hunks(&body[..end]))
}

fn header<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.trim().strip_prefix(marker.trim_end()).map(str::trim)
}

fn is_header(line: &str) -> bool {
    [ADD, DELETE, UPDATE]
        .iter()
        .any(|marker| header(line, marker).is_some())
}

fn hunks(body: &[&str]) -> Vec<Hunk> {
    match body.iter().position(|line| is_header(line)) {
        None => Vec::new(),
        Some(start) => {
            let rest = &body[start + 1..];
            let end = rest
                .iter()
                .position(|line| is_header(line))
                .unwrap_or(rest.len());
            std::iter::once(hunk(body[start], &rest[..end]))
                .chain(hunks(&rest[end..]))
                .collect()
        }
    }
}

fn hunk(head: &str, lines: &[&str]) -> Hunk {
    if let Some(path) = header(head, ADD) {
        Hunk::Add {
            path: path.to_owned(),
            contents: lines
                .iter()
                .filter_map(|line| line.strip_prefix('+'))
                .map(|line| format!("{line}\n"))
                .collect(),
        }
    } else if let Some(path) = header(head, DELETE) {
        Hunk::Delete {
            path: path.to_owned(),
        }
    } else {
        let path = header(head, UPDATE).unwrap_or_default().to_owned();
        let (move_to, changes) = match lines.split_first() {
            Some((first, rest)) if header(first, MOVE).is_some() => {
                (header(first, MOVE).map(str::to_owned), rest)
            }
            _ => (None, lines),
        };
        let collect = |sign: char| {
            changes
                .iter()
                .filter_map(|line| line.strip_prefix(sign))
                .map(str::to_owned)
                .collect()
        };
        Hunk::Update {
            path,
            move_to,
            removed: collect('-'),
            added: collect('+'),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(path: &str, move_to: Option<&str>, removed: &[&str], added: &[&str]) -> Hunk {
        Hunk::Update {
            path: path.to_owned(),
            move_to: move_to.map(str::to_owned),
            removed: removed.iter().map(|s| s.to_string()).collect(),
            added: added.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn reads_each_kind_of_hunk() {
        let patch = "*** Begin Patch
*** Add File: a.md
+one
+two
*** Delete File: b.md
*** Update File: c.md
@@
 keep
-old
+new
*** Update File: d.md
*** Move to: e.md
@@ ctx
-x
*** End Patch
";
        assert_eq!(
            parse(patch),
            Some(vec![
                Hunk::Add {
                    path: "a.md".into(),
                    contents: "one\ntwo\n".into()
                },
                Hunk::Delete {
                    path: "b.md".into()
                },
                update("c.md", None, &["old"], &["new"]),
                update("d.md", Some("e.md"), &["x"], &[]),
            ])
        );
    }

    #[test]
    fn skips_markers_and_context_lines() {
        let patch = "*** Begin Patch
*** Environment ID: local
*** Update File: c.md
@@ fn main
 context
-old
+new

@@
+added
*** End of File
*** End Patch";
        assert_eq!(
            parse(patch),
            Some(vec![update("c.md", None, &["old"], &["new", "added"])])
        );
    }

    #[test]
    fn allows_whitespace_around_markers() {
        let patch = "  *** Begin Patch  \n *** Update File: c.md \n-old\n+new\n *** End Patch\n";
        assert_eq!(
            parse(patch),
            Some(vec![update("c.md", None, &["old"], &["new"])])
        );
    }

    #[test]
    fn reads_patches_in_heredocs() {
        let command =
            "apply_patch <<'EOF'\n*** Begin Patch\n*** Delete File: b.md\n*** End Patch\nEOF\n";
        assert_eq!(
            parse(command),
            Some(vec![Hunk::Delete {
                path: "b.md".into()
            }])
        );
    }

    #[test]
    fn rejects_text_without_a_whole_patch() {
        assert_eq!(parse("ls -la"), None);
        assert_eq!(parse("*** Begin Patch\n*** Delete File: b.md\n"), None);
        assert_eq!(parse("*** Delete File: b.md\n*** End Patch\n"), None);
    }
}
