//! Shared commands for MIDI interchange and configurable offline exports.
use crate::{
    control::{edit, opt, req, Host, Kind, Spec},
    export::{self, ExportOptions},
    midi_file::{self, ImportOptions},
    Result,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

pub const SPECS:&[Spec]=&[
    edit("session.importMidi","Import SMF type 0/1 MIDI into new instrument tracks in one undo step. Quarter-note positions are preserved; reports unsupported controller/tempo-map data.",&[
        req("path",Kind::String,"Source .mid or .midi file."),
        opt("startBar",Kind::Number,"Zero-based destination bar, default 0."),
        opt("importTempo",Kind::Boolean,"Apply the file's initial tempo and meter to the entire session (default false). Later changes are reported and ignored."),
    ]),
    edit("session.exportMidi","Export arrangement notes as SMF type 1 at 960 PPQ with initial tempo/meter. Does not convert audio or embed plugins; includes muted tracks.",&[
        req("path",Kind::String,"Destination .mid file, replaced atomically after success."),
        opt("trackIds",Kind::Array,"MIDI track IDs to export; defaults to all MIDI tracks."),
    ]),
    edit("session.exportAudio","Export an offline stereo WAV, AIFF, FLAC or Ogg Vorbis using the live plugin graph, with chosen rate, format, range and tail. The file type follows the path's extension and every type streams to disk. PCM clips above full scale; float retains headroom. Reports peak/clipping, and the bitrate for Ogg.",&[
        req("path",Kind::String,"Destination .wav, .aiff for AIFF, .flac for lossless FLAC (both pcm16 or pcm24 only) or .ogg for lossy Ogg Vorbis. Replaced atomically after success."),
        opt("sampleRate",Kind::Integer,"44100, 48000 (default), or 96000 Hz."),
        opt("format",Kind::String,"pcm16, pcm24 (default), or float32."),
        opt("startBar",Kind::Number,"Zero-based range start bar, default 0. Use bars or beats, never both."),
        opt("endBar",Kind::Number,"Exclusive end bar; defaults to arrangement end."),
        opt("startBeat",Kind::Number,"Zero-based start in quarter-note beats instead of bars."),
        opt("endBeat",Kind::Number,"Exclusive end in quarter-note beats instead of bars."),
        opt("tailSeconds",Kind::Number,"Effect release tail after range end, 0–120 seconds, default 3."),
        opt("dither",Kind::Boolean,"TPDF dither for integer PCM, default true. Ignored for float32."),
        opt("quality",Kind::Number,"Ogg Vorbis quality 0–1, default 0.6 (about 192 kbit/s; 0.4 ≈ 128, 0.8 ≈ 256). Ignored by the other file types, as format and dither are by Ogg."),
    ]),
    edit("session.exportStems","Export one stereo file per track (WAV, or AIFF, FLAC or Ogg Vorbis with `container`) into a new folder, publishing the entire set only on success. Solo-rendered nonlinear/shared effects can prevent exact summation to the full mix.",&[
        req("directory",Kind::String,"New destination folder; an existing folder is never replaced."),
        opt("trackIds",Kind::Array,"Track IDs to export; defaults to all tracks. Mute/solo are ignored."),
        opt("includeEffects",Kind::Boolean,"Include track inserts and send/bus processing, default true."),
        opt("includeMaster",Kind::Boolean,"Apply master inserts/fader to each stem, default false."),
        opt("sampleRate",Kind::Integer,"44100, 48000 (default), or 96000 Hz."),
        opt("container",Kind::String,"File type of every stem: wav (default), aiff, flac or ogg. aiff and flac need pcm16 or pcm24; ogg uses quality instead of format."),
        opt("format",Kind::String,"pcm16, pcm24 (default), or float32."),
        opt("startBar",Kind::Number,"Zero-based range start bar, default 0. Use bars or beats, never both."),
        opt("endBar",Kind::Number,"Exclusive end bar; defaults to arrangement end for every stem."),
        opt("startBeat",Kind::Number,"Zero-based start in quarter-note beats instead of bars."),
        opt("endBeat",Kind::Number,"Exclusive end in quarter-note beats instead of bars."),
        opt("tailSeconds",Kind::Number,"Effect release tail after range end, 0–120 seconds, default 3."),
        opt("dither",Kind::Boolean,"TPDF dither for integer PCM, default true. Ignored for float32."),
        opt("quality",Kind::Number,"Ogg Vorbis quality 0–1 for container ogg, default 0.6 (about 192 kbit/s)."),
    ]),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MidiImportArgs {
    path: PathBuf,
    #[serde(default)]
    start_bar: f64,
    #[serde(default)]
    import_tempo: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MidiExportArgs {
    path: PathBuf,
    #[serde(default)]
    track_ids: Option<Vec<String>>,
}

pub fn call(host: &mut dyn Host, name: &str, params: &Value, agent: bool) -> Result<Value> {
    // The main registry validates names and primitive types. Typed decoding also
    // protects direct internal callers and nested trackIds values.
    let mut params = params.clone();
    if let Some(map) = params.as_object_mut() {
        map.retain(|_, v| !v.is_null());
    }
    match name {
        "session.importMidi" => {
            let args: MidiImportArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            let options = ImportOptions {
                start_bar: args.start_bar,
                import_tempo: args.import_tempo,
            };
            let (command, report) =
                midi_file::import(&args.path, host.store().session(), &options, agent)?;
            host.dispatch(command)?;
            Ok(json!(report))
        }
        "session.exportMidi" => {
            let args: MidiExportArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Ok(json!(midi_file::export(
                host.store().session(),
                &args.path,
                args.track_ids.as_deref()
            )?))
        }
        "session.exportAudio" => {
            let path = take::<PathBuf>(&mut params, "path")?.ok_or("Export requires path")?;
            let options: ExportOptions =
                serde_json::from_value(params).map_err(|e| e.to_string())?;
            Ok(json!(export::mix(
                host.store().session(),
                host.library(),
                &path,
                &options
            )?))
        }
        "session.exportStems" => {
            let directory = take::<PathBuf>(&mut params, "directory")?
                .ok_or("Stem export requires directory")?;
            let track_ids = take::<Vec<String>>(&mut params, "trackIds")?;
            let include_effects = take::<bool>(&mut params, "includeEffects")?.unwrap_or(true);
            let include_master = take::<bool>(&mut params, "includeMaster")?.unwrap_or(false);
            let options: ExportOptions =
                serde_json::from_value(params).map_err(|e| e.to_string())?;
            Ok(json!(export::stems(
                host.store().session(),
                host.library(),
                &directory,
                &options,
                track_ids.as_deref(),
                include_effects,
                include_master
            )?))
        }
        _ => Err(format!("Unknown media command `{name}`")),
    }
}
fn take<T: serde::de::DeserializeOwned>(params: &mut Value, key: &str) -> Result<Option<T>> {
    params
        .as_object_mut()
        .ok_or("Expected command parameters")?
        .remove(key)
        .map(|value| serde_json::from_value(value).map_err(|e| format!("Invalid {key}: {e}")))
        .transpose()
}
