//! The selection as the agent hears it. The composer shows it as chips ("About Chords ·
//! bars 3–9, Keys, Cycle 5–9") and, unless the person turns it off, appends it to the
//! message after [`CONTEXT_MARK`]; the chat splits it back off to show the words alone.

use ryolune_engine::model::{ClipData, Session};

/// Marks where the person's words end and the window's description of the selection begins.
pub const CONTEXT_MARK: &str = "\n\n[Selected in the window: ";

#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    /// What the person reads in the chip.
    pub label: String,
    /// What the agent reads: names for the person, ids for the tools.
    pub detail: String,
}

/// A zero-based bar as people count it, without a trailing `.0`.
fn bar(n: f64) -> String {
    let v = ((n + 1.0) * 100.0).round() / 100.0;
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// What is selected right now: the region, its track, the cycle range. Empty when nothing is.
pub fn selection_context(s: &Session) -> Vec<Chip> {
    let mut chips = vec![];
    let view = &s.view;
    let track = view
        .selected_track_id
        .as_ref()
        .and_then(|id| s.tracks.iter().find(|t| &t.id == id));
    let clip = view
        .selected_clip_id
        .as_ref()
        .and_then(|id| s.clips.iter().find(|c| &c.id == id));
    if let Some(clip) = clip {
        let (from, to) = (bar(clip.start_bar), bar(clip.start_bar + clip.length_bars));
        let kind = if matches!(clip.data, ClipData::Midi { .. }) {
            "MIDI"
        } else {
            "audio"
        };
        chips.push(Chip {
            label: format!("{} · bars {from}–{to}", clip.name),
            detail: format!(
                "region \"{}\" (clipId {}, {kind}, bars {from} to {to})",
                clip.name, clip.id
            ),
        });
    }
    if let Some(track) = track {
        let kind = if track.kind == "midi" {
            "instrument"
        } else {
            "audio"
        };
        chips.push(Chip {
            label: track.name.clone(),
            detail: format!("track \"{}\" (trackId {}, {kind})", track.name, track.id),
        });
    }
    let t = &s.transport;
    if t.cycle {
        let (from, to) = (bar(t.cycle_start_bar), bar(t.cycle_end_bar));
        chips.push(Chip {
            label: format!("Cycle {from}–{to}"),
            detail: format!("cycle range bars {from} to {to}"),
        });
    }
    chips
}

/// The message as sent: the person's words, then the selection. A slash command goes alone.
pub fn with_selection(prompt: &str, chips: &[Chip]) -> String {
    if chips.is_empty() || prompt.starts_with('/') {
        return prompt.to_string();
    }
    let details: Vec<&str> = chips.iter().map(|c| c.detail.as_str()).collect();
    format!("{prompt}{CONTEXT_MARK}{}]", details.join("; "))
}

/// A sent message split back into the person's words and the selection that went with it.
pub fn split_context(text: &str) -> (&str, &str) {
    match text.rfind(CONTEXT_MARK) {
        Some(at) if text.ends_with(']') => {
            (&text[..at], &text[at + CONTEXT_MARK.len()..text.len() - 1])
        }
        _ => (text, ""),
    }
}

/// The selection under a sent message, without the ids the tools needed.
pub fn caption(context: &str) -> String {
    let mut out = String::with_capacity(context.len());
    let mut rest = context;
    while let Some(open) = rest.find(" (") {
        let inner = &rest[open + 2..];
        if inner.starts_with("clipId") || inner.starts_with("trackId") {
            out.push_str(&rest[..open]);
            rest = inner.find(')').map_or("", |close| &inner[close + 1..]);
        } else {
            out.push_str(&rest[..open + 2]);
            rest = inner;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryolune_engine::store;

    #[test]
    fn the_selection_goes_along_and_comes_back_off() {
        let mut session = store::demo();
        let clip = session.clips[0].clone();
        session.view.selected_clip_id = Some(clip.id.clone());
        session.view.selected_track_id = Some(clip.track_id.clone());
        session.transport.cycle = true;
        session.transport.cycle_start_bar = 4.0;
        session.transport.cycle_end_bar = 8.0;
        let chips = selection_context(&session);
        assert_eq!(chips.len(), 3);
        assert!(chips[0].label.starts_with(&clip.name));
        assert!(chips[0].detail.contains(&format!("clipId {}", clip.id)));
        assert_eq!(chips[2].label, "Cycle 5–9");
        let sent = with_selection("Add drums", &chips);
        assert!(sent.starts_with("Add drums\n\n[Selected in the window: "));
        let (words, context) = split_context(&sent);
        assert_eq!(words, "Add drums");
        assert!(context.contains("cycle range bars 5 to 9"));
        assert!(!caption(context).contains("clipId"));
        assert!(!caption(context).contains("trackId"));
        // A slash command and an empty selection go alone.
        assert_eq!(with_selection("/mix", &chips), "/mix");
        assert_eq!(with_selection("Hello", &[]), "Hello");
        assert_eq!(split_context("No context"), ("No context", ""));
    }

    #[test]
    fn captions_keep_the_names() {
        let context = "track \"Keys\" (trackId t1, instrument)";
        assert_eq!(caption(context), "track \"Keys\"");
        assert_eq!(bar(2.5), "3.5");
        assert_eq!(bar(0.0), "1");
    }
}
