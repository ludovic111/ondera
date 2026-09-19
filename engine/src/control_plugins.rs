//! The plugin library as a musician browses it: every plugin filed in a sound folder
//! (Synths, Drums, Dynamics, Space & Time…), favourites, and what was used last.
use crate::{
    control::{edit, opt, query, req, Args, Host, Kind, Spec},
    host::scan,
    plugin::Descriptor,
    settings::Plugins,
    Result,
};
use serde_json::{json, Value};

/// Folders in the order the browser shows them.
pub const INSTRUMENT_FOLDERS: &[&str] = &[
    "Synths",
    "Keys",
    "Bass",
    "Drums",
    "Pads",
    "Samplers",
    "Textures",
    "Other Instruments",
];
pub const EFFECT_FOLDERS: &[&str] = &[
    "Dynamics",
    "EQ & Filter",
    "Distortion",
    "Modulation",
    "Space & Time",
    "Pitch",
    "Utility",
    "Other Effects",
];

pub const SPECS: &[Spec] = &[
    query("plugin.folders", "The sound folders of the plugin library with how many instruments and effects each holds, plus favourites and recents.", &[]),
    edit("plugin.setFavorite", "Star or unstar a plugin so it shows under Favourites in the browser.", &[
        req("pluginId", Kind::String, "Plugin id from plugin.list."),
        req("favorite", Kind::Boolean, "Star (true) or unstar (false)."),
    ]),
    edit("plugin.setFolder", "File a plugin under another sound folder, or a new one of your own. Omit folder to return it to the automatic one.", &[
        req("pluginId", Kind::String, "Plugin id from plugin.list."),
        opt("folder", Kind::String, "Folder name, 1-40 characters."),
    ]),
];

fn has(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| text.contains(w))
}

/// The folder a plugin belongs in when the user has not filed it themselves. Formats describe
/// themselves differently (CLAP features, VST3 sub-categories, a bare AU type), so the name
/// is read as well as the category.
pub fn automatic_folder(d: &Descriptor) -> &'static str {
    let text = format!("{} {}", d.category, d.name).to_lowercase();
    if d.instrument && (!d.effect || has(&text, &["instrument", "synth"])) {
        return if has(&text, &["bass", "808"]) {
            "Bass"
        } else if has(&text, &["drum", "percuss", "kick", "snare", "beat"]) {
            "Drums"
        } else if has(
            &text,
            &[
                "piano", "keys", "organ", "rhodes", "clav", "wurli", "mallet",
            ],
        ) {
            "Keys"
        } else if has(&text, &["pad", "choir", "string", "ambient", "atmos"]) {
            "Pads"
        } else if has(&text, &["sampl", "rompler", "player"]) {
            "Samplers"
        } else if has(&text, &["riser", "texture", "noise", "drone", "fx"]) {
            "Textures"
        } else if has(
            &text,
            &["synth", "instrument", "lead", "pluck", "wavetable", "fm"],
        ) {
            "Synths"
        } else {
            "Other Instruments"
        };
    }
    if has(
        &text,
        &[
            "compress",
            "limit",
            "gate",
            "dynamic",
            "transient",
            "expander",
            "de-ess",
            "deess",
            "comp",
        ],
    ) {
        "Dynamics"
    } else if has(
        &text,
        &[
            "distort", "satur", "drive", "fuzz", "crush", "amp", "tape", "clip", "lo-fi", "lofi",
        ],
    ) {
        "Distortion"
    } else if has(
        &text,
        &[
            "reverb", "delay", "echo", "space", "room", "hall", "plate", "shimmer",
        ],
    ) {
        "Space & Time"
    } else if has(
        &text,
        &[
            "chorus", "phaser", "flang", "tremolo", "modulat", "vibrato", "rotary", "pan",
        ],
    ) {
        "Modulation"
    } else if has(&text, &["pitch", "tune", "vocoder", "harmon", "shift"]) {
        "Pitch"
    } else if has(&text, &["eq", "filter", "tilt", "shelf"]) {
        "EQ & Filter"
    } else if has(
        &text,
        &[
            "util", "gain", "meter", "analy", "stereo", "width", "tool", "imager", "trim",
        ],
    ) {
        "Utility"
    } else {
        "Other Effects"
    }
}

/// The folder shown for a plugin: the user's filing when there is one.
pub fn folder(d: &Descriptor, library: &Plugins) -> String {
    library
        .folders
        .get(&d.id)
        .cloned()
        .unwrap_or_else(|| automatic_folder(d).to_string())
}

fn entry(d: &Descriptor, library: &Plugins) -> Value {
    let mut value = serde_json::to_value(d).unwrap_or_else(|_| json!({}));
    value["folder"] = json!(folder(d, library));
    value["favorite"] = json!(library.favorites.contains(&d.id));
    value
}

