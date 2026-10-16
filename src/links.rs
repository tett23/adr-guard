//! 文書の中のADRの参照（`ADR 0001`）の検出と、リンクへの置きかえ（ADR 0009）。
//!
//! Markdownは完全には解析せず、フェンスで囲んだコードブロック、インラインのコード、
//! リンクだけを区別する。

/// 行の中の区間。
#[derive(Debug, PartialEq)]
enum Segment<'a> {
    /// 参照を探す対象の文字列
    Text(&'a str),
    /// インラインのコードなど、そのまま残す文字列
    Opaque(&'a str),
    /// インラインのリンク `[text](dest)`。`image` は直前が `!` の画像か
    InlineLink {
        raw: &'a str,
        text: &'a str,
        image: bool,
    },
    /// 参照形式のリンク `[text][label]`、`[text][]`（ADR 0015）
    RefLink {
        raw: &'a str,
        text: &'a str,
        image: bool,
    },
}

/// リンクの無い参照。
#[derive(Debug, PartialEq)]
pub struct Finding {
    /// 1から始まる行番号
    pub line: usize,
    pub number: String,
}

/// 行の中の参照の位置。`first` は `ADR` を含む最初の番号か、区切りに続く番号か。
#[derive(Debug, PartialEq)]
struct Reference<'a> {
    start: usize,
    end: usize,
    number: &'a str,
    first: bool,
}

/// 行頭（前の空白を除く）が3つ以上の `` ` `` か `~` なら、その文字と数。
fn fence(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start();
    let c = *trimmed.as_bytes().first()?;
    let n = trimmed.bytes().take_while(|&b| b == c).count();
    ((c == b'`' || c == b'~') && n >= 3).then_some((c, n))
}

/// 各行と、その行をそのまま残すか（フェンスの中か、フェンスの行か）。
fn lines_with_fences(content: &str) -> Vec<(&str, bool)> {
    content
        .split_inclusive('\n')
        .scan(None::<(u8, usize)>, |open, line| {
            let verbatim = match (*open, fence(line)) {
                (None, Some(f)) => {
                    *open = Some(f);
                    true
                }
                (Some((c, n)), Some((d, m))) if c == d && m >= n => {
                    *open = None;
                    true
                }
                (Some(_), _) => true,
                (None, None) => false,
            };
            Some((line, verbatim))
        })
        .collect()
}

/// `start` にある `` ` `` の並びと同じ数の並びで閉じるインラインのコードの終わり。
fn code_span_end(bytes: &[u8], start: usize) -> Option<usize> {
    let n = bytes[start..].iter().take_while(|&&b| b == b'`').count();
    let mut i = start + n;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let m = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            if m == n {
                return Some(i + m);
            }
            i += m;
        } else {
            i += 1;
        }
    }
    None
}

/// `start` にある開き括弧に対応する閉じ括弧の位置。
fn matching(bytes: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// `start` にある `[` から始まるリンクの終わりと、その区間。
fn link_at(line: &str, start: usize) -> Option<(usize, Segment<'_>)> {
    let bytes = line.as_bytes();
    let close = matching(bytes, start, b'[', b']')?;
    let text = &line[start + 1..close];
    let image = start > 0 && bytes[start - 1] == b'!';
    match bytes.get(close + 1) {
        Some(b'(') => {
            let end = matching(bytes, close + 1, b'(', b')')? + 1;
            let raw = &line[start..end];
            Some((end, Segment::InlineLink { raw, text, image }))
        }
        Some(b'[') => {
            let end = matching(bytes, close + 1, b'[', b']')? + 1;
            let raw = &line[start..end];
            Some((end, Segment::RefLink { raw, text, image }))
        }
        _ => None,
    }
}

/// 行を、参照を探す区間とそのまま残す区間に分ける。
fn segments(line: &str) -> Vec<Segment<'_>> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let special = match bytes[i] {
            b'`' => code_span_end(bytes, i).map(|end| (end, Segment::Opaque(&line[i..end]))),
            b'[' => link_at(line, i),
            _ => None,
        };
        match special {
            Some((end, segment)) => {
                if text_start < i {
                    out.push(Segment::Text(&line[text_start..i]));
                }
                out.push(segment);
                i = end;
                text_start = end;
            }
            // 閉じられない `` ` `` の並びは、並び全体を文字として扱う
            None if bytes[i] == b'`' => {
                i += bytes[i..].iter().take_while(|&&b| b == b'`').count();
            }
            None => i += 1,
        }
    }
    if text_start < bytes.len() {
        out.push(Segment::Text(&line[text_start..]));
    }
    out
}

