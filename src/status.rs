//! ADRのステータスの行の判定と、ステータスの値の解釈（ADR 0012）。
//!
//! ステータスの行は、行頭が `ステータス:` か `Status:` の行。ADRはステータスの行をちょうど1つ持つ。

use crate::links;

/// ステータスの行の見出し（コロンを含む）。
const LABELS: [&str; 2] = ["ステータス:", "Status:"];

/// ステータスの行。
#[derive(Debug, PartialEq)]
pub struct StatusLine<'a> {
    /// 1から始まる行番号（現時点では出力に使わない）
    #[cfg_attr(not(test), allow(dead_code))]
    pub line: usize,
    /// コロンより後ろの、前後の空白を除いた元の値。出力にはこれを使う
    pub raw: &'a str,
    /// 元の値を解釈した状態
    pub status: Status,
}

/// ADRの状態。
#[derive(Debug, PartialEq)]
pub enum Status {
    /// `提案…`、`Proposed`。レビュー待ち
    Proposed,
    /// `採択…`（`採択→` で始まるものを除く）、`Accepted`
    Accepted,
    /// `採択→…`、`Superseded by …`。一部または全部が後のADRで変更された
    Superseded(Vec<Change>),
    /// `却下…`、`Rejected`
    Rejected,
    /// `破棄…`、`Discarded`。採択されずに取りやめた
    Discarded,
    /// `廃止…`、`Deprecated`。かつて採択され、いまは効力が無い
    Deprecated,
    /// 上のどれにも当たらない値（空の値を含む）
    Unknown,
}

/// 後のADRによる変更の1つ（現時点では出力に使わない）。
#[derive(Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Change {
    /// 変わった部分。全体が変わった場合や書かれていない場合はNone
    pub part: Option<String>,
    /// 変更したADRの番号（4桁の数字）
    pub by: Vec<String>,
}

/// 行（改行を含んでもよい）がステータスの行なら、その元の値。
fn value_of(line: &str) -> Option<&str> {
    let line = line.trim_end_matches('\n').trim_end_matches('\r');
    LABELS
        .iter()
        .find_map(|label| line.strip_prefix(label))
        .map(str::trim)
}

/// 行がステータスの行か（バイト列。`--allow-status` の正規化で使う）。
pub fn is_status_line(line: &[u8]) -> bool {
    LABELS
        .iter()
        .any(|label| line.starts_with(label.as_bytes()))
}

/// すべてのステータスの行（行番号と元の値）。
pub fn status_lines(content: &str) -> Vec<(usize, &str)> {
    content
        .split_inclusive('\n')
        .enumerate()
        .filter_map(|(index, line)| value_of(line).map(|raw| (index + 1, raw)))
        .collect()
}

/// ステータスの行がちょうど1つあれば、その行。
pub fn status(content: &str) -> Option<StatusLine<'_>> {
    match status_lines(content).as_slice() {
        [(line, raw)] => Some(StatusLine {
            line: *line,
            raw,
            status: parse(raw),
        }),
        _ => None,
    }
}

/// ステータスの行の数の違反（ちょうど1つでなければ、メッセージの一覧）。
pub fn count_violations(path: &str, content: &str) -> Vec<String> {
    match status_lines(content).as_slice() {
        [] => vec![format!(
            "{path}: ステータスの行がありません（ステータス: または Status: で始まる行が要ります）"
        )],
        [_] => vec![],
        [(first, _), rest @ ..] => rest
            .iter()
            .map(|(line, _)| {
                format!("{path}:{line}: ステータスの行が2つ以上あります（最初は{first}行目）")
            })
            .collect(),
    }
}

/// 英語のキーワードで始まるか（大文字と小文字を区別せず、直後が英数字でないもの）。続く文字列を返す。
fn english<'a>(raw: &'a str, keyword: &str) -> Option<&'a str> {
    let head = raw.get(..keyword.len())?;
    let rest = &raw[keyword.len()..];
    (head.eq_ignore_ascii_case(keyword)
        && !rest
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric()))
    .then_some(rest)
}

/// `採択` に0個以上の空白と `→` が続けば、`→` より後ろ。
fn superseded_ja(raw: &str) -> Option<&str> {
    raw.strip_prefix("採択")?.trim_start().strip_prefix('→')
}

