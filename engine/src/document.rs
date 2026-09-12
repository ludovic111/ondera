use crate::{
    audio::{self, Library},
    model::Session,
    Result,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Write, path::Path, sync::Arc};

#[derive(Deserialize, Serialize)]
struct SessionFile {
    format: String,
    version: u32,
    session: Session,
    audio: HashMap<String, String>,
}

pub fn decode_session(json: &str) -> Result<(Session, Library)> {
    if json.len() > 768 * 1024 * 1024 {
        return Err("Session file exceeds 768 MiB".into());
    }
    let mut file: SessionFile =
        serde_json::from_str(json).map_err(|e| format!("Invalid session: {e}"))?;
    if file.format != "ondera-session" || file.version != 1 {
        return Err("Unsupported Ondera session format or version".into());
    }
    file.session.validate()?;
    let mut library = Library::new();
    for (id, src) in &file.session.sources {
        if src.origin == "generated" {
            continue;
        }
        let data = file
            .audio
            .remove(id)
            .ok_or_else(|| format!("Session is missing embedded audio: {}", src.name))?;
        let bytes = STANDARD
            .decode(data)
            .map_err(|e| format!("Invalid embedded audio: {e}"))?;
        library.insert(id.clone(), Arc::new(audio::decode(bytes, Some("wav"))?));
    }
    audio::prepare_sources(&file.session, &mut library)?;
    file.session.transport.playing = false;
    file.session.transport.recording = false;
    Ok((file.session, library))
}
pub fn load(path: &Path) -> Result<(Session, Library)> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 768 * 1024 * 1024 {
        return Err("Session file exceeds 768 MiB".into());
    }
    decode_session(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
}
pub fn save(session: &Session, library: &Library, path: &Path) -> Result<()> {
    session.validate()?;
    let mut audio = HashMap::new();
    for src in session.sources.values().filter(|s| s.origin != "generated") {
        let buf = library
            .get(&src.id)
            .ok_or_else(|| format!("Cannot save: audio missing for {}", src.name))?;
        audio.insert(src.id.clone(), STANDARD.encode(audio::encode_wav(buf)?));
    }
    let mut stopped = session.clone();
    stopped.transport.playing = false;
    stopped.transport.recording = false;
    let file = SessionFile {
        format: "ondera-session".into(),
        version: 1,
        session: stopped,
        audio,
    };
    atomic_write(path, |f| {
        let mut writer = std::io::BufWriter::new(f);
        serde_json::to_writer(&mut writer, &file).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())
    })
}
/// Write, flush and sync a sibling temporary file, then atomically replace the destination.
pub fn atomic_write(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    write(tmp.as_file_mut())?;
    tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    tmp.persist(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