fn is_word(b: Option<&u8>) -> bool {
    b.is_some_and(u8::is_ascii_alphanumeric)
}

/// `at` から始まる4桁の数字（前後が英数字でないもの）。
fn number_at(text: &str, at: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let digits = bytes.get(at..at + 4)?;
    (digits.iter().all(u8::is_ascii_digit) && !is_word(bytes.get(at + 4)))
        .then(|| &text[at..at + 4])
}

/// `、` か `,` と、続く空白の後ろの位置。
fn after_separator(text: &str, at: usize) -> Option<usize> {
    let rest = &text[at..];
    let skipped = rest.strip_prefix('、').or_else(|| rest.strip_prefix(','))?;
    let spaces = skipped.len() - skipped.trim_start_matches(' ').len();
    Some(text.len() - skipped.len() + spaces)
}

/// 文字列の中の参照。`ADR 0005、0006` の続く番号も含む。
fn references(text: &str) -> Vec<Reference<'_>> {
    let bytes = text.as_bytes();
    text.match_indices("ADR ")
        .filter(|&(at, _)| at == 0 || !is_word(bytes.get(at - 1)))
        .filter_map(|(at, _)| {
            let number = number_at(text, at + 4)?;
            let first = Reference {
                start: at,
                end: at + 8,
                number,
                first: true,
            };
            let rest = std::iter::successors(Some(first), |prev| {
                let at = after_separator(text, prev.end)?;
                number_at(text, at).map(|number| Reference {
                    start: at,
                    end: at + 4,
                    number,
                    first: false,
                })
            });
            Some(rest.collect::<Vec<_>>())
        })
        .flatten()
        .collect()
}

/// 文書の参照を調べ、リンクの無い参照をリンクに置きかえた内容と、その参照の一覧を返す。
///
/// * `self_number` - 文書がADRなら、そのADRの番号（自身への参照は対象外）
/// * `target` - 番号から、文書のディレクトリからの相対パスを求める。参照先が無ければNone
pub fn scan(
    content: &str,
    self_number: Option<&str>,
    target: impl Fn(&str) -> Option<String>,
) -> (String, Vec<Finding>) {
    let mut findings = Vec::new();
    let fixed = lines_with_fences(content)
        .into_iter()
        .enumerate()
        .map(|(index, (line, verbatim))| {
            if verbatim {
                return line.to_owned();
            }
            segments(line)
                .into_iter()
                .map(|segment| match segment {
                    Segment::Text(text) => {
                        let mut out = String::new();
                        let mut last = 0;
                        for reference in references(text) {
                            if Some(reference.number) == self_number {
                                continue;
                            }
                            let Some(path) = target(reference.number) else {
                                continue;
                            };
                            findings.push(Finding {
                                line: index + 1,
                                number: reference.number.to_owned(),
                            });
                            out.push_str(&text[last..reference.start]);
                            out.push_str(&format!(
                                "[{}]({path})",
                                &text[reference.start..reference.end]
                            ));
                            last = reference.end;
                        }
                        out.push_str(&text[last..]);
                        out
                    }
                    Segment::Opaque(raw)
                    | Segment::InlineLink { raw, .. }
                    | Segment::RefLink { raw, .. } => raw.to_owned(),
                })
                .collect::<String>()
        })
        .collect();
    (fixed, findings)
}

/// 行がリンクの参照定義（行頭から3つまでの空白、`[ラベル]:`、…）か。
fn is_definition(line: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    let indent = line.len() - rest.len();
    indent <= 3
        && rest.starts_with('[')
        && matching(rest.as_bytes(), 0, b'[', b']')
            .is_some_and(|close| close > 1 && rest.as_bytes().get(close + 1) == Some(&b':'))
}

