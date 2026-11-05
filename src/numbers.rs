//! ADRの番号の重複の検出と、番号の振りなおし（ADR 0018）。

use std::collections::{BTreeMap, HashMap};

use crate::links;

/// ADRのファイル名を、4桁の番号と `-` の後ろの名前に分ける（`0017-b.md` は `0017` と `b.md`）。
pub fn split_file_name(file: &str) -> Option<(&str, &str)> {
    let number = file.get(..4)?;
    let name = file[4..].strip_prefix('-')?;
    (number.bytes().all(|b| b.is_ascii_digit()) && name.ends_with(".md") && name.len() > 3)
        .then_some((number, name))
}

/// ADRのディレクトリの直下にある、番号の付いたADRなら、ディレクトリ・ファイル名・番号・名前。
fn entry(path: &str) -> Option<(&str, &str, &str, &str)> {
    let (dir, file) = path.rsplit_once('/')?;
    let last = dir.rsplit('/').next()?;
    let (number, name) = split_file_name(file)?;
    links::ADR_DIR_NAMES
        .contains(&last)
        .then_some((dir, file, number, name))
}

/// 同じディレクトリの、同じ番号のADR。
#[derive(Debug, PartialEq)]
pub struct Duplicate {
    pub dir: String,
    pub number: String,
    /// ファイル名の昇順
    pub files: Vec<String>,
}

/// ADRのパスの一覧から、番号の重複を探す（ディレクトリ、番号の順）。
pub fn duplicates(adr_paths: &[String]) -> Vec<Duplicate> {
    let groups = adr_paths.iter().filter_map(|path| entry(path)).fold(
        BTreeMap::new(),
        |mut groups, (dir, file, number, _)| {
            groups
                .entry((dir.to_owned(), number.to_owned()))
                .or_insert_with(Vec::new)
                .push(file.to_owned());
            groups
        },
    );
    groups
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|((dir, number), mut files)| {
            files.sort();
            Duplicate { dir, number, files }
        })
        .collect()
}

/// 番号の振りなおし。
#[derive(Debug, PartialEq, Clone)]
pub struct Rename {
    pub dir: String,
    pub old_file: String,
    pub new_file: String,
    pub old_number: String,
    pub new_number: String,
}

impl Rename {
    pub fn old_path(&self) -> String {
        format!("{}/{}", self.dir, self.old_file)
    }

    pub fn new_path(&self) -> String {
        format!("{}/{}", self.dir, self.new_file)
    }
}

/// 番号の重複を直す振りなおしの計画。重複した番号のADRのうち、`added`（追加された時刻。
/// 未コミットならNone）が最も早いもの（同じならファイル名の昇順で先のもの）を残し、
/// 残りに、ディレクトリの最大の番号の次から順に番号を割り当てる。
pub fn plan(adr_paths: &[String], added: impl Fn(&str) -> Option<i64>) -> Vec<Rename> {
    let mut max: HashMap<String, u32> = adr_paths.iter().filter_map(|path| entry(path)).fold(
        HashMap::new(),
        |mut max, (dir, _, number, _)| {
            let number: u32 = number.parse().unwrap_or(0);
            let current = max.entry(dir.to_owned()).or_insert(0);
            *current = (*current).max(number);
            max
        },
    );
    duplicates(adr_paths)
        .into_iter()
        .flat_map(|duplicate| {
            let mut files = duplicate.files.clone();
            files.sort_by_key(|file| {
                (
                    added(&format!("{}/{file}", duplicate.dir)).unwrap_or(i64::MAX),
                    file.clone(),
                )
            });
            files
                .into_iter()
                .skip(1)
                .map(|file| (duplicate.dir.clone(), duplicate.number.clone(), file))
                .collect::<Vec<_>>()
        })
        .map(|(dir, old_number, old_file)| {
            let next = max.get(&dir).copied().unwrap_or(0) + 1;
            max.insert(dir.clone(), next);
            let new_number = format!("{next:04}");
            let name = &old_file[5..];
            Rename {
                new_file: format!("{new_number}-{name}"),
                dir,
                old_file,
                old_number,
                new_number,
            }
        })
        .collect()
}

/// 1行目が `# ADR <old>:` で始まれば、その番号を `new` にする。
pub fn rewrite_title(content: &str, old: &str, new: &str) -> String {
    let prefix = format!("# ADR {old}:");
    match content.strip_prefix(&prefix) {
        Some(rest) => format!("# ADR {new}:{rest}"),
        None => content.to_owned(),
    }
}

/// `..` と `.` を取り除いたパス。
fn normalize(path: &str) -> String {
    path.split('/')
        .fold(Vec::new(), |mut parts, part| {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                part => parts.push(part),
            }
            parts
        })
        .join("/")
}

/// リンクのテキストの番号を、振りなおしに合わせて書きかえる（`ADR <old>` と `<old>` だけ）。
fn renumber_text(text: &str, old: &str, new: &str) -> String {
    if text == format!("ADR {old}") {
        format!("ADR {new}")
    } else if text == old {
        new.to_owned()
    } else {
        text.to_owned()
    }
}

/// 文書のリンクのうち、振りなおしたADRを指すもののリンク先とテキストを書きかえる。置きかえた数も返す。
pub fn rewrite_links(doc: &str, content: &str, renames: &[Rename]) -> (String, usize) {
    let dir = doc.rsplit_once('/').map_or("", |(dir, _)| dir);
    links::map_inline_links(content, |text, dest| {
        if dest.contains("://") || dest.starts_with('/') || dest.starts_with('#') {
            return None;
        }
        let (path, fragment) = dest
            .split_once('#')
            .map_or((dest, None), |(p, f)| (p, Some(f)));
        let target = normalize(&format!("{dir}/{path}"));
        let rename = renames.iter().find(|rename| rename.old_path() == target)?;
        let base = path.rsplit_once('/').map_or("", |(base, _)| base);
        let new_path = if base.is_empty() {
            rename.new_file.clone()
        } else {
            format!("{base}/{}", rename.new_file)
        };
        let new_dest = fragment.map_or(new_path.clone(), |f| format!("{new_path}#{f}"));
        Some((
            renumber_text(text, &rename.old_number, &rename.new_number),
            new_dest,
        ))
    })
}

