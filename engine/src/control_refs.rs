//! References an agent can type: a track, clip or marker by its id or by its name, and the
//! fuzzy matching behind plugin and parameter search. Every command runs its parameters
//! through [`resolve`] before it validates them, so `trackId: "Bass"` works wherever a track
//! id does, and a wrong name answers with the names that exist and the closest one.

use crate::{control::Spec, model::*, Result};
use serde_json::Value;

/// Lowercase letters and digits only: "Pro-Q 3" and "proq3" compare equal.
pub fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// How well `query` names `candidate`, higher is better; `None` when it does not. Every word
/// of the query must appear in the candidate (ignoring case, spaces and punctuation); an
/// exact name beats a prefix, which beats a word inside the name.
pub fn score(query: &str, candidate: &str) -> Option<u32> {
    let whole = normalize(candidate);
    let wanted = normalize(query);
    if wanted.is_empty() {
        return Some(1);
    }
    if whole == wanted {
        return Some(1000);
    }
    let words: Vec<String> = query
        .split_whitespace()
        .map(normalize)
        .filter(|w| !w.is_empty())
        .collect();
    if !words.iter().all(|w| whole.contains(w.as_str())) && !whole.contains(&wanted) {
        return None;
    }
    let mut points = 100;
    if whole.starts_with(&wanted) {
        points += 400;
    } else if whole.contains(&wanted) {
        points += 200;
    }
    // Shorter candidates are the more specific match for the same words.
    points += 100u32.saturating_sub(whole.len().min(100) as u32);
    Some(points)
}

/// Edit distance between two short strings, for "did you mean".
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(prev + usize::from(ca != *cb));
            prev = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// The candidate closest to `query`: a fuzzy match first, else the smallest edit distance
/// within a third of the length.
pub fn suggest<'a>(query: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let wanted = normalize(query);
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        if score(query, c).is_some() {
            return Some(c);
        }
        let d = distance(&wanted, &normalize(c));
        if wanted.len() >= 3 && d <= (wanted.len() / 3).max(2) && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c)
}

/// A bounded, readable list of names for an error message.
fn listing(names: &[String]) -> String {
    const MAX: usize = 24;
    let mut out = names
        .iter()
        .take(MAX)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > MAX {
        out.push_str(&format!(" and {} more", names.len() - MAX));
    }
    if out.is_empty() {
        "none".into()
    } else {
        out
    }
}

