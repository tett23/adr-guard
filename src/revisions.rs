//! 旧形式のステータスに記録された修正先をADRのパスへ解決する。

use crate::links;
use crate::status::{self, Change, Status};

#[derive(Clone, Debug)]
struct Reference {
    number: String,
    qualifier: Option<String>,
    destination: Option<String>,
}

fn normalize_relative(doc: &str, destination: &str) -> Result<String, String> {
    if destination.starts_with('/') || destination.contains("//") || destination.contains('#') {
        return Err(format!(
            "リンク先 {destination} はADRの相対パスではありません"
        ));
    }
    let dir = doc.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut components: Vec<&str> = dir.split('/').filter(|part| !part.is_empty()).collect();
    for part in destination.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(format!("リンク先 {destination} がリポジトリ外を指します"));
                }
            }
            item => components.push(item),
        }
    }
    Ok(components.join("/"))
}

fn number_of(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?;
    let number = name.get(..4)?;
    (number.bytes().all(|b| b.is_ascii_digit()) && name[4..].starts_with('-')).then_some(number)
}

fn in_dir<'a>(adrs: &'a [String], dir: &str, number: &str) -> Vec<&'a String> {
    let prefix = format!("{dir}/{number}-");
    adrs.iter()
        .filter(|path| path.starts_with(&prefix) && !path[prefix.len()..].contains('/'))
        .collect()
}

fn qualifier_dir(doc: &str, qualifier: &str, adrs: &[String]) -> Option<String> {
    let dirs: std::collections::HashSet<&str> = adrs
        .iter()
        .filter_map(|path| path.rsplit_once('/').map(|(dir, _)| dir))
        .collect();
    if qualifier.contains('/') {
        let path = qualifier.trim_start_matches("./").trim_end_matches('/');
        return std::iter::once(path.to_owned())
            .chain(links::ADR_DIR_NAMES.map(|name| format!("{path}/{name}")))
            .find(|dir| dirs.contains(dir.as_str()));
    }
    let mut prefix = doc.rsplit_once('/').map_or("", |(dir, _)| dir);
    loop {
        let ancestor = prefix.strip_suffix("/docs").unwrap_or(prefix);
        let root = if ancestor.is_empty() {
            String::new()
        } else {
            format!("{ancestor}/")
        };
        for name in links::ADR_DIR_NAMES {
            let dir = format!("{root}docs/{qualifier}/{name}");
            if dirs.contains(dir.as_str()) {
                return Some(dir);
            }
        }
        match prefix.rsplit_once('/') {
            Some((parent, _)) => prefix = parent,
            None => break,
        }
    }
    links::ADR_DIR_NAMES
        .into_iter()
        .map(|name| format!("docs/{qualifier}/{name}"))
        .find(|dir| dirs.contains(dir.as_str()))
}

fn resolve_reference(doc: &str, reference: &Reference, adrs: &[String]) -> Result<String, String> {
    let number = &reference.number;
    let linked = reference
        .destination
        .as_deref()
        .map(|dest| normalize_relative(doc, dest))
        .transpose()?;
    if let Some(linked) = &linked {
        if !adrs.contains(linked) || number_of(linked) != Some(number) {
            return Err(format!(
                "ADR {number} のリンク先 {linked} がありません、または番号が違います"
            ));
        }
        let has_qualifier = reference.qualifier.as_deref().is_some_and(|qualifier| {
            qualifier.contains(['/', '-']) || qualifier_dir(doc, qualifier, adrs).is_some()
        });
        if !has_qualifier {
            return Ok(linked.clone());
        }
    }
    let qualified = reference
        .qualifier
        .as_deref()
        .and_then(|qualifier| qualifier_dir(doc, qualifier, adrs).map(|dir| (qualifier, dir)));
    let expected = if let Some((_, dir)) = qualified {
        match in_dir(adrs, &dir, number).as_slice() {
            [path] => Some((*path).clone()),
            [] => return Err(format!("{dir} に ADR {number} がありません")),
            _ => return Err(format!("{dir} に ADR {number} が2つ以上あります")),
        }
    } else {
        if let Some(qualifier) = &reference.qualifier
            && qualifier.contains(['/', '-'])
        {
            return Err(format!("{qualifier} のADRのディレクトリがありません"));
        }
        let mut found = None;
        for dir in links::candidate_dirs(doc) {
            match in_dir(adrs, &dir, number).as_slice() {
                [path] => {
                    found = Some((*path).clone());
                    break;
                }
                [] => {}
                _ => return Err(format!("{dir} に ADR {number} が2つ以上あります")),
            }
        }
        found
    };
    match (expected, linked) {
        (Some(expected), Some(linked)) if expected != linked => Err(format!(
            "ADR {number} のリンク先 {linked} は {expected} ではありません"
        )),
        (Some(expected), _) => Ok(expected),
        _ => Err(format!("ADR {number} の修正先がありません")),
    }
}

