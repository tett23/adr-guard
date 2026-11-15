//! ADRのステータスの行の判定と、ステータスの値の解釈（ADR 0012）。
//!
//! ステータスの行は、行頭が `ステータス:` か `Status:` の行。ADRはステータスの行をちょうど1つ持つ。

use crate::links;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    /// `提案…`、`Proposed`。レビュー待ち
    Proposed,
    /// `採択…`（`採択→`、`採択（未実装）` で始まるものを除く）、`Accepted`
    Accepted,
    /// `採択（未実装）…`、`Accepted (not implemented)`。採択されたが、まだ実装されていない（ADR 0016）
    AcceptedNotImplemented,
    /// `採択→…`、`Superseded by …`。一部または全部が後のADRで変更された
    Superseded(Vec<Change>),
    /// `superseded-pending [...]`。採択して実装を待つまま、後のADRの変更を受けた（ADR 0025）
    SupersededPending(Vec<Change>),
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
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Change {
    /// 変わった部分。全体が変わった場合や書かれていない場合はNone
    pub part: Option<String>,
    /// 変更したADRの番号（4桁の数字）か、修正先（ルートからのパスか `https://` の URL。ADR 0025）
    pub by: Vec<String>,
    /// 変更の種類（ADR 0025）
    pub kind: Kind,
    /// 注記（ADR 0025）
    pub note: Option<String>,
}

/// 変更の種類（ADR 0025）。省いたときは置換。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Kind {
    #[default]
    Replace,
    Supplement,
    Note,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Supplement => "supplement",
            Self::Note => "note",
        }
    }
}

