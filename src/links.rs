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

/// 参照の誤り（ADR 0009、0019）。
#[derive(Debug, PartialEq)]
pub enum Problem {
    /// リンクが無い
    Unlinked,
    /// 修飾子から参照先が決まらない。値は理由
    Unresolved(String),
    /// 修飾子の付いたリンクのリンク先が、修飾子から決まる参照先と違う
    WrongLink { dest: String, expected: String },
    /// 修飾子がリンクの外にある
    Outside,
}

/// 文書の参照の誤り。
#[derive(Debug, PartialEq)]
pub struct Finding {
    /// 1から始まる行番号
    pub line: usize,
    pub number: String,
    /// 修飾子（ADR 0019）
    pub qualifier: Option<String>,
    pub problem: Problem,
}

impl Finding {
    fn reference(&self) -> String {
        match &self.qualifier {
            Some(q) => format!("{q} ADR {}", self.number),
            None => format!("ADR {}", self.number),
        }
    }

    /// 報告の文（パスと行を除く）。
    pub fn message(&self) -> String {
        let reference = self.reference();
        match &self.problem {
            Problem::Unlinked => format!("{reference} にリンクがありません"),
            Problem::Unresolved(reason) => format!("{reference}: {reason}"),
            Problem::WrongLink { dest, expected } => {
                format!("{reference} のリンク先 {dest} は、{expected} ではありません")
            }
            Problem::Outside => format!("{reference} の修飾子がリンクの外にあります"),
        }
    }

    /// `--fix` で直せるか。
    pub fn fixable(&self) -> bool {
        !matches!(self.problem, Problem::Unresolved(_))
    }
}

/// 参照先を探した結果（ADR 0019）。
#[derive(Debug, PartialEq)]
enum Target {
    /// 参照先のADRのリポジトリのルートからのパス
    Found(String),
    /// リンクにせず、報告もしない（参照先の無い修飾子の無い参照、自身への参照）
    Skip,
    /// 修飾子のADRのディレクトリが無い
    NoDir,
    /// ディレクトリに番号のADRが無い
    NoAdr(String),
    /// ディレクトリに番号のADRが2つ以上ある
    Many(String),
}

impl Target {
    fn reason(&self, qualifier: &str, number: &str) -> Option<String> {
        match self {
            Target::NoDir => Some(format!("{qualifier} のADRのディレクトリがありません")),
            Target::NoAdr(dir) => Some(format!("{dir} に ADR {number} がありません")),
            Target::Many(dir) => Some(format!("{dir} に ADR {number} が2つ以上あります")),
            Target::Found(_) | Target::Skip => None,
        }
    }
}

/// 参照先の探し方。
trait Resolve {
    /// 文書のパス（リポジトリのルートから）
    fn doc(&self) -> &str;
    /// 語を修飾子とみなすか
    fn qualifies(&self, word: &str) -> bool;
    fn resolve(&self, qualifier: Option<&str>, number: &str) -> Target;
}

/// 作業ツリーのADRの一覧から参照先を探す（ADR 0009、0011、0014、0019）。
struct Adrs<'a> {
    doc: &'a str,
    adrs: &'a [String],
    dirs: std::collections::HashSet<&'a str>,
    self_number: Option<&'a str>,
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

impl Adrs<'_> {
    /// `dir` の直下の `<番号>-*.md`。
    fn in_dir(&self, dir: &str, number: &str) -> Vec<&String> {
        let prefix = format!("{dir}/{number}-");
        self.adrs
            .iter()
            .filter(|adr| adr.starts_with(&prefix) && !adr[prefix.len()..].contains('/'))
            .collect()
    }

    fn module_dir(&self, name: &str) -> Option<String> {
        ancestor_prefixes(self.doc)
            .into_iter()
            .flat_map(|prefix| ADR_DIR_NAMES.map(|adr| format!("{prefix}docs/{name}/{adr}")))
            .find(|dir| self.dirs.contains(dir.as_str()))
    }

    fn path_dir(&self, path: &str) -> Option<String> {
        let mut path = path;
        while let Some(rest) = path.strip_prefix("./") {
            path = rest;
        }
        let path = path.trim_end_matches('/');
        std::iter::once(path.to_owned())
            .chain(ADR_DIR_NAMES.map(|adr| format!("{path}/{adr}")))
            .find(|dir| self.dirs.contains(dir.as_str()))
    }
}