/// リンクのテキストのADRの番号を、振りなおした新しい番号から古い番号に読みかえる（`--allow-renumber` の比較）。
/// `mapping` は（新しい番号、古い番号）の組。
pub fn restore_link_texts(content: &str, mapping: &[(String, String)]) -> String {
    links::map_inline_links(content, |text, dest| {
        mapping.iter().find_map(|(new, old)| {
            let restored = renumber_text(text, new, old);
            (restored != text).then(|| (restored, dest.to_owned()))
        })
    })
    .0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(items: &[&str]) -> Vec<String> {
        items.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn splits_file_names() {
        assert_eq!(split_file_name("0017-b.md"), Some(("0017", "b.md")));
        assert_eq!(split_file_name("0017.md"), None);
        assert_eq!(split_file_name("017-b.md"), None);
        assert_eq!(split_file_name("0017-b.txt"), None);
    }

    #[test]
    fn finds_duplicates_in_the_same_directory() {
        let found = duplicates(&paths(&[
            "docs/adr/0017-b.md",
            "docs/adr/0017-a.md",
            "docs/adr/0016-x.md",
            "docs/server/adr/0017-a.md",
            "docs/adr/0017.md",
            "docs/adr/images/0017-c.md",
        ]));
        assert_eq!(
            found,
            [Duplicate {
                dir: "docs/adr".to_owned(),
                number: "0017".to_owned(),
                files: vec!["0017-a.md".to_owned(), "0017-b.md".to_owned()],
            }]
        );
    }

    #[test]
    fn plans_new_numbers_after_the_maximum() {
        let all = paths(&[
            "docs/adr/0017-a.md",
            "docs/adr/0017-b.md",
            "docs/adr/0017-c.md",
            "docs/adr/0020-z.md",
        ]);
        let added = |path: &str| match path {
            "docs/adr/0017-a.md" => Some(3),
            "docs/adr/0017-b.md" => Some(1),
            _ => None,
        };
        let planned: Vec<(String, String)> = plan(&all, added)
            .into_iter()
            .map(|r| (r.old_file, r.new_file))
            .collect();
        assert_eq!(
            planned,
            [
                ("0017-a.md".to_owned(), "0021-a.md".to_owned()),
                ("0017-c.md".to_owned(), "0022-c.md".to_owned()),
            ]
        );
    }

    #[test]
    fn keeps_the_first_file_name_when_not_committed() {
        let all = paths(&["docs/adr/0017-b.md", "docs/adr/0017-a.md"]);
        let planned = plan(&all, |_| None);
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].old_file, "0017-b.md");
        assert_eq!(planned[0].new_file, "0018-b.md");
    }

    #[test]
    fn rewrites_titles() {
        assert_eq!(
            rewrite_title("# ADR 0017: b\n本文\n", "0017", "0018"),
            "# ADR 0018: b\n本文\n"
        );
        assert_eq!(
            rewrite_title("# ADR 0016: x\n", "0017", "0018"),
            "# ADR 0016: x\n"
        );
    }

    fn rename() -> Vec<Rename> {
        vec![Rename {
            dir: "docs/adr".to_owned(),
            old_file: "0017-b.md".to_owned(),
            new_file: "0018-b.md".to_owned(),
            old_number: "0017".to_owned(),
            new_number: "0018".to_owned(),
        }]
    }

    #[test]
    fn rewrites_links_to_renamed_adrs() {
        let cases = [
            (
                "docs/adr/0016-x.md",
                "[ADR 0017](0017-b.md)",
                "[ADR 0018](0018-b.md)",
            ),
            (
                "docs/adr/0016-x.md",
                "[参照](0017-b.md)",
                "[参照](0018-b.md)",
            ),
            (
                "docs/adr/0016-x.md",
                "[0017](0017-b.md#節)",
                "[0018](0018-b.md#節)",
            ),
            (
                "docs/adr/0016-x.md",
                "[ADR 0017](0017-a.md)",
                "[ADR 0017](0017-a.md)",
            ),
            (
                "docs/server/a.md",
                "[ADR 0017](../adr/0017-b.md)",
                "[ADR 0018](../adr/0018-b.md)",
            ),
            (
                "README.md",
                "[ADR 0017](docs/adr/0017-b.md)",
                "[ADR 0018](docs/adr/0018-b.md)",
            ),
            (
                "README.md",
                "[ADR 0017](https://x/docs/adr/0017-b.md)",
                "[ADR 0017](https://x/docs/adr/0017-b.md)",
            ),
            (
                "README.md",
                "`[ADR 0017](docs/adr/0017-b.md)`",
                "`[ADR 0017](docs/adr/0017-b.md)`",
            ),
        ];
        for (doc, content, expected) in cases {
            assert_eq!(
                rewrite_links(doc, content, &rename()).0,
                expected,
                "{doc} {content}"
            );
        }
    }

    #[test]
    fn restores_link_texts() {
        let mapping = vec![("0018".to_owned(), "0017".to_owned())];
        assert_eq!(
            restore_link_texts("[ADR 0018](0018-b.md) と [ADR 0016](x.md)", &mapping),
            "[ADR 0017](0018-b.md) と [ADR 0016](x.md)"
        );
    }
}