/// `--allow-links` の比較のために、リンクに関する違いを取り除く（ADR 0009、0015）。
///
/// リンク（インラインと参照形式）はテキストだけにし、画像はリンク先だけを取り除き、
/// リンクの参照定義の行は行ごと取り除く。フェンスの中とインラインのコードの中は変えない。
pub fn strip_links(content: &str) -> String {
    lines_with_fences(content)
        .into_iter()
        .map(|(line, verbatim)| {
            if verbatim {
                return line.to_owned();
            }
            if is_definition(line) {
                return String::new();
            }
            segments(line)
                .into_iter()
                .map(|segment| match segment {
                    Segment::InlineLink {
                        text, image: false, ..
                    }
                    | Segment::RefLink {
                        text, image: false, ..
                    } => text.to_owned(),
                    Segment::InlineLink {
                        text, image: true, ..
                    } => format!("[{text}]()"),
                    Segment::RefLink {
                        text, image: true, ..
                    } => format!("[{text}][]"),
                    Segment::Text(raw) | Segment::Opaque(raw) => raw.to_owned(),
                })
                .collect::<String>()
        })
        .collect()
}

/// インラインのリンク `[テキスト](リンク先)` をテキストだけにする（ADR 0012 のステータスの値の解析）。
pub fn link_texts(text: &str) -> String {
    segments(text)
        .into_iter()
        .map(|segment| match segment {
            Segment::InlineLink { text, .. } => text,
            Segment::Text(raw) | Segment::Opaque(raw) | Segment::RefLink { raw, .. } => raw,
        })
        .collect()
}

/// ADRのディレクトリの名前。この順に参照先を探す（ADR 0014）。
pub const ADR_DIR_NAMES: [&str; 4] = ["adr", "adrs", "decisions", "decision"];

/// 監査や計測の記録のディレクトリの名前（ADR 0013、0014）。
pub const RECORD_DIR_NAMES: [&str; 4] = ["audit", "audits", "benchmarks", "benchmark"];