pub(crate) fn page(args: &Args, library: &Plugins) -> Result<Value> {
    let format = args.opt_str("format");
    if format.is_some_and(|format| !["stock", "native", "clap", "vst3", "au"].contains(&format)) {
        return Err("Plugin format must be stock, native, clap, vst3 or au".into());
    }
    let kind = args.opt_str("kind");
    if kind.is_some_and(|kind| !["instrument", "effect"].contains(&kind)) {
        return Err("Plugin kind must be instrument or effect".into());
    }
    let sort = args.opt_str("sort").unwrap_or("name");
    if !["name", "recent"].contains(&sort) {
        return Err("Plugin sort must be name or recent".into());
    }
    let limit = args.opt_int("limit").unwrap_or(50);
    let offset = args.opt_int("offset").unwrap_or(0);
    if !(1..=200).contains(&limit) || offset < 0 {
        return Err("Plugin limit must be 1-200 and offset must be non-negative".into());
    }
    let query = args.opt_str("query").unwrap_or("").to_lowercase();
    let wanted_folder = args.opt_str("folder").map(str::to_lowercase);
    let favorite = args.opt_bool("favorite").unwrap_or(false);
    let mut filtered: Vec<Descriptor> = scan::installed()
        .into_iter()
        .filter(|plugin| {
            format.is_none_or(|format| format == plugin.format.prefix())
                && kind.is_none_or(|kind| {
                    if kind == "instrument" {
                        plugin.instrument
                    } else {
                        plugin.effect
                    }
                })
                && (!favorite || library.favorites.contains(&plugin.id))
                && wanted_folder
                    .as_ref()
                    .is_none_or(|f| folder(plugin, library).to_lowercase() == *f)
                && (query.is_empty()
                    || plugin.name.to_lowercase().contains(&query)
                    || plugin.vendor.to_lowercase().contains(&query)
                    || plugin.id.to_lowercase().contains(&query)
                    || folder(plugin, library).to_lowercase().contains(&query))
        })
        .collect();
    if sort == "recent" {
        let rank = |d: &Descriptor| {
            library
                .recent
                .iter()
                .position(|id| *id == d.id)
                .unwrap_or(usize::MAX)
        };
        filtered.retain(|d| rank(d) != usize::MAX);
        filtered.sort_by_key(rank);
    }
    let total = filtered.len();
    let offset = usize::try_from(offset).map_err(|_| "Plugin offset is too large")?;
    let end = offset.saturating_add(limit as usize).min(total);
    let page: Vec<Value> = filtered[offset.min(total)..end]
        .iter()
        .map(|d| entry(d, library))
        .collect();
    Ok(
        json!({"plugins":page,"total":total,"offset":offset,"limit":limit,
        "nextOffset":if end<total {Some(end)} else {None},"cachePath":scan::cache_path()}),
    )
}

/// Remember that a plugin was just loaded. Best effort: a read-only settings file must not
/// fail the edit that loaded the plugin.
pub(crate) fn note_recent(host: &mut dyn Host, plugin_id: &str) {
    let mut settings = host.settings();
    if settings
        .plugins
        .recent
        .first()
        .is_some_and(|id| id == plugin_id)
    {
        return;
    }
    settings.plugins.recent.retain(|id| id != plugin_id);
    settings.plugins.recent.insert(0, plugin_id.to_string());
    settings.plugins.recent.truncate(12);
    let _ = host.update_settings(settings);
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args) -> Result<Value> {
    let installed = scan::installed();
    let known = |id: &str| -> Result<Descriptor> {
        installed
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .ok_or_else(|| format!("Unknown plugin `{id}`. Run plugin.scan, then plugin.list."))
    };
    match name {
        "plugin.folders" => {
            let library = host.settings().plugins;
            let mut names: Vec<String> = INSTRUMENT_FOLDERS
                .iter()
                .chain(EFFECT_FOLDERS)
                .map(|s| s.to_string())
                .collect();
            for custom in library.folders.values() {
                if !names.contains(custom) {
                    names.push(custom.clone());
                }
            }
            let folders: Vec<Value> = names
                .iter()
                .map(|name| {
                    let inside: Vec<&Descriptor> = installed.iter().filter(|d| folder(d, &library) == *name).collect();
                    json!({ "name": name,
                        "custom": !INSTRUMENT_FOLDERS.contains(&name.as_str()) && !EFFECT_FOLDERS.contains(&name.as_str()),
                        "instruments": inside.iter().filter(|d| d.instrument).count(),
                        "effects": inside.iter().filter(|d| d.effect).count() })
                })
                .filter(|f| f["instruments"] != 0 || f["effects"] != 0 || f["custom"] == true)
                .collect();
            Ok(
                json!({ "folders": folders, "favorites": library.favorites, "recent": library.recent }),
            )
        }
        "plugin.setFavorite" => {
            let plugin = known(a.str("pluginId")?)?;
            let favorite = a.bool("favorite")?;
            let mut settings = host.settings();
            settings.plugins.favorites.retain(|id| *id != plugin.id);
            if favorite {
                settings.plugins.favorites.push(plugin.id.clone());
            }
            let reply = entry(&plugin, &settings.plugins);
            host.update_settings(settings)?;
            Ok(reply)
        }
        "plugin.setFolder" => {
            let plugin = known(a.str("pluginId")?)?;
            let mut settings = host.settings();
            match a.opt_str("folder").map(str::trim).filter(|f| !f.is_empty()) {
                Some(name) if name == automatic_folder(&plugin) => {
                    settings.plugins.folders.remove(&plugin.id);
                }
                Some(name) => {
                    settings
                        .plugins
                        .folders
                        .insert(plugin.id.clone(), name.to_string());
                }
                None => {
                    settings.plugins.folders.remove(&plugin.id);
                }
            }
            settings.validate()?;
            let reply = entry(&plugin, &settings.plugins);
            host.update_settings(settings)?;
            Ok(reply)
        }
        _ => Err(format!("Unknown command `{name}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stock_plugin_lands_in_a_named_folder() {
        for d in crate::stock::descriptors() {
            let folder = automatic_folder(&d);
            assert!(
                !folder.starts_with("Other"),
                "{} fell into {folder}",
                d.name
            );
            let list = if d.instrument {
                INSTRUMENT_FOLDERS
            } else {
                EFFECT_FOLDERS
            };
            assert!(list.contains(&folder), "{} -> {folder}", d.name);
        }
    }
}