fn qualifier_before(text: &str, at: usize) -> Option<String> {
    let prefix = &text[..at];
    let prefix = prefix.strip_suffix(' ')?;
    let word = prefix
        .rsplit(|c: char| !c.is_ascii_alphanumeric() && !"./_-".contains(c))
        .next()?;
    (word.starts_with(|c: char| c.is_ascii_alphanumeric())
        && (word.contains(['/', '-']) || !word.is_empty()))
    .then(|| word.to_owned())
}

fn references_in(text: &str, destination: Option<&str>) -> Vec<Reference> {
    let mut refs = Vec::new();
    if let Some(destination) = destination {
        let number = text.trim();
        if number.len() == 4 && number.bytes().all(|b| b.is_ascii_digit()) {
            return vec![Reference {
                number: number.to_owned(),
                qualifier: None,
                destination: Some(destination.to_owned()),
            }];
        }
    }
    for (at, _) in text.match_indices("ADR ").chain(text.match_indices("ADR-")) {
        let Some(number) = text.get(at + 4..at + 8) else {
            continue;
        };
        if !number.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        refs.push(Reference {
            number: number.to_owned(),
            qualifier: qualifier_before(text, at),
            destination: destination.map(str::to_owned),
        });
        let mut end = at + 8;
        while let Some(rest) = text.get(end..) {
            let Some(after) = rest.strip_prefix([',', '、']) else {
                break;
            };
            let skipped = after.trim_start_matches(' ');
            let Some(next) = skipped.get(..4) else {
                break;
            };
            if !next.bytes().all(|b| b.is_ascii_digit()) {
                break;
            }
            refs.push(Reference {
                number: next.to_owned(),
                qualifier: None,
                destination: destination.map(str::to_owned),
            });
            end = text.len() - skipped.len() + 4;
        }
    }
    refs
}

fn references(raw: &str) -> Vec<Reference> {
    let mut refs = Vec::new();
    let mut remainder = raw;
    while let Some(start) = remainder.find('[') {
        refs.extend(references_in(&remainder[..start], None));
        let link = &remainder[start + 1..];
        let Some(close) = link.find("](") else {
            remainder = &remainder[start + 1..];
            continue;
        };
        let target = &link[close + 2..];
        let Some(end) = target.find(')') else {
            remainder = &remainder[start + 1..];
            continue;
        };
        refs.extend(references_in(&link[..close], Some(&target[..end])));
        remainder = &target[end + 1..];
    }
    refs.extend(references_in(remainder, None));
    if refs.is_empty() {
        let bare = raw.split_once('→').map(|(_, rest)| rest).unwrap_or(raw);
        let numbers: Option<Vec<_>> = bare
            .split(['、', ','])
            .map(|part| {
                let number = part.trim();
                (number.len() == 4 && number.bytes().all(|b| b.is_ascii_digit())).then(|| {
                    Reference {
                        number: number.to_owned(),
                        qualifier: None,
                        destination: None,
                    }
                })
            })
            .collect();
        if let Some(numbers) = numbers {
            return numbers;
        }
    }
    refs
}

/// 旧形式の変更部分と修正先を維持し、番号をルート相対パスに置き換える。
pub fn resolve_legacy(doc: &str, raw: &str, adrs: &[String]) -> Result<Vec<Change>, String> {
    let Status::Superseded(changes) = status::parse(raw) else {
        return Err("修正済みのステータスではありません".to_owned());
    };
    if changes.is_empty() || changes.iter().any(|change| change.by.is_empty()) {
        return Err("修正先の番号がありません".to_owned());
    }
    let refs = references(raw);
    let expected = changes.iter().map(|change| change.by.len()).sum::<usize>();
    if refs.len() != expected {
        return Err("修正先の参照を対応づけられません".to_owned());
    }
    let mut refs = refs.into_iter();
    changes
        .into_iter()
        .map(|change| {
            let by = change
                .by
                .iter()
                .map(|number| {
                    let reference = refs.next().expect("参照の数は確認済み");
                    if reference.number != *number {
                        return Err(format!("ADR {number} の参照を対応づけられません"));
                    }
                    resolve_reference(doc, &reference, adrs)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Change {
                part: change.part,
                by,
            })
        })
        .collect()
}