/// 文書のパスから、参照先を探すADRのディレクトリの候補（優先する順）。
///
/// 文書のディレクトリから上にたどった各祖先（ルートを含み、`docs` の要素を含むものを除く）について、
/// 文書が `<祖先>/docs/<モジュール>/` 以下なら `<祖先>/docs/<モジュール>/<名前>` を、続けて
/// `<祖先>/docs/<名前>` を候補にする。`<名前>` は [`ADR_DIR_NAMES`] の順に並べる。
/// 近い祖先が先（ADR 0009、0011、0014）。
pub fn candidate_dirs(doc: &str) -> Vec<String> {
    let parts: Vec<&str> = doc.split('/').collect();
    let dirs = &parts[..parts.len().saturating_sub(1)];
    (0..=dirs.len())
        .rev()
        .filter(|&depth| !dirs[..depth].contains(&"docs"))
        .flat_map(|depth| {
            let prefix: String = dirs[..depth].iter().map(|dir| format!("{dir}/")).collect();
            let module = match &dirs[depth..] {
                ["docs", module, ..] if !module.is_empty() && !ADR_DIR_NAMES.contains(module) => {
                    Some(format!("{prefix}docs/{module}"))
                }
                _ => None,
            };
            module
                .into_iter()
                .chain(std::iter::once(format!("{prefix}docs")))
                .flat_map(|base| ADR_DIR_NAMES.map(|name| format!("{base}/{name}")))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `from_dir`（リポジトリのルートからのディレクトリ。ルートなら空）から `to` への相対パス。
pub fn relative(from_dir: &str, to: &str) -> String {
    let from: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let to: Vec<&str> = to.split('/').collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    std::iter::repeat_n("..", from.len() - common)
        .chain(to[common..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(content: &str) -> (String, Vec<Finding>) {
        scan(content, None, |n| Some(format!("{n}.md")))
    }

    fn fixed(content: &str) -> String {
        fix(content).0
    }

    #[test]
    fn links_a_reference() {
        assert_eq!(fixed("ADR 0001 を参照\n"), "[ADR 0001](0001.md) を参照\n");
        assert_eq!(
            fixed("ほかのADR 0001を参照"),
            "ほかの[ADR 0001](0001.md)を参照"
        );
    }

    #[test]
    fn links_listed_numbers() {
        assert_eq!(
            fixed("（ADR 0005、0006）"),
            "（[ADR 0005](0005.md)、[0006](0006.md)）"
        );
        assert_eq!(
            fixed("ADR 0005, 0006,0007"),
            "[ADR 0005](0005.md), [0006](0006.md),[0007](0007.md)"
        );
    }

    #[test]
    fn links_every_reference_in_a_line() {
        assert_eq!(
            fixed("ADR 0001 と ADR 0002"),
            "[ADR 0001](0001.md) と [ADR 0002](0002.md)"
        );
    }

    #[test]
    fn reports_line_numbers() {
        let (_, findings) = fix("a\nADR 0001\n\nADR 0002、0003\n");
        let lines: Vec<(usize, &str)> = findings
            .iter()
            .map(|f| (f.line, f.number.as_str()))
            .collect();
        assert_eq!(lines, [(2, "0001"), (4, "0002"), (4, "0003")]);
    }

    #[test]
    fn ignores_non_references() {
        for content in [
            "[ADR 0001](x.md)",
            "[参照：ADR 0001](x.md)",
            "[ADR 0001][1]",
            "`ADR 0001`",
            "`` `ADR 0001` ``",
            "```\nADR 0001\n```\n",
            "~~~\nADR 0001\n~~~\n",
            "  ```sh\n  ADR 0001\n  ```\n",
            "ADR0001",
            "ADR-0001",
            "adr 0001",
            "ADR 001",
            "ADR 00012",
            "XADR 0001",
            "ADR 0001a",
        ] {
            assert_eq!(fix(content), (content.to_owned(), vec![]), "{content:?}");
        }
    }

    #[test]
    fn checks_after_a_closed_fence() {
        assert_eq!(
            fixed("```\nADR 0001\n```\nADR 0002\n"),
            "```\nADR 0001\n```\n[ADR 0002](0002.md)\n"
        );
        assert_eq!(
            fixed("````\n```\nADR 0001\n````\nADR 0002"),
            "````\n```\nADR 0001\n````\n[ADR 0002](0002.md)"
        );
    }

    #[test]
    fn unclosed_backticks_are_text() {
        assert_eq!(fixed("`ADR 0001"), "`[ADR 0001](0001.md)");
    }

    #[test]
    fn links_after_a_linked_reference() {
        assert_eq!(
            fixed("[ADR 0001](0001.md) と ADR 0002"),
            "[ADR 0001](0001.md) と [ADR 0002](0002.md)"
        );
    }

    #[test]
    fn skips_self_references_and_unresolved_numbers() {
        let (content, findings) = scan("# ADR 0002: ADR 0001 と ADR 0099\n", Some("0002"), |n| {
            (n != "0099").then(|| format!("{n}.md"))
        });
        assert_eq!(content, "# ADR 0002: [ADR 0001](0001.md) と ADR 0099\n");
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn keeps_crlf() {
        assert_eq!(fixed("ADR 0001\r\n"), "[ADR 0001](0001.md)\r\n");
    }

    #[test]
    fn unlinks_reference_links() {
        assert_eq!(
            strip_links("[ADR 0005](a.md)、[0006](b.md)\n"),
            "ADR 0005、0006\n"
        );
        // ADR 0015 から、ADRの参照以外のリンクもテキストだけにする
        assert_eq!(strip_links("[参照](a.md)"), "参照");
        assert_eq!(strip_links("[ADR 0001 の節](a.md)"), "ADR 0001 の節");
        for content in ["`[ADR 0001](a.md)`", "```\n[ADR 0001](a.md)\n```\n"] {
            assert_eq!(strip_links(content), content, "{content:?}");
        }
    }

    #[test]
    fn strips_all_links_but_keeps_images() {
        let cases = [
            ("[a][1] と [b][]", "a と b"),
            (
                "[仕様](../specifications.md) と [本家](https://x/y)",
                "仕様 と 本家",
            ),
            ("![図](a.png)", "![図]()"),
            ("![図][1]", "![図][]"),
            (
                "前 ![図](a.png) と [リンク](b.md) 後",
                "前 ![図]() と リンク 後",
            ),
            ("`[a](b)`", "`[a](b)`"),
            ("<https://example.com>", "<https://example.com>"),
        ];
        for (content, expected) in cases {
            assert_eq!(strip_links(content), expected, "{content:?}");
        }
    }

    #[test]
    fn strips_link_reference_definitions() {
        let cases = [
            ("本文\n[1]: https://example.com\n", "本文\n"),
            ("[1]: https://example.com \"title\"\n本文\n", "本文\n"),
            ("   [label]: <https://example.com>\n", ""),
            (
                "    [label]: https://example.com\n",
                "    [label]: https://example.com\n",
            ),
            (
                "```\n[1]: https://example.com\n```\n",
                "```\n[1]: https://example.com\n```\n",
            ),
            ("[]: x\n", "[]: x\n"),
            ("[a] b: c\n", "[a] b: c\n"),
        ];
        for (content, expected) in cases {
            assert_eq!(strip_links(content), expected, "{content:?}");
        }
    }

    #[test]
    fn candidate_directories() {
        // 各候補の docs のディレクトリ（`<祖先>/docs/<モジュール>` か `<祖先>/docs`）。実際の候補は、それぞれに
        // ADR_DIR_NAMES（adr、adrs、decisions、decision）の順で名前を付けたもの（ADR 0014）。
        let cases: [(&str, &[&str]); 10] = [
            ("README.md", &["docs"]),
            ("docs/specifications.md", &["docs"]),
            ("docs/adr/0002-b.md", &["docs"]),
            ("docs/server/adr/0002-b.md", &["docs/server", "docs"]),
            ("docs/server/notes.md", &["docs/server", "docs"]),
            ("src/docs/a.md", &["src/docs", "docs"]),
            // ADR 0011
            (
                "packages/server/README.md",
                &["packages/server/docs", "packages/docs", "docs"],
            ),
            (
                "packages/server/docs/api/notes.md",
                &[
                    "packages/server/docs/api",
                    "packages/server/docs",
                    "packages/docs",
                    "docs",
                ],
            ),
            (
                "packages/server/docs/adr/0002-b.md",
                &["packages/server/docs", "packages/docs", "docs"],
            ),
            // ADR 0014: decisions はモジュールとみなさない
            ("docs/decisions/0002-b.md", &["docs"]),
        ];
        for (doc, bases) in cases {
            let expected: Vec<String> = bases
                .iter()
                .flat_map(|base| ADR_DIR_NAMES.map(|name| format!("{base}/{name}")))
                .collect();
            assert_eq!(candidate_dirs(doc), expected, "{doc}");
        }
    }

    #[test]
    fn candidate_directories_list_name_variants() {
        // ADR 0014 のテスト設計の例
        assert_eq!(
            candidate_dirs("README.md"),
            ["docs/adr", "docs/adrs", "docs/decisions", "docs/decision"]
        );
        assert_eq!(
            candidate_dirs("docs/server/notes.md"),
            [
                "docs/server/adr",
                "docs/server/adrs",
                "docs/server/decisions",
                "docs/server/decision",
                "docs/adr",
                "docs/adrs",
                "docs/decisions",
                "docs/decision",
            ]
        );
        assert_eq!(
            candidate_dirs("docs/decisions/0002-b.md"),
            ["docs/adr", "docs/adrs", "docs/decisions", "docs/decision"]
        );
    }

    #[test]
    fn relative_paths() {
        let cases = [
            ("", "docs/adr/0001-a.md", "docs/adr/0001-a.md"),
            ("docs", "docs/adr/0001-a.md", "adr/0001-a.md"),
            ("docs/adr", "docs/adr/0001-a.md", "0001-a.md"),
            ("docs/server", "docs/server/adr/0001-a.md", "adr/0001-a.md"),
            ("docs/server", "docs/adr/0001-a.md", "../adr/0001-a.md"),
            (
                "docs/server/adr",
                "docs/adr/0001-a.md",
                "../../adr/0001-a.md",
            ),
        ];
        for (from, to, expected) in cases {
            assert_eq!(relative(from, to), expected, "{from} -> {to}");
        }
    }
}