/// The track (or bus) a reference means: its id, its name (ignoring case), or for the buses
/// "master", "bus-a", "bus-b" and their names.
pub fn track_id(s: &Session, reference: &str) -> Result<String> {
    if s.tracks.iter().any(|t| t.id == reference) || is_bus(reference) {
        return Ok(reference.to_string());
    }
    let wanted = reference.trim();
    let named: Vec<&Track> = s
        .tracks
        .iter()
        .filter(|t| t.name.trim().eq_ignore_ascii_case(wanted))
        .collect();
    match named.as_slice() {
        [one] => return Ok(one.id.clone()),
        [] => {}
        many => {
            return Err(format!(
                "Track name `{wanted}` is ambiguous: {}. Pass the id.",
                many.iter()
                    .map(|t| format!("{} (index {})", t.id, index_of(s, &t.id)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
    for bus in [MASTER, BUS_A, BUS_B] {
        if bus.eq_ignore_ascii_case(wanted) || bus_name(bus).eq_ignore_ascii_case(wanted) {
            return Ok(bus.to_string());
        }
    }
    let names: Vec<String> = s
        .tracks
        .iter()
        .map(|t| format!("{} ({})", t.name, t.id))
        .collect();
    let hint = suggest(wanted, s.tracks.iter().map(|t| t.name.as_str()))
        .map(|n| format!(" Did you mean {n}?"))
        .unwrap_or_default();
    Err(format!(
        "Unknown track `{wanted}`. Tracks: {}; buses: master, bus-a, bus-b.{hint}",
        listing(&names)
    ))
}

fn index_of(s: &Session, id: &str) -> usize {
    s.tracks.iter().position(|t| t.id == id).unwrap_or(0)
}

/// The clip a reference means: its id, or a name that only one clip has.
pub fn clip_id(s: &Session, reference: &str) -> Result<String> {
    if s.clips.iter().any(|c| c.id == reference) {
        return Ok(reference.to_string());
    }
    let wanted = reference.trim();
    let named: Vec<&Clip> = s
        .clips
        .iter()
        .filter(|c| c.name.trim().eq_ignore_ascii_case(wanted))
        .collect();
    let describe = |c: &Clip| {
        let track = s
            .tracks
            .iter()
            .find(|t| t.id == c.track_id)
            .map_or("?", |t| t.name.as_str());
        format!(
            "{} on {track} at bar {} ({})",
            c.name,
            c.start_bar + 1.0,
            c.id
        )
    };
    match named.as_slice() {
        [one] => Ok(one.id.clone()),
        [] => {
            let hint = suggest(wanted, s.clips.iter().map(|c| c.name.as_str()))
                .map(|n| format!(" Did you mean {n}?"))
                .unwrap_or_default();
            let names: Vec<String> = s.clips.iter().map(describe).collect();
            Err(format!(
                "Unknown clip `{wanted}`. Clips: {}.{hint} Use clip.list or session.overview.",
                listing(&names)
            ))
        }
        many => Err(format!(
            "Clip name `{wanted}` is ambiguous: {}. Pass the id.",
            many.iter()
                .map(|c| describe(c))
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}

/// The marker a reference means: its id, or its name.
pub fn marker_id(s: &Session, reference: &str) -> Result<String> {
    if s.markers.iter().any(|m| m.id == reference) {
        return Ok(reference.to_string());
    }
    let wanted = reference.trim();
    let named: Vec<&Marker> = s
        .markers
        .iter()
        .filter(|m| m.name.trim().eq_ignore_ascii_case(wanted))
        .collect();
    match named.as_slice() {
        [one] => Ok(one.id.clone()),
        [] => {
            let names: Vec<String> = s
                .markers
                .iter()
                .map(|m| format!("{} ({})", m.name, m.id))
                .collect();
            let hint = suggest(wanted, s.markers.iter().map(|m| m.name.as_str()))
                .map(|n| format!(" Did you mean {n}?"))
                .unwrap_or_default();
            Err(format!(
                "Unknown marker `{wanted}`. Markers: {}.{hint}",
                listing(&names)
            ))
        }
        many => Err(format!(
            "Marker name `{wanted}` is ambiguous: {}. Pass the id.",
            many.iter()
                .map(|m| format!("{} at bar {}", m.id, m.bar + 1.0))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Replace names with ids in the reference parameters a command declares (`trackId`,
/// `clipId`, `markerId`). Returns `None` when nothing needed resolving, so the common case
/// costs no copy.
pub fn resolve(s: &Session, spec: &Spec, params: &Value) -> Result<Option<Value>> {
    let Value::Object(map) = params else {
        return Ok(None);
    };
    let mut out: Option<serde_json::Map<String, Value>> = None;
    for key in ["trackId", "clipId", "markerId"] {
        if !spec.params.iter().any(|p| p.name == key) {
            continue;
        }
        let Some(reference) = map.get(key).and_then(Value::as_str) else {
            continue;
        };
        let id = match key {
            "trackId" => track_id(s, reference)?,
            "clipId" => clip_id(s, reference)?,
            _ => marker_id(s, reference)?,
        };
        if id != reference {
            out.get_or_insert_with(|| map.clone())
                .insert(key.into(), Value::String(id));
        }
    }
    Ok(out.map(Value::Object))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_scores_prefer_exact_then_prefix_then_words() {
        assert_eq!(score("Pro-Q 3", "pro q3"), Some(1000));
        assert!(score("proq", "Pro-Q 3").unwrap() > score("q 3", "Pro-Q 3").unwrap());
        assert!(score("fab q", "FabFilter Pro-Q 3").is_some());
        assert!(score("reverb", "Pro-Q 3").is_none());
        assert_eq!(suggest("Bas", ["Drums", "Bass"]), Some("Bass"));
        assert_eq!(suggest("Drmus", ["Drums", "Bass"]), Some("Drums"));
        assert_eq!(suggest("Violin", ["Drums", "Bass"]), None);
    }
}