impl Resolve for Adrs<'_> {
    fn doc(&self) -> &str {
        self.doc
    }

    fn qualifies(&self, word: &str) -> bool {
        word.contains(['/', '-']) || self.module_dir(word).is_some()
    }

    fn resolve(&self, qualifier: Option<&str>, number: &str) -> Target {
        let Some(qualifier) = qualifier else {
            if Some(number) == self.self_number {
                return Target::Skip;
            }
            return candidate_dirs(self.doc)
                .into_iter()
                .find_map(|dir| match self.in_dir(&dir, number).as_slice() {
                    [only] => Some(Target::Found((*only).clone())),
                    _ => None,
                })
                .unwrap_or(Target::Skip);
        };
        let dir = if qualifier.contains('/') {
            self.path_dir(qualifier)
        } else {
            self.module_dir(qualifier)
        };
        let Some(dir) = dir else {
            return Target::NoDir;
        };
        match self.in_dir(&dir, number).as_slice() {
            [] => Target::NoAdr(dir),
            [only] => Target::Found((*only).clone()),
            _ => Target::Many(dir),
        }
    }
}

/// 文書の参照を調べ、直せるものを直した内容と、誤りの一覧を返す（ADR 0009、0019）。
///
/// * `doc` - 文書のパス（リポジトリのルートから）
/// * `adrs` - 作業ツリーのADRのパスの一覧
/// * `self_number` - 文書がADRなら、そのADRの番号（修飾子の無い自身への参照は対象外）
pub fn check(
    doc: &str,
    content: &str,
    adrs: &[String],
    self_number: Option<&str>,
) -> (String, Vec<Finding>) {
    let resolver = Adrs {
        doc,
        adrs,
        dirs: adrs.iter().map(|adr| dir_of(adr)).collect(),
        self_number,
    };
    scan_with(content, &resolver)
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

/// 修飾子を見ない探し方（ADR 0009 の単体テスト）。
#[cfg(test)]
struct Unqualified<'a, F> {
    self_number: Option<&'a str>,
    target: F,
}

#[cfg(test)]
impl<F: Fn(&str) -> Option<String>> Resolve for Unqualified<'_, F> {
    fn doc(&self) -> &str {
        "x.md"
    }

    fn qualifies(&self, _: &str) -> bool {
        false
    }

    fn resolve(&self, _: Option<&str>, number: &str) -> Target {
        if Some(number) == self.self_number {
            return Target::Skip;
        }
        (self.target)(number).map_or(Target::Skip, Target::Found)
    }
}

/// 修飾子を見ずに、リンクの無い参照をリンクに置きかえる（ADR 0009）。
///
/// * `self_number` - 文書がADRなら、そのADRの番号（自身への参照は対象外）
/// * `target` - 番号から、文書のディレクトリからの相対パスを求める。参照先が無ければNone
#[cfg(test)]
pub fn scan(
    content: &str,
    self_number: Option<&str>,
    target: impl Fn(&str) -> Option<String>,
) -> (String, Vec<Finding>) {
    scan_with(
        content,
        &Unqualified {
            self_number,
            target,
        },
    )
}

/// 修飾子の語の文字か（ADR 0019）。
fn is_qualifier_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-')
}

/// `text[..end]` が `<語> ` で終わるなら、語の始まりと語（ADR 0019）。
fn word_before(text: &str, end: usize) -> Option<(usize, &str)> {
    let head = text[..end].strip_suffix(' ')?;
    let len = head
        .bytes()
        .rev()
        .take_while(|&b| is_qualifier_char(b))
        .count();
    let start = head.len() - len;
    let word = &head[start..];
    word.as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        .then_some((start, word))
}

