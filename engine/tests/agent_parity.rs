//! GUI parity for agents: whatever the window does goes through a registry command, so the
//! CLI, MCP and the built-in agent can do it too. These tests read the frontend's sources and
//! the parity table in docs/ and fail when they drift from the registry.
use ondera_engine::control::COMMANDS;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}
fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}
fn registered(name: &str) -> bool {
    COMMANDS.iter().any(|s| s.name == name)
}
fn families() -> BTreeSet<&'static str> {
    COMMANDS
        .iter()
        .filter_map(|s| s.name.split('.').next())
        .collect()
}

/// Double-quoted literals shaped like `family.action` whose family is a registry family,
/// with the text just before each (to skip `case` labels).
fn command_literals(source: &str) -> Vec<(String, String)> {
    let families = families();
    let mut out = vec![];
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.') {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b'"' {
                let text = &source[start..end];
                if let Some((family, action)) = text.split_once('.') {
                    if families.contains(family)
                        && !action.is_empty()
                        && action.chars().all(|c| c.is_ascii_alphabetic())
                    {
                        let before = source[..i].trim_end();
                        let context = before[before.len().saturating_sub(12)..].to_string();
                        out.push((text.to_string(), context));
                    }
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn frontend_sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                walk(&path, out);
            } else if (name.ends_with(".ts") || name.ends_with(".tsx")) && !name.contains(".test.")
            {
                out.push((path.clone(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let mut out = vec![];
    walk(&root().join("frontend/src"), &mut out);
    assert!(out.len() > 20, "frontend sources not found");
    out
}

/// The ids of the `ActionId` union in actions.ts.
fn action_ids() -> BTreeSet<String> {
    let source = read("frontend/src/state/actions.ts");
    let start = source
        .find("export type ActionId =")
        .expect("ActionId union");
    let body = &source[start..start + source[start..].find(';').unwrap()];
    body.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[test]
fn every_menu_shortcut_and_palette_action_maps_to_registry_commands() {
    let table: Value = serde_json::from_str(&read("docs/agent-parity.json")).unwrap();
    let mapped = table["actions"].as_object().expect("actions");
    let ids = action_ids();
    assert!(ids.len() > 40, "parsed {} action ids", ids.len());
    for id in &ids {
        let commands = mapped.get(id).unwrap_or_else(|| {
            panic!("action `{id}` in actions.ts has no entry in docs/agent-parity.json: add the registry commands that do the same")
        });
        let commands = commands.as_array().expect("a list of commands");
        assert!(!commands.is_empty(), "action `{id}` maps to nothing");
        for c in commands {
            let c = c.as_str().unwrap();
            assert!(
                registered(c),
                "action `{id}` maps to `{c}`, which is not a registry command"
            );
        }
    }
    for id in mapped.keys() {
        assert!(
            ids.contains(id),
            "docs/agent-parity.json maps `{id}`, which actions.ts no longer has"
        );
    }
    // The human table lists every action too.
    let doc = read("docs/AGENT_PARITY.md");
    for id in &ids {
        assert!(
            doc.contains(&format!("`{id}`")),
            "docs/AGENT_PARITY.md does not list action `{id}`"
        );
    }
}

#[test]
fn every_command_the_window_calls_by_name_is_in_the_registry() {
    let translated: BTreeSet<String> = command_literals(&read("frontend/src/state/native.ts"))
        .into_iter()
        .filter(|(_, before)| before.ends_with("case"))
        .map(|(name, _)| name)
        .collect();
    let mut unknown = vec![];
    for (path, source) in frontend_sources() {
        let file = path
            .strip_prefix(root())
            .unwrap_or(&path)
            .display()
            .to_string();
        for (name, before) in command_literals(&source) {
            if before.ends_with("case") || registered(&name) {
                continue;
            }
            // Presentation names (core/index.ts) that NativeStore translates into commands.
            if translated.contains(&name) {
                continue;
            }
            unknown.push(format!("{file}: {name}"));
        }
    }
    assert!(
        unknown.is_empty(),
        "The window calls names that are neither registry commands nor translated by NativeStore:\n{}",
        unknown.join("\n")
    );
}

#[test]
fn the_parity_document_names_only_real_commands() {
    let doc = read("docs/AGENT_PARITY.md");
    let families = families();
    let mut unknown = BTreeSet::new();
    for chunk in doc.split('`').skip(1).step_by(2) {
        // `strip.setParameter parameter=… text=…`: the command is the first word.
        let word = chunk.split_whitespace().next().unwrap_or("");
        let Some((family, action)) = word.split_once('.') else {
            continue;
        };
        if families.contains(family)
            && !action.is_empty()
            && action.chars().all(|c| c.is_ascii_alphabetic())
            && !registered(word)
        {
            unknown.insert(word.to_string());
        }
    }
    assert!(
        unknown.is_empty(),
        "docs/AGENT_PARITY.md names unknown commands: {unknown:?}"
    );
}

#[test]
fn every_command_says_what_it_does_and_documents_its_parameters() {
    let mut vague = vec![];
    for spec in COMMANDS.iter() {
        if spec.doc.len() < 30 || !spec.doc.ends_with(['.', ')']) {
            vague.push(format!("{}: say what it does, in a sentence", spec.name));
        }
        for p in spec.params {
            if p.doc.len() < 8 {
                vague.push(format!(
                    "{} `{}`: say what it is, with units or range",
                    spec.name, p.name
                ));
            }
        }
    }
    assert!(vague.is_empty(), "{}", vague.join("\n"));
}