/// 元の値を状態に変換する。
pub fn parse(raw: &str) -> Status {
    if raw.starts_with("提案") {
        Status::Proposed
    } else if let Some(rest) = superseded_ja(raw) {
        Status::Superseded(changes_ja(rest))
    } else if raw.starts_with("採択") {
        Status::Accepted
    } else if raw.starts_with("却下") {
        Status::Rejected
    } else if raw.starts_with("破棄") {
        Status::Discarded
    } else if raw.starts_with("廃止") {
        Status::Deprecated
    } else if english(raw, "proposed").is_some() {
        Status::Proposed
    } else if english(raw, "accepted").is_some() {
        Status::Accepted
    } else if let Some(rest) = english(raw, "superseded") {
        Status::Superseded(changes_en(rest))
    } else if english(raw, "rejected").is_some() {
        Status::Rejected
    } else if english(raw, "discarded").is_some() {
        Status::Discarded
    } else if english(raw, "deprecated").is_some() {
        Status::Deprecated
    } else {
        Status::Unknown
    }
}

/// ADRの番号の参照の並び（`ADR 0005、0006`、`ADR-0005`）。
#[derive(Debug, PartialEq)]
struct Sequence {
    start: usize,
    end: usize,
    numbers: Vec<String>,
}

fn is_word(b: Option<&u8>) -> bool {
    b.is_some_and(u8::is_ascii_alphanumeric)
}

/// `at` から始まる4桁の数字（直後が英数字でないもの）。
fn number_at(text: &str, at: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let digits = bytes.get(at..at + 4)?;
    (digits.iter().all(u8::is_ascii_digit) && !is_word(bytes.get(at + 4)))
        .then(|| &text[at..at + 4])
}

/// `at` にある `、` か `,` と、続く空白の後ろの位置。
fn after_separator(text: &str, at: usize) -> Option<usize> {
    let rest = &text[at..];
    let skipped = rest.strip_prefix('、').or_else(|| rest.strip_prefix(','))?;
    Some(text.len() - skipped.trim_start_matches(' ').len())
}

/// 文字列の中の参照の並び。
fn sequences(text: &str) -> Vec<Sequence> {
    let bytes = text.as_bytes();
    text.match_indices("ADR")
        .filter(|&(at, _)| at == 0 || !is_word(bytes.get(at - 1)))
        .filter(|&(at, _)| matches!(bytes.get(at + 3), Some(b' ' | b'-')))
        .filter_map(|(at, _)| {
            let first = number_at(text, at + 4)?;
            let (end, rest) = std::iter::successors(Some((at + 8, first)), |&(end, _)| {
                let next = after_separator(text, end)?;
                number_at(text, next).map(|number| (next + 4, number))
            })
            .fold((at + 8, Vec::new()), |(_, mut numbers), (end, number)| {
                numbers.push(number.to_owned());
                (end, numbers)
            });
            Some(Sequence {
                start: at,
                end,
                numbers: rest,
            })
        })
        .collect()
}

/// `0004`、`0004、0005` のように、4桁の数字を区切って並べただけの文字列なら、その番号。
fn bare_numbers(text: &str) -> Option<Vec<String>> {
    let numbers: Vec<String> = text
        .split(['、', ','])
        .map(str::trim)
        .map(|part| {
            (part.len() == 4 && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.to_owned())
        })
        .collect::<Option<_>>()?;
    (!numbers.is_empty()).then_some(numbers)
}

/// 変わった部分の文字列を整える。空ならNone。
fn part(text: &str) -> Option<String> {
    let trimmed = text
        .trim()
        .trim_start_matches(['、', ','])
        .trim()
        .trim_end_matches('は')
        .trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// `採択→` より後ろから変更を取り出す。
fn changes_ja(rest: &str) -> Vec<Change> {
    let text = links::link_texts(rest);
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }
    if let Some(by) = bare_numbers(text) {
        return vec![Change { part: None, by }];
    }
    let found = sequences(text);
    let last_end = found.last().map_or(0, |sequence| sequence.end);
    let starts = std::iter::once(0).chain(found.iter().map(|sequence| sequence.end));
    let changes = found.iter().zip(starts).map(|(sequence, from)| Change {
        part: part(&text[from..sequence.start]),
        by: sequence.numbers.clone(),
    });
    let tail = text[last_end..].trim_matches(|c: char| c.is_whitespace() || c == '、');
    let trailing = (!tail.is_empty()).then(|| Change {
        part: Some(tail.to_owned()),
        by: vec![],
    });
    changes.chain(trailing).collect()
}