/// リンクのテキストが `<語> ADR NNNN` で始まるなら、語と番号（ADR 0019）。
fn qualified_text(text: &str) -> Option<(&str, &str)> {
    let (word, rest) = text.split_once(' ')?;
    let valid = word.bytes().all(is_qualifier_char)
        && word
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric);
    let number = rest.strip_prefix("ADR ").and_then(|r| number_at(r, 0))?;
    valid.then_some((word, number))
}

/// `dir` からの相対パス `path` の、リポジトリのルートからのパス。
fn join(dir: &str, path: &str) -> String {
    let (base, path) = match path.strip_prefix('/') {
        Some(path) => ("", path),
        None => (dir, path),
    };
    base.split('/')
        .chain(path.split('/'))
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

/// 文書の参照を調べ、直せるものを直した内容と、誤りの一覧を返す。
fn scan_with(content: &str, resolver: &impl Resolve) -> (String, Vec<Finding>) {
    let mut findings = Vec::new();
    let fixed = lines_with_fences(content)
        .into_iter()
        .enumerate()
        .map(|(index, (line, verbatim))| {
            if verbatim {
                line.to_owned()
            } else {
                scan_line(line, index + 1, resolver, &mut findings)
            }
        })
        .collect();
    (fixed, findings)
}

fn scan_line(
    line: &str,
    line_number: usize,
    resolver: &impl Resolve,
    findings: &mut Vec<Finding>,
) -> String {
    let doc = resolver.doc();
    let doc_dir = dir_of(doc);
    let finding = |number: &str, qualifier: Option<&str>, problem| Finding {
        line: line_number,
        number: number.to_owned(),
        qualifier: qualifier.map(str::to_owned),
        problem,
    };
    let segments = segments(line);
    let mut out: Vec<String> = Vec::new();
    // 列挙の続く番号に引きつぐ修飾子
    let mut chain: Option<&str> = None;
    for (i, segment) in segments.iter().enumerate() {
        match *segment {
            Segment::Text(text) => {
                let mut fixed = String::new();
                let mut last = 0;
                let mut qualifier: Option<(usize, &str)> = None;
                let mut last_qualified: Option<(usize, &str)> = None;
                let references = references(text);
                for reference in &references {
                    if reference.first {
                        qualifier = word_before(text, reference.start)
                            .filter(|&(_, word)| resolver.qualifies(word));
                    }
                    let q = qualifier.map(|(_, word)| word);
                    last_qualified = q.map(|word| (reference.end, word));
                    let start = match qualifier {
                        Some((start, _)) if reference.first => start,
                        _ => reference.start,
                    };
                    match resolver.resolve(q, reference.number) {
                        Target::Found(path) if q.is_some() && path == doc => {}
                        Target::Found(path) => {
                            findings.push(finding(reference.number, q, Problem::Unlinked));
                            fixed.push_str(&text[last..start]);
                            fixed.push_str(&format!(
                                "[{}]({})",
                                &text[start..reference.end],
                                relative(doc_dir, &path)
                            ));
                            last = reference.end;
                        }
                        Target::Skip => {}
                        target => {
                            let reason = target.reason(q.unwrap_or(""), reference.number);
                            if let Some(reason) = reason {
                                findings.push(finding(
                                    reference.number,
                                    q,
                                    Problem::Unresolved(reason),
                                ));
                            }
                        }
                    }
                }
                fixed.push_str(&text[last..]);
                out.push(fixed);
                let only_separator = after_separator(text, 0) == Some(text.len());
                chain = if references.is_empty() && only_separator {
                    chain
                } else {
                    last_qualified
                        .filter(|&(end, _)| after_separator(text, end) == Some(text.len()))
                        .map(|(_, word)| word)
                };
            }
            Segment::InlineLink {
                raw,
                text,
                image: false,
            } => {
                let previous = match i.checked_sub(1).map(|j| &segments[j]) {
                    Some(Segment::Text(previous)) => Some(*previous),
                    _ => None,
                };
                let inside = qualified_text(text).filter(|&(word, _)| resolver.qualifies(word));
                let outside = || {
                    let number = text.strip_prefix("ADR ").and_then(|r| number_at(r, 0))?;
                    let previous = previous?;
                    let (_, word) = word_before(previous, previous.len())?;
                    resolver.qualifies(word).then_some((word, number))
                };
                let continued = || {
                    let q = chain?;
                    (text.len() == 4 && number_at(text, 0).is_some()).then_some((q, text))
                };
                let (kind, (q, number)) = match (inside, outside(), continued()) {
                    (Some(found), _, _) => (Kind::Inside, found),
                    (None, Some(found), _) => (Kind::Outside, found),
                    (None, None, Some(found)) => (Kind::Continued, found),
                    (None, None, None) => {
                        out.push(raw.to_owned());
                        chain = None;
                        continue;
                    }
                };
                chain = Some(q);
                let dest = &raw[text.len() + 3..raw.len() - 1];
                if dest.contains("://") {
                    out.push(raw.to_owned());
                    continue;
                }
                let (path, fragment) = match dest.find('#') {
                    Some(at) => dest.split_at(at),
                    None => (dest, ""),
                };
                let expected = match resolver.resolve(Some(q), number) {
                    Target::Found(expected) => expected,
                    target => {
                        if let Some(reason) = target.reason(q, number) {
                            findings.push(finding(number, Some(q), Problem::Unresolved(reason)));
                        }
                        out.push(raw.to_owned());
                        continue;
                    }
                };
                let wrong = join(doc_dir, path) != expected;
                let moved = kind == Kind::Outside;
                if wrong {
                    findings.push(finding(
                        number,
                        Some(q),
                        Problem::WrongLink {
                            dest: dest.to_owned(),
                            expected: expected.clone(),
                        },
                    ));
                } else if moved {
                    findings.push(finding(number, Some(q), Problem::Outside));
                } else {
                    out.push(raw.to_owned());
                    continue;
                }
                let new_dest = if wrong {
                    format!("{}{fragment}", relative(doc_dir, &expected))
                } else {
                    dest.to_owned()
                };
                let new_text = if moved {
                    if let Some(previous) = out.last_mut() {
                        previous.truncate(previous.len() - q.len() - 1);
                    }
                    format!("{q} {text}")
                } else {
                    text.to_owned()
                };
                out.push(format!("[{new_text}]({new_dest})"));
            }
            Segment::Opaque(raw)
            | Segment::InlineLink { raw, .. }
            | Segment::RefLink { raw, .. } => {
                out.push(raw.to_owned());
                chain = None;
            }
        }
    }
    out.concat()
}

/// 修飾子の付いたリンクの形（ADR 0019）。
#[derive(PartialEq)]
enum Kind {
    Inside,
    Outside,
    Continued,
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

/// インラインのリンク（画像を除く）を、`f(テキスト, リンク先)` が返す新しいテキストとリンク先に置きかえる。
/// `f` が None を返すリンクと、フェンスの中やインラインのコードの中は変えない。置きかえた数も返す（ADR 0018）。
pub fn map_inline_links(
    content: &str,
    f: impl Fn(&str, &str) -> Option<(String, String)>,
) -> (String, usize) {
    let mut count = 0;
    let mapped = lines_with_fences(content)
        .into_iter()
        .map(|(line, verbatim)| {
            if verbatim {
                return line.to_owned();
            }
            segments(line)
                .into_iter()
                .map(|segment| match segment {
                    Segment::InlineLink {
                        raw,
                        text,
                        image: false,
                    } => {
                        let dest = &raw[text.len() + 3..raw.len() - 1];
                        match f(text, dest) {
                            Some((text, dest)) => {
                                count += 1;
                                format!("[{text}]({dest})")
                            }
                            None => raw.to_owned(),
                        }
                    }
                    Segment::Text(raw)
                    | Segment::Opaque(raw)
                    | Segment::InlineLink { raw, .. }
                    | Segment::RefLink { raw, .. } => raw.to_owned(),
                })
                .collect::<String>()
        })
        .collect();
    (mapped, count)
}

/// リンクの無いADRの参照の、行番号と番号（参照先を解決しない。ADR 0018）。
pub fn unlinked_numbers(content: &str) -> Vec<(usize, String)> {
    lines_with_fences(content)
        .into_iter()
        .enumerate()
        .filter(|(_, (_, verbatim))| !verbatim)
        .flat_map(|(index, (line, _))| {
            segments(line)
                .into_iter()
                .filter_map(|segment| match segment {
                    Segment::Text(text) => Some(text),
                    _ => None,
                })
                .flat_map(references)
                .map(|reference| (index + 1, reference.number.to_owned()))
                .collect::<Vec<_>>()
        })
        .collect()
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

/// 文書のディレクトリから上にたどった祖先のうち、`docs` の要素を含まないものの深さ（近い順）。
fn ancestor_depths(doc: &str) -> Vec<usize> {
    let parts: Vec<&str> = doc.split('/').collect();
    let dirs = &parts[..parts.len().saturating_sub(1)];
    (0..=dirs.len())
        .rev()
        .filter(|&depth| !dirs[..depth].contains(&"docs"))
        .collect()
}

/// [`ancestor_depths`] の各祖先の、末尾に `/` を付けたパス（ルートは空。ADR 0019）。
fn ancestor_prefixes(doc: &str) -> Vec<String> {
    let parts: Vec<&str> = doc.split('/').collect();
    ancestor_depths(doc)
        .into_iter()
        .map(|depth| parts[..depth].iter().map(|dir| format!("{dir}/")).collect())
        .collect()
}

/// 文書のパスから、参照先を探すADRのディレクトリの候補（優先する順）。
///
/// 文書のディレクトリから上にたどった各祖先（ルートを含み、`docs` の要素を含むものを除く）について、
/// 文書が `<祖先>/docs/<モジュール>/` 以下なら `<祖先>/docs/<モジュール>/<名前>` を、続けて
/// `<祖先>/docs/<名前>` を候補にする。`<名前>` は [`ADR_DIR_NAMES`] の順に並べる。
/// 近い祖先が先（ADR 0009、0011、0014）。
pub fn candidate_dirs(doc: &str) -> Vec<String> {
    let parts: Vec<&str> = doc.split('/').collect();
    let dirs = &parts[..parts.len().saturating_sub(1)];
    ancestor_depths(doc)
        .into_iter()
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

    // ADR 0019: 修飾子の付いた参照

    const GOLEM_ADRS: [&str; 7] = [
        "docs/adr/0008-first-release-verification.md",
        "docs/adr/0015-regex-styles.md",
        "docs/adr/0187-x.md",
        "docs/golem-driver/adr/0015-vertical-dashes.md",
        "docs/golem-driver/adr/0016-y.md",
        "docs/golem-doc/adr/0008-multiple-index-and-bibliography.md",
        "docs/golem-html/adr/0003-cite-links-across-files.md",
    ];

    fn golem(doc: &str, content: &str) -> (String, Vec<String>) {
        let adrs: Vec<String> = GOLEM_ADRS.iter().map(|s| s.to_string()).collect();
        let (fixed, findings) = check(doc, content, &adrs, None);
        (
            fixed,
            findings
                .iter()
                .map(|f| format!("{}: {}", f.line, f.message()))
                .collect(),
        )
    }

    fn golem_fixed(doc: &str, content: &str) -> String {
        golem(doc, content).0
    }

    #[test]
    fn links_qualified_references_to_their_directory() {
        let cases = [
            (
                "docs/adr/x.md",
                "（golem-driver ADR 0015）",
                "（[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)）",
            ),
            (
                "docs/golem-html/adr/x.md",
                "golem-doc ADR 0008 を参照",
                "[golem-doc ADR 0008](../../golem-doc/adr/0008-multiple-index-and-bibliography.md) を参照",
            ),
            (
                "docs/specifications.md",
                "docs/adr ADR 0187",
                "[docs/adr ADR 0187](adr/0187-x.md)",
            ),
            (
                "docs/specifications.md",
                "docs/golem-driver ADR 0015",
                "[docs/golem-driver ADR 0015](golem-driver/adr/0015-vertical-dashes.md)",
            ),
            (
                "README.md",
                "docs/adr/ ADR 0187",
                "[docs/adr/ ADR 0187](docs/adr/0187-x.md)",
            ),
            // 列挙の続く番号は、最初の番号の修飾子で解決する
            (
                "docs/adr/x.md",
                "golem-driver ADR 0015、0016",
                "[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)、[0016](../golem-driver/adr/0016-y.md)",
            ),
        ];
        for (doc, content, expected) in cases {
            assert_eq!(golem_fixed(doc, content), expected, "{doc}: {content}");
        }
    }

    #[test]
    fn reports_qualified_unlinked_references() {
        assert_eq!(
            golem("docs/adr/x.md", "a\ngolem-driver ADR 0015\n").1,
            ["2: golem-driver ADR 0015 にリンクがありません"]
        );
    }

    #[test]
    fn module_names_without_hyphens_qualify_only_when_the_module_exists() {
        let adrs: Vec<String> = ["docs/adr/0001-a.md", "docs/server/adr/0001-b.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let fixed = |content: &str| check("docs/adr/x.md", content, &adrs, None).0;
        assert_eq!(
            fixed("server ADR 0001"),
            "[server ADR 0001](../server/adr/0001-b.md)"
        );
        assert_eq!(fixed("see ADR 0001"), "see [ADR 0001](0001-a.md)");
    }

    #[test]
    fn does_not_qualify_other_words() {
        for (content, expected) in [
            ("- ADR 0015", "- [ADR 0015](0015-regex-styles.md)"),
            (
                "golem-driverのADR 0015",
                "golem-driverの[ADR 0015](0015-regex-styles.md)",
            ),
            (
                "golem-driver  ADR 0015",
                "golem-driver  [ADR 0015](0015-regex-styles.md)",
            ),
        ] {
            assert_eq!(golem_fixed("docs/adr/x.md", content), expected, "{content}");
        }
    }

    #[test]
    fn reports_unresolved_qualified_references() {
        let content = "golem-nope ADR 0001\ngolem-driver ADR 0099\ndocs/nope ADR 0001\n";
        let (fixed, findings) = golem("docs/adr/x.md", content);
        assert_eq!(fixed, content);
        assert_eq!(
            findings,
            [
                "1: golem-nope ADR 0001: golem-nope のADRのディレクトリがありません",
                "2: golem-driver ADR 0099: docs/golem-driver/adr に ADR 0099 がありません",
                "3: docs/nope ADR 0001: docs/nope のADRのディレクトリがありません",
            ]
        );
    }

    #[test]
    fn reports_duplicated_qualified_targets() {
        let adrs: Vec<String> = ["docs/m-a/adr/0001-a.md", "docs/m-a/adr/0001-b.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (_, findings) = check("README.md", "m-a ADR 0001", &adrs, None);
        assert_eq!(
            findings.iter().map(Finding::message).collect::<Vec<_>>(),
            ["m-a ADR 0001: docs/m-a/adr に ADR 0001 が2つ以上あります"]
        );
    }

    #[test]
    fn keeps_unqualified_references() {
        assert_eq!(
            golem_fixed("docs/golem-driver/notes.md", "ADR 0015 と ADR 0187"),
            "[ADR 0015](adr/0015-vertical-dashes.md) と [ADR 0187](../adr/0187-x.md)"
        );
    }

    #[test]
    fn skips_qualified_self_references() {
        let adrs: Vec<String> = GOLEM_ADRS.iter().map(|s| s.to_string()).collect();
        let doc = "docs/golem-driver/adr/0015-vertical-dashes.md";
        let content = "# ADR 0015: x\n\ngolem-driver ADR 0015\n";
        assert_eq!(
            check(doc, content, &adrs, Some("0015")),
            (content.to_owned(), vec![])
        );
    }

    #[test]
    fn fixes_wrong_qualified_links() {
        let (fixed, findings) = golem(
            "docs/adr/0172-x.md",
            "（golem-driver [ADR 0015](0015-regex-styles.md)）",
        );
        assert_eq!(
            fixed,
            "（[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)）"
        );
        assert_eq!(
            findings,
            [
                "1: golem-driver ADR 0015 のリンク先 0015-regex-styles.md は、docs/golem-driver/adr/0015-vertical-dashes.md ではありません"
            ]
        );
        let (fixed, findings) = golem(
            "docs/golem-html/adr/0003-x.md",
            "golem-doc [ADR 0008](../../adr/0008-first-release-verification.md) が",
        );
        assert_eq!(
            fixed,
            "[golem-doc ADR 0008](../../golem-doc/adr/0008-multiple-index-and-bibliography.md) が"
        );
        assert_eq!(findings.len(), 1);
        let (fixed, findings) = golem(
            "docs/adr/0172-x.md",
            "[golem-driver ADR 0015 の節](0015-regex-styles.md#a)",
        );
        assert_eq!(
            fixed,
            "[golem-driver ADR 0015 の節](../golem-driver/adr/0015-vertical-dashes.md#a)"
        );
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn moves_qualifiers_into_links() {
        let (fixed, findings) = golem(
            "docs/adr/x.md",
            "golem-driver [ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)",
        );
        assert_eq!(
            fixed,
            "[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)"
        );
        assert_eq!(
            findings,
            ["1: golem-driver ADR 0015 の修飾子がリンクの外にあります"]
        );
    }

    #[test]
    fn checks_listed_qualified_links() {
        let (fixed, findings) = golem(
            "docs/adr/x.md",
            "[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)、[0016](0016-z.md)",
        );
        assert_eq!(
            fixed,
            "[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)、[0016](../golem-driver/adr/0016-y.md)"
        );
        assert_eq!(
            findings,
            [
                "1: golem-driver ADR 0016 のリンク先 0016-z.md は、docs/golem-driver/adr/0016-y.md ではありません"
            ]
        );
    }

    #[test]
    fn reports_unresolved_qualified_links() {
        let content = "[golem-nope ADR 0001](0001-a.md)";
        let (fixed, findings) = golem("docs/adr/x.md", content);
        assert_eq!(fixed, content);
        assert_eq!(
            findings,
            ["1: golem-nope ADR 0001: golem-nope のADRのディレクトリがありません"]
        );
    }

    #[test]
    fn accepts_correct_and_unchecked_links() {
        for (doc, content) in [
            (
                "docs/adr/x.md",
                "[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)、[0016](../golem-driver/adr/0016-y.md)",
            ),
            (
                "README.md",
                "[golem-driver ADR 0015](./docs/golem-driver/adr/0015-vertical-dashes.md)",
            ),
            (
                "docs/adr/x.md",
                "[shem ADR 0001](https://example.com/0001.md)",
            ),
            (
                "docs/adr/x.md",
                "[peg-hs ADR 0039](https://example.com/0039.md)",
            ),
            ("docs/adr/x.md", "[ADR 0015](0015-regex-styles.md)"),
            ("docs/adr/x.md", "`golem-driver ADR 0015`"),
        ] {
            assert_eq!(
                golem(doc, content),
                (content.to_owned(), vec![]),
                "{content}"
            );
        }
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
