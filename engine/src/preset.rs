//! Plugin presets: named parameter sets (plus captured state for external plugins) stored
//! as JSON under the application data directory, and factory presets for the stock library.

use crate::{host::scan::data_dir, stock, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPreset {
    pub name: String,
    pub plugin_id: String,
    #[serde(default)]
    pub plugin_name: String,
    #[serde(default)]
    pub params: BTreeMap<u32, f64>,
    /// Base64 plugin state, for plugins whose sound is not fully described by parameters.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub blob: String,
    /// Shipped with Ondera; cannot be deleted or overwritten.
    #[serde(default)]
    pub factory: bool,
}

pub fn directory() -> PathBuf {
    data_dir().join("presets")
}
/// A filesystem-safe name that still reads like the original.
pub fn slug(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        "preset".into()
    } else {
        out
    }
}
fn plugin_dir(plugin_id: &str) -> PathBuf {
    directory().join(slug(&plugin_id.replace(':', "-")))
}
fn preset_path(plugin_id: &str, name: &str) -> PathBuf {
    plugin_dir(plugin_id).join(format!("{}.json", slug(name)))
}
pub fn factory(plugin_id: Option<&str>) -> Vec<PluginPreset> {
    stock::FACTORY_PRESETS
        .iter()
        .map(|(plugin, name, values)| PluginPreset {
            name: name.to_string(),
            plugin_id: format!("stock:{plugin}"),
            plugin_name: plugin.to_string(),
            params: values.iter().copied().collect(),
            blob: String::new(),
            factory: true,
        })
        .filter(|p| plugin_id.is_none_or(|id| id == p.plugin_id))
        .collect()
}
fn read(path: &Path) -> Result<PluginPreset> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("Preset file exceeds 64 MiB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut preset: PluginPreset = serde_json::from_str(&text)
        .map_err(|e| format!("Invalid preset {}: {e}", path.display()))?;
    preset.factory = false;
    if preset.params.values().any(|v| !v.is_finite()) {
        return Err(format!("Preset {} has non-finite values", path.display()));
    }
    Ok(preset)
}
/// Factory presets first, then the user's, sorted by name within each plugin.
pub fn list(plugin_id: Option<&str>) -> Result<Vec<PluginPreset>> {
    let mut all = factory(plugin_id);
    let dirs: Vec<PathBuf> = match plugin_id {
        Some(id) => vec![plugin_dir(id)],
        None => std::fs::read_dir(directory())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default(),
    };
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") && path.is_file() {
                if let Ok(preset) = read(&path) {
                    if plugin_id.is_none_or(|id| id == preset.plugin_id) {
                        all.push(preset);
                    }
                }
            }
        }
    }
    all.sort_by(|a, b| {
        (&a.plugin_id, !a.factory, a.name.to_lowercase()).cmp(&(
            &b.plugin_id,
            !b.factory,
            b.name.to_lowercase(),
        ))
    });
    Ok(all)
}
pub fn load(plugin_id: &str, name: &str) -> Result<PluginPreset> {
    let path = preset_path(plugin_id, name);
    if path.is_file() {
        return read(&path);
    }
    factory(Some(plugin_id))
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("No preset `{name}` for {plugin_id}. Use preset.list."))
}
pub fn save(preset: &PluginPreset) -> Result<PathBuf> {
    if preset.name.trim().is_empty() || preset.name.len() > 120 {
        return Err("Preset names are 1-120 characters".into());
    }
    if preset.plugin_id.is_empty() {
        return Err("Preset needs a plugin id".into());
    }
    if factory(Some(&preset.plugin_id))
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&preset.name))
    {
        return Err(format!(
            "`{}` is a factory preset; choose another name",
            preset.name
        ));
    }
    if preset.blob.len() > 64 * 1024 * 1024 || preset.params.values().any(|v| !v.is_finite()) {
        return Err("Preset state is too large or invalid".into());
    }
    let path = preset_path(&preset.plugin_id, &preset.name);
    std::fs::create_dir_all(path.parent().ok_or("Preset path has no parent")?)
        .map_err(|e| e.to_string())?;
    let mut stored = preset.clone();
    stored.factory = false;
    let json = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    crate::document::atomic_write(&path, |f| {
        use std::io::Write;
        f.write_all(json.as_bytes()).map_err(|e| e.to_string())
    })?;
    Ok(path)
}
pub fn delete(plugin_id: &str, name: &str) -> Result<()> {
    let path = preset_path(plugin_id, name);
    if path.is_file() {
        return std::fs::remove_file(&path).map_err(|e| e.to_string());
    }
    if factory(Some(plugin_id))
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(name))
    {
        return Err("Factory presets cannot be deleted".into());
    }
    Err(format!("No preset `{name}` for {plugin_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_round_trip_and_factory_presets_are_protected() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("ONDERA_DATA_DIR", dir.path());
        let factory_count = list(Some("stock:Space")).unwrap().len();
        assert!(factory_count >= 2);
        let preset = PluginPreset {
            name: "My hall / take 2".into(),
            plugin_id: "stock:Space".into(),
            plugin_name: "Space".into(),
            params: [(0, 80.0), (4, 35.0)].into_iter().collect(),
            blob: String::new(),
            factory: false,
        };
        save(&preset).unwrap();
        let listed = list(Some("stock:Space")).unwrap();
        assert_eq!(listed.len(), factory_count + 1);
        assert_eq!(
            load("stock:Space", "My hall / take 2").unwrap().params[&0],
            80.0
        );
        assert!(save(&PluginPreset {
            name: "Cathedral".into(),
            ..preset.clone()
        })
        .is_err());
        assert!(delete("stock:Space", "Cathedral").is_err());
        delete("stock:Space", "My hall / take 2").unwrap();
        assert_eq!(list(Some("stock:Space")).unwrap().len(), factory_count);
        assert!(load("stock:Space", "cathedral").unwrap().factory);
        std::env::remove_var("ONDERA_DATA_DIR");
    }
}