/// `Superseded` より後ろから変更を取り出す。
fn changes_en(rest: &str) -> Vec<Change> {
    let text = links::link_texts(rest);
    let text = text.trim_start();
    let text = english(text, "by").unwrap_or(text);
    vec![Change {
        part: None,
        by: sequences(text)
            .into_iter()
            .flat_map(|sequence| sequence.numbers)
            .collect(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(content: &str) -> Option<&str> {
        status(content).map(|line| line.raw)
    }

    #[test]
    fn recognizes_status_lines() {
        let cases = [
            ("ステータス: 提案", "提案"),
            ("ステータス:提案", "提案"),
            ("Status: Accepted", "Accepted"),
            ("Status:Accepted", "Accepted"),
            ("Status: Accepted\r\n", "Accepted"),
            ("Status:", ""),
            ("# ADR 0001: x\n\nStatus: Proposed\n\nbody\n", "Proposed"),
        ];
        for (content, expected) in cases {
            assert_eq!(raw(content), Some(expected), "{content:?}");
        }
    }

    #[test]
    fn rejects_other_lines() {
        for content in [
            "- Status: Accepted",
            "* Status: proposed",
            "- ステータス: 採択",
            "status: accepted",
            "STATUS: Accepted",
            "Status：Accepted",
            "Status : Accepted",
            "  Status: Accepted",
            "**Status:** Accepted",
            "## Status\n\nAccepted\n",
            "前置き Status: Accepted",
            "Statuses: x",
            "ステータス：提案",
        ] {
            assert_eq!(raw(content), None, "{content:?}");
        }
    }

    #[test]
    fn requires_exactly_one_status_line() {
        let content = "Status: Accepted\nステータス: 提案\n";
        assert_eq!(raw(content), None);
        assert_eq!(status_lines(content), [(1, "Accepted"), (2, "提案")]);
        assert_eq!(
            count_violations("a.md", content),
            ["a.md:2: ステータスの行が2つ以上あります（最初は1行目）"]
        );
        assert_eq!(
            count_violations("a.md", "- Status: Accepted\n"),
            ["a.md: ステータスの行がありません（ステータス: または Status: で始まる行が要ります）"]
        );
        assert!(count_violations("a.md", "ステータス: 採択\n").is_empty());
        assert_eq!(status("x\n\nStatus: Accepted\n").map(|s| s.line), Some(3));
    }

    #[test]
    fn parses_simple_statuses() {
        let cases = [
            ("提案", Status::Proposed),
            ("提案（レビュー中）", Status::Proposed),
            ("Proposed", Status::Proposed),
            ("proposed", Status::Proposed),
            ("PROPOSED", Status::Proposed),
            ("採択", Status::Accepted),
            ("採択（2026-10-01）", Status::Accepted),
            ("Accepted", Status::Accepted),
            ("accepted", Status::Accepted),
            ("却下", Status::Rejected),
            ("Rejected", Status::Rejected),
            ("破棄", Status::Discarded),
            ("Discarded", Status::Discarded),
            ("廃止", Status::Deprecated),
            ("廃止（v2 で削除）", Status::Deprecated),
            ("Deprecated", Status::Deprecated),
            ("Draft", Status::Unknown),
            ("採用", Status::Unknown),
            ("Acceptedly", Status::Unknown),
            ("Proposal", Status::Unknown),
            ("", Status::Unknown),
        ];
        for (raw, expected) in cases {
            assert_eq!(parse(raw), expected, "{raw:?}");
        }
    }

    fn change(part: Option<&str>, by: &[&str]) -> Change {
        Change {
            part: part.map(str::to_owned),
            by: by.iter().map(|n| (*n).to_owned()).collect(),
        }
    }

    #[test]
    fn parses_changes() {
        let cases = [
            ("採択→0004", vec![change(None, &["0004"])]),
            ("採択 → 0004", vec![change(None, &["0004"])]),
            ("採択→", vec![]),
            (
                "採択→ADRのディレクトリは[ADR 0002](0002-module-adr-directories.md)",
                vec![change(Some("ADRのディレクトリ"), &["0002"])],
            ),
            (
                "採択→proposed の出力は[ADR 0006](0006-x.md)、list の出力は[ADR 0007](0007-y.md)",
                vec![
                    change(Some("proposed の出力"), &["0006"]),
                    change(Some("list の出力"), &["0007"]),
                ],
            ),
            (
                "採択→A、BはADR 0005、0006",
                vec![change(Some("A、B"), &["0005", "0006"])],
            ),
            (
                "採択→一部は別の方法に変更",
                vec![change(Some("一部は別の方法に変更"), &[])],
            ),
            (
                "Superseded by [ADR-0005](0005-example.md)",
                vec![change(None, &["0005"])],
            ),
            (
                "superseded by ADR 0003, 0004",
                vec![change(None, &["0003", "0004"])],
            ),
            ("Superseded", vec![change(None, &[])]),
            (
                "Superseded by ADR 0003 on 2026-10-01",
                vec![change(None, &["0003"])],
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(parse(raw), Status::Superseded(expected), "{raw:?}");
        }
    }

    #[test]
    fn detects_status_line_bytes() {
        assert!(is_status_line("ステータス: 提案\n".as_bytes()));
        assert!(is_status_line(b"Status: Accepted\n"));
        assert!(!is_status_line(b"- Status: Accepted\n"));
    }
}
