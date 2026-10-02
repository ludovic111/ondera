//! Slash commands in the message box: `/mix`, `/beat`… fill the message with a prompt to
//! edit before sending; `/generate`, `/takes` and `/variation` open their tab instead.
//! Choosing one never talks to the agent.

pub struct Slash {
    pub name: &'static str,
    pub label: &'static str,
    /// The message it writes; empty for the ones that open a tab.
    pub prompt: &'static str,
}

const fn s(name: &'static str, label: &'static str, prompt: &'static str) -> Slash {
    Slash {
        name,
        label,
        prompt,
    }
}

pub const COMMANDS: &[Slash] = &[
    s("generate", "Make a sound, loop or instrument with your sound service", ""),
    s("diagnose", "Diagnose any problem", "Diagnose this problem in my project: [describe the problem]. Inspect the relevant audio, MIDI, plugins, routing, files or settings. Identify the cause before making changes, explain it plainly, and verify the fix."),
    s("mix", "Review the mix", "Inspect this mix and suggest the three most useful improvements. Check gain, panning, effects, routing and audibility. Explain before changing anything."),
    s("arrange", "Develop the arrangement", "Help develop this song into an arrangement with distinct sections. Inspect the existing music, propose a structure, and preserve the original material."),
    s("beat", "Write a drum groove", "Add a four-bar drum groove on a new track that fits this project. Check that the new track is audible and explain any Solo or Mute blocker."),
    s("chords", "Write a chord progression", "Write a four-bar chord progression matching this project, on a new instrument track."),
    s("humanize", "Humanize the selected region", "Apply subtle timing and velocity humanization to the selected MIDI region. Preserve the pitches and keep notes within its boundaries."),
    s("quantize", "Tighten the selected region", "Quantize the selected MIDI region to the current grid, preserving its pitches and velocities."),
    s("scale", "Fit notes to the song’s key", "Fit the selected MIDI region to the project key using the closest notes. Preserve timing and rhythm."),
    s("velocity", "Shape the dynamics", "Shape the selected MIDI region’s velocities into a musical dynamic phrase. Preserve timing and pitch."),
    s("repeat", "Repeat the selected region", "Repeat the selected region three times immediately after it, preserving its notes and sound."),
    s("reverse", "Reverse the MIDI phrase", "Reverse the note timing of the selected MIDI region inside its current boundaries, preserving pitch and velocity."),
    s("legato", "Connect a melodic phrase", "Make the selected MIDI melody legato by extending each note to the next distinct onset, staying inside the region."),
    s("plugins", "Find an instrument or effect", "Find suitable installed instruments or effects for this project, including external plugins. Inspect their available parameters and recommend a choice before loading it."),
    s("explain", "Understand the project", "Explain this project in plain musical language: tracks, arrangement, instruments, effects and routing. Do not change anything."),
    s("variation", "Create a protected A/B variation", ""),
    s("takes", "Compare creative takes", ""),
    s("export", "Prepare an export", "Inspect the song and help prepare an audio export. Check its range, tails, levels and output format; ask me for any missing destination or format choice before exporting."),
];

/// The commands a draft like `/mi` offers: only while the draft is one word after `/`.
pub fn matches(draft: &str) -> Vec<&'static Slash> {
    match draft.strip_prefix('/') {
        Some(word) if !draft.contains(char::is_whitespace) => {
            let word = word.to_lowercase();
            COMMANDS
                .iter()
                .filter(|c| c.name.starts_with(&word))
                .collect()
        }
        _ => vec![],
    }
}

/// What a chosen command does.
#[derive(Debug, PartialEq)]
pub enum Choice {
    /// Write this message, for the person to finish and send.
    Draft(&'static str),
    Generate,
    Takes,
}

pub fn choose(command: &Slash) -> Choice {
    match command.name {
        "generate" => Choice::Generate,
        "takes" | "variation" => Choice::Takes,
        _ => Choice::Draft(command.prompt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slash_word_offers_commands_and_choosing_one_writes_its_prompt() {
        let found = matches("/diag");
        assert_eq!(found.len(), 1);
        assert!(
            matches!(choose(found[0]), Choice::Draft(p) if p.starts_with("Diagnose this problem"))
        );
        assert_eq!(choose(matches("/generate")[0]), Choice::Generate);
        assert_eq!(choose(matches("/variation")[0]), Choice::Takes);
        assert_eq!(matches("/").len(), COMMANDS.len());
        assert!(matches("/mix it").is_empty());
        assert!(matches("mix").is_empty());
        assert_eq!(matches("/R").len(), 2, "repeat and reverse, case aside");
    }
}