/// 修正先（ADR 0025）。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target<'a> {
    /// リポジトリのルートからの相対パス
    Path(&'a str),
    /// `https://` で始まる、別のリポジトリなどの URL。たどらない
    Url(&'a str),
}

impl<'a> Target<'a> {
    pub fn of(by: &'a str) -> Self {
        if by.starts_with("https://") {
            Self::Url(by)
        } else {
            Self::Path(by)
        }
    }
}

impl Change {
    /// 旧形式から読んだ変更（種類は置換、注記なし）。
    pub fn legacy(part: Option<String>, by: Vec<String>) -> Self {
        Self {
            part,
            by,
            kind: Kind::Replace,
            note: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdrState {
    Absent,
    Present(Status),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Transition {
    Propose,
    Accept,
    AcceptPending,
    Reject,
    Discard,
    Complete,
    Revise {
        origin: ReviseOrigin,
        changes: Vec<Change>,
    },
    /// Pending から SupersededPending（ADR 0025）
    RevisePending {
        changes: Vec<Change>,
    },
    /// `pending` なら SupersededPending どうし（ADR 0025）
    UpdateRevisions {
        old: Vec<Change>,
        new: Vec<Change>,
        pending: bool,
    },
    /// SupersededPending から同じ変更の Superseded（ADR 0025）
    CompleteRevised {
        changes: Vec<Change>,
    },
    Deprecate {
        origin: DeprecateOrigin,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviseOrigin {
    Accepted,
    Pending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeprecateOrigin {
    Accepted,
    Pending,
    Superseded(Vec<Change>),
    SupersededPending(Vec<Change>),
}

impl Transition {
    #[cfg(test)]
    pub fn endpoints(self) -> (AdrState, AdrState) {
        use AdrState::{Absent, Present};
        use Status::{
            Accepted, AcceptedNotImplemented, Deprecated, Discarded, Proposed, Rejected,
            Superseded, SupersededPending,
        };
        match self {
            Self::Propose => (Absent, Present(Proposed)),
            Self::Accept => (Present(Proposed), Present(Accepted)),
            Self::AcceptPending => (Present(Proposed), Present(AcceptedNotImplemented)),
            Self::Reject => (Present(Proposed), Present(Rejected)),
            Self::Discard => (Present(Proposed), Present(Discarded)),
            Self::Complete => (Present(AcceptedNotImplemented), Present(Accepted)),
            Self::Revise { origin, changes } => (
                Present(match origin {
                    ReviseOrigin::Accepted => Accepted,
                    ReviseOrigin::Pending => AcceptedNotImplemented,
                }),
                Present(Superseded(changes)),
            ),
            Self::RevisePending { changes } => (
                Present(AcceptedNotImplemented),
                Present(SupersededPending(changes)),
            ),
            Self::UpdateRevisions {
                old,
                new,
                pending: false,
            } => (Present(Superseded(old)), Present(Superseded(new))),
            Self::UpdateRevisions {
                old,
                new,
                pending: true,
            } => (
                Present(SupersededPending(old)),
                Present(SupersededPending(new)),
            ),
            Self::CompleteRevised { changes } => (
                Present(SupersededPending(changes.clone())),
                Present(Superseded(changes)),
            ),
            Self::Deprecate { origin } => (
                Present(match origin {
                    DeprecateOrigin::Accepted => Accepted,
                    DeprecateOrigin::Pending => AcceptedNotImplemented,
                    DeprecateOrigin::Superseded(changes) => Superseded(changes),
                    DeprecateOrigin::SupersededPending(changes) => SupersededPending(changes),
                }),
                Present(Deprecated),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Classification {
    Unchanged,
    Direct(Transition),
    Unclassified,
}

pub fn classify(before: AdrState, after: AdrState) -> Classification {
    use AdrState::{Absent, Present};
    use Classification::{Direct, Unchanged, Unclassified};
    use Status::{
        Accepted, AcceptedNotImplemented, Deprecated, Discarded, Proposed, Rejected, Superseded,
        SupersededPending, Unknown,
    };
    match (before, after) {
        (Present(Unknown), _) | (_, Present(Unknown)) => Unclassified,
        (Absent, Absent) => Unchanged,
        (Present(old), Present(new)) if old == new => Unchanged,
        (Absent, Present(Proposed)) => Direct(Transition::Propose),
        (Present(Proposed), Present(Accepted)) => Direct(Transition::Accept),
        (Present(Proposed), Present(AcceptedNotImplemented)) => Direct(Transition::AcceptPending),
        (Present(Proposed), Present(Rejected)) => Direct(Transition::Reject),
        (Present(Proposed), Present(Discarded)) => Direct(Transition::Discard),
        (Present(AcceptedNotImplemented), Present(Accepted)) => Direct(Transition::Complete),
        (Present(Accepted), Present(Superseded(changes))) => Direct(Transition::Revise {
            origin: ReviseOrigin::Accepted,
            changes,
        }),
        (Present(AcceptedNotImplemented), Present(Superseded(changes))) => {
            Direct(Transition::Revise {
                origin: ReviseOrigin::Pending,
                changes,
            })
        }
        (Present(Superseded(old)), Present(Superseded(new))) => {
            Direct(Transition::UpdateRevisions {
                old,
                new,
                pending: false,
            })
        }
        (Present(AcceptedNotImplemented), Present(SupersededPending(changes))) => {
            Direct(Transition::RevisePending { changes })
        }
        (Present(SupersededPending(old)), Present(SupersededPending(new))) => {
            Direct(Transition::UpdateRevisions {
                old,
                new,
                pending: true,
            })
        }
        (Present(SupersededPending(old)), Present(Superseded(new))) if old == new => {
            Direct(Transition::CompleteRevised { changes: new })
        }
        (Present(Accepted), Present(Deprecated)) => Direct(Transition::Deprecate {
            origin: DeprecateOrigin::Accepted,
        }),
        (Present(AcceptedNotImplemented), Present(Deprecated)) => Direct(Transition::Deprecate {
            origin: DeprecateOrigin::Pending,
        }),
        (Present(Superseded(changes)), Present(Deprecated)) => Direct(Transition::Deprecate {
            origin: DeprecateOrigin::Superseded(changes),
        }),
        (Present(SupersededPending(changes)), Present(Deprecated)) => {
            Direct(Transition::Deprecate {
                origin: DeprecateOrigin::SupersededPending(changes),
            })
        }
        _ => Unclassified,
    }
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
        [(line, _)] => {
            let original = content
                .lines()
                .nth(line - 1)
                .unwrap_or_default()
                .trim_end_matches('\r');
            if is_reserved(original.strip_prefix("Status: ").unwrap_or_default()) {
                parse_canonical_line(original)
                    .err()
                    .map(|reason| vec![format!("{path}:{line}: {reason}")])
                    .unwrap_or_default()
            } else {
                vec![]
            }
        }
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

/// 括弧（全角か半角）で囲んだ `inner` で始まるか（ADR 0016）。`ignore_case` なら英字の大文字と小文字を区別しない。
fn bracketed(text: &str, inner: &str, ignore_case: bool) -> bool {
    let Some(rest) = text.strip_prefix(['（', '(']) else {
        return false;
    };
    let Some(head) = rest.get(..inner.len()) else {
        return false;
    };
    let same = if ignore_case {
        head.eq_ignore_ascii_case(inner)
    } else {
        head == inner
    };
    same && rest[inner.len()..].starts_with(['）', ')'])
}

/// `採択（未実装）…` か `Accepted (not implemented)…` か（ADR 0016）。
/// 英語では、`Accepted` と括弧の間の半角の空白は1つあってもなくてもよい（`Accepted（not implemented）`）。
fn accepted_not_implemented(raw: &str) -> bool {
    raw.strip_prefix("採択")
        .is_some_and(|rest| bracketed(rest, "未実装", false))
        || english(raw, "accepted")
            .map(|rest| rest.strip_prefix(' ').unwrap_or(rest))
            .is_some_and(|rest| bracketed(rest, "not implemented", true))
}

/// 元の値を状態に変換する。
pub fn parse(raw: &str) -> Status {
    if let Some(status) = parse_canonical_value(raw) {
        status
    } else if is_reserved(raw) {
        Status::Unknown
    } else if raw.starts_with("提案") {
        Status::Proposed
    } else if accepted_not_implemented(raw) {
        Status::AcceptedNotImplemented
    } else if let Some(rest) = superseded_ja(raw) {
        Status::Superseded(changes_ja(rest))
    } else if raw.starts_with("採択") {
        Status::Accepted
    } else if let Some(rest) = raw.strip_prefix("置換") {
        Status::Superseded(changes_ja(&replaced_body(rest)))
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

/// 新形式の行だけを受け付ける。新規または変更された行の検査にも使う。
pub fn parse_canonical_line(line: &str) -> Result<Status, String> {
    let raw = line
        .strip_prefix("Status: ")
        .ok_or_else(|| "新形式のステータス行は `Status: ` で始めてください".to_owned())?;
    parse_canonical_value(raw).ok_or_else(|| "新形式のステータス値が不正です".to_owned())
}

/// JSON の誤りを旧形式に読み替えない、新形式の予約した接頭辞（ADR 0022、0025）。
pub fn is_reserved(raw: &str) -> bool {
    raw.starts_with("superseded [") || raw.starts_with("superseded-pending [")
}

fn parse_canonical_value(raw: &str) -> Option<Status> {
    let simple = match raw {
        "proposed" => Some(Status::Proposed),
        "accepted" => Some(Status::Accepted),
        "accepted-pending" => Some(Status::AcceptedNotImplemented),
        "rejected" => Some(Status::Rejected),
        "discarded" => Some(Status::Discarded),
        "deprecated" => Some(Status::Deprecated),
        _ => None,
    };
    simple.or_else(|| {
        let (json, pending) = raw
            .strip_prefix("superseded-pending ")
            .map(|json| (json, true))
            .or_else(|| raw.strip_prefix("superseded ").map(|json| (json, false)))?;
        if !json.starts_with('[') {
            return None;
        }
        let changes: Vec<CanonicalChange> = serde_json::from_str(json).ok()?;
        let changes: Vec<Change> = changes.into_iter().map(|item| item.0).collect();
        match (changes.is_empty(), pending) {
            (true, _) => None,
            (false, true) => Some(Status::SupersededPending(changes)),
            (false, false) => Some(Status::Superseded(changes)),
        }
    })
}

struct CanonicalChange(Change);

impl<'de> Deserialize<'de> for CanonicalChange {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ChangeVisitor;

        impl<'de> Visitor<'de> for ChangeVisitor {
            type Value = CanonicalChange;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("part、by、kind、note を持つ変更")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                const FIELDS: &[&str] = &["part", "by", "kind", "note"];
                let mut part: Option<Option<String>> = None;
                let mut by: Option<Vec<String>> = None;
                let mut kind: Option<String> = None;
                let mut note: Option<String> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "part" if part.is_none() => part = Some(map.next_value()?),
                        "by" if by.is_none() => by = Some(map.next_value()?),
                        "kind" if kind.is_none() => kind = Some(map.next_value()?),
                        "note" if note.is_none() => note = Some(map.next_value()?),
                        "part" | "by" | "kind" | "note" => {
                            return Err(de::Error::custom("重複したキー"));
                        }
                        _ => return Err(de::Error::unknown_field(&key, FIELDS)),
                    }
                }
                let part = part.ok_or_else(|| de::Error::missing_field("part"))?;
                let by = by.ok_or_else(|| de::Error::missing_field("by"))?;
                let kind = match kind.as_deref() {
                    None | Some("replace") => Kind::Replace,
                    Some("supplement") => Kind::Supplement,
                    Some("note") => Kind::Note,
                    Some(other) => {
                        return Err(de::Error::unknown_variant(
                            other,
                            &["replace", "supplement", "note"],
                        ));
                    }
                };
                let valid_note = note
                    .as_ref()
                    .is_none_or(|note| !note.is_empty() && !note.contains(['\n', '\r']));
                let valid_by = match kind {
                    Kind::Note => by.is_empty() && note.is_some(),
                    Kind::Replace | Kind::Supplement => !by.is_empty(),
                };
                if part.as_ref().is_some_and(String::is_empty)
                    || !valid_note
                    || !valid_by
                    || by.iter().any(|target| !valid_target(target))
                {
                    return Err(de::Error::custom("変更部分、修正先、種類、注記が不正です"));
                }
                Ok(CanonicalChange(Change {
                    part,
                    by,
                    kind,
                    note,
                }))
            }
        }

        deserializer.deserialize_map(ChangeVisitor)
    }
}

/// 修正先が、ルートからの相対パスか `https://` の URL か（ADR 0022、0025）。
fn valid_target(target: &str) -> bool {
    match Target::of(target) {
        Target::Url(url) => url.len() > "https://".len() && !url.contains(char::is_whitespace),
        Target::Path(path) => {
            !(path.starts_with('/')
                || path
                    .split('/')
                    .any(|segment| segment.is_empty() || segment == "." || segment == "..")
                || path.contains('\\'))
        }
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
        .trim_start_matches(['、', ',', '。'])
        .trim()
        .trim_end_matches('→')
        .trim_end()
        .trim_end_matches('は')
        .trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// 旧形式のステータス行の参照（ADR 0023、0026）。位置はリンクの文字だけを残した文字列の中のもの。
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyRef {
    /// 参照の始まり（`ADR` の位置、続く番号の位置、リンクの文字の始まり）
    pub start: usize,
    /// 参照の終わり
    pub end: usize,
    /// 4桁の番号
    pub number: String,
    /// `ADR` で始まる参照の最初の番号なら、`ADR` の位置（修飾子を探すのに使う）
    pub adr_at: Option<usize>,
    /// `ADR 0005、0006` の `0006` のように、前の参照に区切りで続く番号
    pub continued: bool,
    /// 参照を含むリンクのリンク先
    pub destination: Option<String>,
    /// リンクの参照（ADR 0026）で、リンクの文字の番号がリンク先の番号と合わないもの
    pub mismatched: bool,
}

/// リンク先のファイル名の先頭の4桁の数字（`0005-x.md` の `0005`）。
fn destination_number(destination: &str) -> Option<&str> {
    let path = destination.split('#').next()?;
    let name = path.rsplit('/').next()?;
    let number = name.get(..4)?;
    (number.bytes().all(|b| b.is_ascii_digit()) && name[4..].starts_with('-')).then_some(number)
}

/// 前後が数字でない4桁の数字の並び。
fn four_digit_runs(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(3))
        .filter(|&at| {
            bytes[at..at + 4].iter().all(u8::is_ascii_digit)
                && (at == 0 || !bytes[at - 1].is_ascii_digit())
                && !bytes.get(at + 4).is_some_and(u8::is_ascii_digit)
        })
        .map(|at| &text[at..at + 4])
        .collect()
}

/// リンクの文字だけを残した文字列と、その中の参照（行の中の順）。
pub fn legacy_refs(raw: &str) -> (String, Vec<LegacyRef>) {
    let (text, link_list) = links::status_text_and_links(raw);
    let destination_at = |at: usize| {
        link_list
            .iter()
            .find(|(range, _)| range.contains(&at))
            .map(|(_, destination)| destination.clone())
    };
    let link_start = |at: usize| {
        link_list
            .iter()
            .find(|(range, _)| range.contains(&at))
            .map(|(range, _)| range.start)
    };
    // 参照の始まり。リンクの中ならリンクの文字の始まり、外なら直前の修飾子の始まり（ADR 0028）
    let reference_start = |adr_at: usize| {
        link_start(adr_at).unwrap_or_else(|| {
            qualifier_len(&text[..adr_at]).map_or(adr_at, |len| adr_at - 1 - len)
        })
    };
    let adr_refs: Vec<LegacyRef> = links::status_reference_numbers(&text)
        .into_iter()
        .map(|(at, number, first)| LegacyRef {
            start: if first { reference_start(at - 4) } else { at },
            end: at + 4,
            number: number.to_owned(),
            adr_at: first.then(|| at - 4),
            continued: !first,
            destination: destination_at(at),
            mismatched: false,
        })
        .collect();
    let link_refs = link_list.iter().filter_map(|(range, destination)| {
        if adr_refs.iter().any(|r| range.contains(&r.start)) {
            return None;
        }
        let number = destination_number(destination)?;
        Some(LegacyRef {
            start: range.start,
            end: range.end,
            number: number.to_owned(),
            adr_at: None,
            continued: false,
            destination: Some(destination.clone()),
            mismatched: four_digit_runs(&text[range.clone()])
                .iter()
                .any(|digits| *digits != number),
        })
    });
    let mut refs: Vec<LegacyRef> = adr_refs.iter().cloned().chain(link_refs).collect();
    refs.sort_by_key(|r| r.start);
    (text, refs)
}

/// `ADR` の直前に半角の空白1つをはさんで置かれた修飾子の語の長さ（ADR 0019、0028）。
fn qualifier_len(before: &str) -> Option<usize> {
    let prefix = before.strip_suffix(' ')?;
    let word_len = prefix
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || "._/-".contains(*c))
        .count();
    let word = &prefix[prefix.len() - word_len..];
    let start = word.find(|c: char| c.is_ascii_alphanumeric())?;
    let word = &word[start..];
    (!word.is_empty()).then_some(word.len())
}

/// 移行スクリプトが部分名を決められない旧形式の行の理由（ADR 0028）。
#[allow(dead_code)]
pub fn legacy_part_problem(raw: &str) -> Option<&'static str> {
    let rest = superseded_ja(raw)
        .map(str::to_owned)
        .or_else(|| raw.strip_prefix("置換").map(replaced_body))?;
    let (text, refs) = legacy_refs(&rest);
    let pairs = refs.iter().zip(refs.iter().skip(1));
    let continues = pairs.clone().any(|(reference, next)| {
        let between = &text[reference.end..next.start];
        !next.continued
            && !is_separator(between)
            && !between
                .trim_start()
                .starts_with(['、', ',', '。', '）', ')'])
    });
    if continues {
        return Some("参照の後ろに部分名の続きがあります");
    }
    let unbalanced = refs
        .iter()
        .scan(0, |previous_end, reference| {
            let segment = &text[*previous_end..reference.start];
            *previous_end = reference.end;
            Some(segment)
        })
        .any(|segment| {
            let opened = segment.matches(['（', '(']).count();
            let closed = segment.matches(['）', ')']).count();
            opened != closed
        });
    unbalanced.then_some("括弧が部分名をまたぎます")
}

/// 二つの参照のあいだが区切りだけか（ADR 0026）。
fn is_separator(between: &str) -> bool {
    matches!(between.trim(), "" | "、" | "," | "と" | "および")
}

/// 最後の参照の後ろの文字列の扱い（ADR 0026）。
enum Tail {
    /// 空か、閉じ括弧だけ
    Empty,
    /// `は` で終わる。修正先の抜け
    MissingTarget(String),
    /// 注記
    Note(String),
}

fn tail(text: &str) -> Tail {
    let trimmed = text.trim_matches(|c: char| c.is_whitespace() || c == '、' || c == ',');
    if trimmed.chars().all(|c| c == '）' || c == ')') {
        return Tail::Empty;
    }
    let inner = trimmed
        .strip_prefix(['（', '('])
        .and_then(|rest| rest.strip_suffix(['）', ')']))
        .unwrap_or(trimmed)
        .trim();
    if inner.ends_with('は') {
        Tail::MissingTarget(inner.to_owned())
    } else {
        Tail::Note(inner.to_owned())
    }
}

/// `採択→` より後ろから、変更と、最後の参照の後ろの注記を取り出す（ADR 0026）。
fn ja_changes(rest: &str) -> (Vec<Change>, Option<String>) {
    let (text, refs) = legacy_refs(rest);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return (vec![], None);
    }
    if refs.is_empty() {
        let change = match bare_numbers(trimmed) {
            Some(by) => Change::legacy(None, by),
            None => Change::legacy(
                Some(
                    trimmed
                        .trim_matches(|c: char| c.is_whitespace() || c == '、')
                        .to_owned(),
                ),
                vec![],
            ),
        };
        return (vec![change], None);
    }
    let (changes, last_end) = refs.iter().fold(
        (Vec::<Change>::new(), 0),
        |(mut changes, previous_end), reference| {
            let between = &text[previous_end..reference.start];
            let join = reference.continued || is_separator(between);
            match changes.last_mut() {
                Some(last) if join => last.by.push(reference.number.clone()),
                _ => changes.push(Change::legacy(
                    part(between),
                    vec![reference.number.clone()],
                )),
            }
            (changes, reference.end)
        },
    );
    match tail(&text[last_end..]) {
        Tail::Empty => (changes, None),
        Tail::Note(note) => (changes, Some(note)),
        Tail::MissingTarget(missing) => {
            let trailing = Change::legacy(part(&missing).or(Some(missing)), vec![]);
            (changes.into_iter().chain([trailing]).collect(), None)
        }
    }
}

/// `置換` より後ろを、`採択→` より後ろと同じ形にする（ADR 0027）。
/// 前後の空白と先頭の `→` を除き、先頭の開き括弧と、それに対応する閉じ括弧を除く。
fn replaced_body(rest: &str) -> String {
    let rest = rest.trim();
    let rest = rest.strip_prefix('→').map_or(rest, str::trim_start);
    let Some(inner) = rest.strip_prefix(['（', '(']) else {
        return rest.to_owned();
    };
    let close = inner
        .char_indices()
        .scan(1i32, |depth, (at, c)| {
            *depth += match c {
                '（' | '(' => 1,
                '）' | ')' => -1,
                _ => 0,
            };
            Some((at, c, *depth))
        })
        .find(|&(_, _, depth)| depth == 0);
    match close {
        Some((at, c, _)) => format!("{}{}", &inner[..at], &inner[at + c.len_utf8()..]),
        None => rest.to_owned(),
    }
}

/// `採択→` より後ろから変更を取り出す。
fn changes_ja(rest: &str) -> Vec<Change> {
    ja_changes(rest).0
}

/// 旧形式の `採択→…` の行の、最後の参照の後ろの注記（ADR 0026。移行スクリプトが使う）。
#[allow(dead_code)]
pub fn legacy_note(raw: &str) -> Option<String> {
    match superseded_ja(raw) {
        Some(rest) => ja_changes(rest).1,
        // `置換…`（ADR 0027）
        None => raw
            .strip_prefix("置換")
            .and_then(|rest| ja_changes(&replaced_body(rest)).1),
    }
}

/// `Superseded` より後ろから変更を取り出す。
fn changes_en(rest: &str) -> Vec<Change> {
    let text = links::link_texts(rest);
    let text = text.trim_start();
    let text = english(text, "by").unwrap_or(text);
    vec![Change::legacy(
        None,
        sequences(text)
            .into_iter()
            .flat_map(|sequence| sequence.numbers)
            .collect(),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ADR 0028

    fn parts(raw: &str) -> Vec<Option<String>> {
        match parse(raw) {
            Status::Superseded(changes) => changes.into_iter().map(|c| c.part).collect(),
            other => panic!("{raw}: {other:?}"),
        }
    }

    fn some(part: &str) -> Option<String> {
        Some(part.to_owned())
    }

    #[test]
    fn part_inv1_qualifier() {
        for raw in [
            "採択→a は golem-x ADR 0002",
            "採択→a は [golem-x ADR 0002](../golem-x/adr/0002-b.md)",
            "採択→a は [x.example ADR 0002](https://x.example/docs/adr/0002-b.md)",
            "採択→a は docs/adr ADR 0002",
        ] {
            assert_eq!(parts(raw), vec![some("a")], "{raw}");
        }
        assert_eq!(
            parts("採択→a は golem-x ADR 0002、b は [golem-y ADR 0003](../golem-y/adr/0003-c.md)"),
            vec![some("a"), some("b")]
        );
    }

    #[test]
    fn part_inv2_delimiters() {
        assert_eq!(
            parts("採択→a は [ADR 0002](0002-b.md)。b は [ADR 0003](0003-c.md)"),
            vec![some("a"), some("b")]
        );
        assert_eq!(parts("採択→a は→[ADR 0002](0002-b.md)"), vec![some("a")]);
        assert_eq!(
            parts("採択→a 部分は → [ADR 0002](0002-b.md)"),
            vec![some("a 部分")]
        );
    }

    #[test]
    fn part_inv4_problems() {
        assert_eq!(
            legacy_part_problem("採択→a、[ADR 0002](0002-b.md) の述語、b は [ADR 0003](0003-c.md)"),
            Some("参照の後ろに部分名の続きがあります")
        );
        assert_eq!(
            legacy_part_problem(
                "採択→a は x に移した（[ADR 0002](0002-b.md)）、b は [ADR 0003](0003-c.md)"
            ),
            Some("括弧が部分名をまたぎます")
        );
        assert_eq!(
            legacy_part_problem(
                "採択→a は [ADR 0002](0002-b.md) で消した、b は [ADR 0003](0003-c.md)"
            ),
            Some("参照の後ろに部分名の続きがあります")
        );
        for raw in [
            "採択→（a の読み方）は [ADR 0002](0002-b.md)",
            "採択→a は [ADR 0002](0002-b.md) で消した",
            "採択→a は [ADR 0002](0002-b.md)、b は [ADR 0003](0003-c.md)",
            "採択→a は [ADR 0002](0002-b.md) と [ADR 0003](0003-c.md)",
            "採択→a は ADR 0002、0003",
            "採択→a は [ADR 0002](0002-b.md)（新しい規則）",
            "置換（[ADR 0002](0002-b.md)）",
            "採択",
        ] {
            assert_eq!(legacy_part_problem(raw), None, "{raw}");
        }
    }

    #[test]
    fn part_inv5_unchanged() {
        let cases = [
            ("採択→a は [ADR 0002](0002-b.md)", vec![some("a")]),
            (
                "採択→a は [ADR 0002](0002-b.md)、b は [ADR 0003](0003-c.md)",
                vec![some("a"), some("b")],
            ),
            ("採択→A、BはADR 0005、0006", vec![some("A、B")]),
            ("採択→0004、0009", vec![None]),
            ("置換（[ADR 0002](0002-b.md)）", vec![None]),
        ];
        for (raw, expected) in cases {
            assert_eq!(parts(raw), expected, "{raw}");
        }
    }

    // ADR 0027

    #[test]
    fn replaced_inv1_status() {
        for raw in [
            "置換（[ADR 0002](0002-b.md)）",
            "置換([ADR 0002](0002-b.md))",
            "置換（[ADR 0002](0002-b.md)、[ADR 0003](0003-c.md)）",
            "置換→[ADR 0002](0002-b.md)",
            "置換",
            "置換（[ADR 0002](0002-b.md)）で削った",
        ] {
            assert!(matches!(parse(raw), Status::Superseded(_)), "{raw}");
        }
        assert_eq!(
            parse("採択（X は置換（[ADR 0002](0002-b.md)））"),
            Status::Accepted
        );
    }

    #[test]
    fn replaced_inv2_changes() {
        let pairs = [
            (
                "置換（[ADR 0002](0002-b.md)）",
                "採択→[ADR 0002](0002-b.md)",
            ),
            ("置換([ADR 0002](0002-b.md))", "採択→[ADR 0002](0002-b.md)"),
            (
                "置換（[ADR 0002](0002-b.md)、[ADR 0003](0003-c.md)）",
                "採択→[ADR 0002](0002-b.md)、[ADR 0003](0003-c.md)",
            ),
            ("置換→[ADR 0002](0002-b.md)", "採択→[ADR 0002](0002-b.md)"),
        ];
        for (replaced, accepted) in pairs {
            assert_eq!(parse(replaced), parse(accepted), "{replaced}");
        }
        assert_eq!(parse("置換"), Status::Superseded(vec![]));
        assert_eq!(
            legacy_note("置換（[ADR 0002](0002-b.md)）で削った"),
            Some("で削った".to_owned())
        );
    }

    #[test]
    fn replaced_inv3_others() {
        let cases = [
            ("提案", Status::Proposed),
            ("採択", Status::Accepted),
            ("採択（未実装）", Status::AcceptedNotImplemented),
            ("却下", Status::Rejected),
            ("破棄", Status::Discarded),
            ("廃止", Status::Deprecated),
            ("補足（[ADR 0002](0002-b.md)）", Status::Unknown),
            ("Replaced by ADR 0002", Status::Unknown),
        ];
        for (raw, expected) in cases {
            assert_eq!(parse(raw), expected, "{raw}");
        }
    }

    // ADR 0025

    fn canonical(line: &str) -> Status {
        parse_canonical_line(line).unwrap_or_else(|error| panic!("{line}: {error}"))
    }

    #[test]
    fn amend_inv1_key_order() {
        let pairs = [
            (
                r#"Status: superseded-pending [{"part":null,"by":["docs/adr/0002-b.md"]}]"#,
                r#"Status: superseded-pending [ { "by" : [ "docs/adr/0002-b.md" ] , "part" : null } ]"#,
            ),
            (
                r#"Status: superseded [{"part":"a","by":[],"kind":"note","note":"引用符\"と\\と日本語"}]"#,
                r#"Status: superseded [{"note":"引用符\"と\\と日本語","kind":"note","by":[],"part":"a"}]"#,
            ),
            (
                r#"Status: superseded [{"part":"a","by":["https://example.com/adr/0001.md"],"kind":"supplement"}]"#,
                r#"Status: superseded [{"kind":"supplement", "by":["https://example.com/adr/0001.md"], "part":"a"}]"#,
            ),
        ];
        for (left, right) in pairs {
            assert_eq!(canonical(left), canonical(right), "{left}");
        }
    }

    #[test]
    fn amend_inv2_default_kind() {
        let omitted = canonical(r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"]}]"#);
        let explicit = canonical(
            r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"kind":"replace"}]"#,
        );
        assert_eq!(omitted, explicit);
        assert_eq!(
            classify(AdrState::Present(omitted), AdrState::Present(explicit)),
            Classification::Unchanged
        );
    }

    #[test]
    fn amend_inv3_parse_errors() {
        for line in [
            r#"Status: superseded-pending []"#,
            r#"Status: superseded [{"part":"a","by":[],"kind":"note"}]"#,
            r#"Status: superseded [{"part":"a","by":[]}]"#,
            r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"kind":"note","note":"n"}]"#,
            r#"Status: superseded [{"part":"a","by":["http://example.com/adr/0001.md"]}]"#,
            r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"kind":"other"}]"#,
            r#"Status: superseded [{"part":"a","by":["docs/adr/0002-b.md"],"note":""}]"#,
            r#"Status: superseded [{"part":"a","by":["https://"]}]"#,
        ] {
            assert!(parse_canonical_line(line).is_err(), "{line}");
            assert_eq!(
                parse(line.strip_prefix("Status: ").unwrap()),
                Status::Unknown,
                "{line}"
            );
        }
    }

    #[test]
    fn amend_inv5_transitions() {
        let x = vec![Change::legacy(None, vec!["docs/adr/0002-b.md".to_owned()])];
        let y = vec![Change {
            kind: Kind::Supplement,
            ..x[0].clone()
        }];
        let present = AdrState::Present;
        let transitions = [
            Transition::RevisePending { changes: x.clone() },
            Transition::UpdateRevisions {
                old: x.clone(),
                new: y.clone(),
                pending: true,
            },
            Transition::CompleteRevised { changes: x.clone() },
            Transition::Deprecate {
                origin: DeprecateOrigin::SupersededPending(x.clone()),
            },
        ];
        for transition in transitions {
            let (before, after) = transition.clone().endpoints();
            assert_eq!(classify(before, after), Classification::Direct(transition));
        }
        let unclassified = [
            (
                Status::SupersededPending(x.clone()),
                Status::Superseded(y.clone()),
            ),
            (Status::SupersededPending(x.clone()), Status::Accepted),
            (Status::Accepted, Status::SupersededPending(x.clone())),
            (Status::Proposed, Status::SupersededPending(x.clone())),
        ];
        for (before, after) in unclassified {
            assert_eq!(
                classify(present(before.clone()), present(after.clone())),
                Classification::Unclassified,
                "{before:?} -> {after:?}"
            );
        }
    }

    #[test]
    fn inv1_direct_endpoints() {
        let cases = [
            (AdrState::Absent, AdrState::Present(Status::Proposed)),
            (
                AdrState::Present(Status::Proposed),
                AdrState::Present(Status::Accepted),
            ),
            (
                AdrState::Present(Status::Proposed),
                AdrState::Present(Status::AcceptedNotImplemented),
            ),
            (
                AdrState::Present(Status::Proposed),
                AdrState::Present(Status::Rejected),
            ),
            (
                AdrState::Present(Status::Proposed),
                AdrState::Present(Status::Discarded),
            ),
            (
                AdrState::Present(Status::AcceptedNotImplemented),
                AdrState::Present(Status::Accepted),
            ),
            (
                AdrState::Present(Status::Accepted),
                AdrState::Present(Status::Superseded(vec![change(
                    None,
                    &["docs/adr/0002-x.md"],
                )])),
            ),
            (
                AdrState::Present(Status::AcceptedNotImplemented),
                AdrState::Present(Status::Superseded(vec![change(
                    None,
                    &["docs/adr/0002-x.md"],
                )])),
            ),
            (
                AdrState::Present(Status::Superseded(vec![change(
                    None,
                    &["docs/adr/0002-x.md"],
                )])),
                AdrState::Present(Status::Superseded(vec![change(
                    None,
                    &["docs/adr/0003-y.md"],
                )])),
            ),
            (
                AdrState::Present(Status::Accepted),
                AdrState::Present(Status::Deprecated),
            ),
            (
                AdrState::Present(Status::AcceptedNotImplemented),
                AdrState::Present(Status::Deprecated),
            ),
            (
                AdrState::Present(Status::Superseded(vec![change(
                    None,
                    &["docs/adr/0002-x.md"],
                )])),
                AdrState::Present(Status::Deprecated),
            ),
        ];
        for (before, after) in cases {
            match classify(before.clone(), after.clone()) {
                Classification::Direct(transition) => {
                    assert_eq!(transition.endpoints(), (before, after))
                }
                other => panic!("expected Direct: {other:?}"),
            }
        }
    }

    #[test]
    fn inv2_equivalent_spellings() {
        assert_eq!(parse("提案"), parse("proposed"));
        assert_eq!(parse("採択（未実装）"), parse("accepted-pending"));
        assert_eq!(
            classify(AdrState::Absent, AdrState::Absent),
            Classification::Unchanged
        );
        assert_eq!(
            classify(
                AdrState::Present(parse("提案")),
                AdrState::Present(parse("proposed"))
            ),
            Classification::Unchanged
        );
    }

    #[test]
    fn inv3_unclassified_pairs() {
        assert_eq!(
            classify(
                AdrState::Present(Status::Unknown),
                AdrState::Present(Status::Unknown)
            ),
            Classification::Unclassified
        );
        assert_eq!(
            classify(AdrState::Absent, AdrState::Present(Status::Accepted)),
            Classification::Unclassified
        );
        assert_eq!(
            classify(
                AdrState::Present(Status::Accepted),
                AdrState::Present(Status::Proposed)
            ),
            Classification::Unclassified
        );
    }

    #[test]
    fn canonical_status_lines() {
        assert_eq!(
            parse_canonical_line("Status: accepted-pending"),
            Ok(Status::AcceptedNotImplemented)
        );
        assert_eq!(
            parse_canonical_line(
                "Status: superseded [{\"part\":\"状態\",\"by\":[\"docs/adr/0002-x.md\"]}]"
            ),
            Ok(Status::Superseded(vec![change(
                Some("状態"),
                &["docs/adr/0002-x.md"]
            )]))
        );
        for invalid in [
            "ステータス: 提案",
            "Status: 提案",
            "Status: Accepted",
            "Status: absent",
            "Status: superseded []",
            "Status: superseded  [{\"part\":null,\"by\":[\"docs/adr/0002-x.md\"]}]",
            "Status: superseded [{\"part\":null,\"by\":[]}]",
            "Status: superseded [{\"part\":\"\",\"by\":[\"docs/adr/0002-x.md\"]}]",
            "Status: superseded [{\"part\":null,\"by\":[\"../0002-x.md\"]}]",
            "Status: superseded [{\"part\":null,\"by\":[\"docs/adr/0002-x.md\"],\"extra\":1}]",
            "Status: superseded [{\"part\":null,\"part\":null,\"by\":[\"docs/adr/0002-x.md\"]}]",
        ] {
            assert!(parse_canonical_line(invalid).is_err(), "{invalid:?}");
        }
        let malformed = "Status: superseded [{\"part\":null,\"by\":[]}]\n";
        assert_eq!(
            parse("superseded [{\"part\":null,\"by\":[]}]"),
            Status::Unknown
        );
        assert_eq!(count_violations("docs/adr/0001-x.md", malformed).len(), 1);
        assert!(
            count_violations("docs/adr/0001-x.md", "Status: Superseded by ADR 0002\n").is_empty()
        );
        assert_eq!(
            parse("Superseded by ADR 0002"),
            Status::Superseded(vec![change(None, &["0002"])])
        );
    }

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
    fn parses_accepted_not_implemented() {
        // ADR 0016 の表
        for raw in [
            "採択（未実装）",
            "採択(未実装)",
            "採択（未実装)",
            "採択(未実装）",
            "採択（未実装）。実装は別のリポジトリ",
            "Accepted (not implemented)",
            "accepted (Not Implemented)",
            "Accepted（not implemented）",
        ] {
            assert_eq!(parse(raw), Status::AcceptedNotImplemented, "{raw:?}");
        }
        for raw in [
            "採択",
            "採択（2026-10-01）",
            "採択 （未実装）",
            "採択（ 未実装 ）",
            "採択【未実装】",
        ] {
            assert_eq!(parse(raw), Status::Accepted, "{raw:?}");
        }
        assert!(matches!(
            parse("採択→[ADR 0002](0002-x.md)"),
            Status::Superseded(_)
        ));
        assert_eq!(parse("未実装"), Status::Unknown);
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
        Change::legacy(
            part.map(str::to_owned),
            by.iter().map(|n| (*n).to_owned()).collect(),
        )
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
