//! The network half of `generate.audio`: one call to the service chosen in Settings >
//! Generation, on a worker thread. The engine (`control_generate`) shapes the request from the
//! song, keeps the result and places it; this file only speaks each service's API and hands
//! back the audio bytes as the service sent them.
use ryolune_engine::{
    control_generate::{GenKind, Request},
    settings::{Service, Settings},
};
use serde_json::{json, Value};
use std::{io::Read, time::Duration};

type Result<T> = std::result::Result<T, String>;

/// A generous ceiling: three minutes of WAV is about 30 MB.
const MAX_BYTES: u64 = 200 * 1024 * 1024;

/// Audio as a service returned it.
pub(crate) struct Sound {
    pub bytes: Vec<u8>,
    pub extension: String,
    /// The model the service used, when it says.
    pub model: String,
}

fn client() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(30)))
        // A song takes the service a minute or two to compose.
        .timeout_recv_response(Some(Duration::from_secs(420)))
        .timeout_recv_body(Some(Duration::from_secs(300)))
        .user_agent(format!("ryolune/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Ask the request's service for the sound.
pub(crate) fn fetch(settings: &Settings, request: &Request) -> Result<Sound> {
    let key = settings.generation_key(request.service);
    match request.service {
        Service::ElevenLabs => elevenlabs(&key.ok_or("No ElevenLabs API key")?, request),
        Service::Stability => stability(&key.ok_or("No Stability AI API key")?, request),
        Service::Fal => fal(
            &key.ok_or("No fal.ai API key")?,
            settings.generation.fal_model.trim(),
            request,
        ),
        Service::Custom => custom(
            settings.generation.custom_url.trim(),
            key.as_deref(),
            request,
        ),
    }
}

fn elevenlabs(key: &str, r: &Request) -> Result<Sound> {
    // Eleven Music composes songs and loops of 3 s or more; the sound-effects model makes
    // one-shots, instrument notes and short loops (up to 30 s).
    let music = matches!(r.kind, GenKind::Song) || (r.kind == GenKind::Loop && r.seconds >= 3.0);
    let (url, body, model) = if music {
        let mut body = json!({
            "prompt": r.prompt,
            "music_length_ms": (r.seconds * 1000.0).round().clamp(3_000.0, 600_000.0) as u64,
            "force_instrumental": r.instrumental || r.kind == GenKind::Loop,
        });
        if let Some(seed) = r.seed {
            body["seed"] = json!(seed);
        }
        ("https://api.elevenlabs.io/v1/music", body, "music_v1")
    } else {
        let body = json!({
            "text": r.prompt,
            "duration_seconds": r.seconds.clamp(0.5, 30.0),
            "prompt_influence": 0.6,
            "loop": r.kind == GenKind::Loop,
        });
        (
            "https://api.elevenlabs.io/v1/sound-generation",
            body,
            "eleven_text_to_sound_v2",
        )
    };
    let mut response = client()
        .post(url)
        .query("output_format", "mp3_44100_192")
        .header("xi-api-key", key)
        .header("accept", "audio/mpeg")
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| format!("Could not reach ElevenLabs: {e}"))?;
    let sound = audio_response(&mut response, "ElevenLabs")?;
    Ok(Sound {
        model: model.into(),
        ..sound
    })
}

fn stability(key: &str, r: &Request) -> Result<Sound> {
    let mut fields = vec![
        ("prompt", r.prompt.clone()),
        (
            "duration",
            format!("{}", r.seconds.clamp(1.0, 190.0).round()),
        ),
        ("output_format", "wav".to_string()),
        ("model", "stable-audio-2.5".to_string()),
    ];
    if let Some(seed) = r.seed {
        fields.push(("seed", (seed % 4_294_967_295).to_string()));
    }
    let boundary = format!("ryolune-{}", std::process::id());
    let mut body = String::new();
    for (name, value) in &fields {
        body.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    body.push_str(&format!("--{boundary}--\r\n"));
    let mut response = client()
        .post("https://api.stability.ai/v2beta/audio/stable-audio-2/text-to-audio")
        .header("authorization", &format!("Bearer {key}"))
        .header("accept", "audio/*")
        .header(
            "content-type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send(body)
        .map_err(|e| format!("Could not reach Stability AI: {e}"))?;
    let sound = audio_response(&mut response, "Stability AI")?;
    Ok(Sound {
        model: "stable-audio-2.5".into(),
        ..sound
    })
}

fn fal(key: &str, model: &str, r: &Request) -> Result<Sound> {
    if model.is_empty() {
        return Err("Choose a fal.ai model in Settings > Generation".into());
    }
    let mut body = json!({ "prompt": r.prompt });
    // fal's models name the length differently; Lyria makes fixed 30 s clips.
    if model.contains("stable-audio") {
        body["seconds_total"] = json!(r.seconds.round().clamp(1.0, 47.0) as u64);
    } else if !model.contains("lyria") {
        body["duration"] = json!(r.seconds);
    }
    if let Some(seed) = r.seed {
        body["seed"] = json!(seed % 4_294_967_295);
    }
    let mut response = client()
        .post(&format!("https://fal.run/{model}"))
        .header("authorization", &format!("Key {key}"))
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| format!("Could not reach fal.ai: {e}"))?;
    let status = response.status().as_u16();
    let text = read_text(&mut response)?;
    if status != 200 {
        return Err(format!("fal.ai answered {status}: {}", api_error(&text)));
    }
    let value: Value =
        serde_json::from_str(&text).map_err(|_| "fal.ai did not answer with JSON".to_string())?;
    let url = audio_url(&value).ok_or("fal.ai's answer holds no audio file")?;
    let sound = download(&url, false)?;
    Ok(Sound {
        model: model.into(),
        ..sound
    })
}

/// A custom endpoint receives the request as JSON and answers with audio bytes, or with JSON
/// holding an `audio` (base64) or a `url` to fetch. docs/AI_CONTROL.md describes the contract.
fn custom(url: &str, key: Option<&str>, r: &Request) -> Result<Sound> {
    if url.is_empty() {
        return Err("Set the custom endpoint's address in Settings > Generation".into());
    }
    let body = json!({
        "prompt": r.prompt,
        "description": r.description,
        "kind": r.kind.key(),
        "seconds": r.seconds,
        "instrumental": r.instrumental,
        "seed": r.seed,
        "name": r.name,
    });
    let mut request = client()
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "audio/*, application/json");
    if let Some(key) = key {
        request = request.header("authorization", &format!("Bearer {key}"));
    }
    let mut response = request
        .send(body.to_string())
        .map_err(|e| format!("Could not reach {url}: {e}"))?;
    let status = response.status().as_u16();
    let content_type = content_type(&response);
    if status == 200 && content_type.starts_with("audio/") {
        return audio_response(&mut response, "The endpoint");
    }
    let text = read_text(&mut response)?;
    if status != 200 {
        return Err(format!(
            "The endpoint answered {status}: {}",
            api_error(&text)
        ));
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|_| "The endpoint answered neither audio nor JSON".to_string())?;
    let model = value["model"].as_str().unwrap_or_default().to_string();
    if let Some(data) = value["audio"].as_str() {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|_| "The endpoint's audio is not valid base64".to_string())?;
        let extension = value["format"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| sniff(&bytes).into());
        return Ok(Sound {
            bytes,
            extension,
            model,
        });
    }
    let found = audio_url(&value).ok_or("The endpoint's answer holds no audio or url")?;
    let sound = download(&found, url.starts_with("http://"))?;
    Ok(Sound { model, ..sound })
}

/// Read an audio answer, or turn an error answer into a message.
fn audio_response(response: &mut ureq::http::Response<ureq::Body>, who: &str) -> Result<Sound> {
    let status = response.status().as_u16();
    let content_type = content_type(response);
    if status != 200 {
        let text = read_text(response)?;
        return Err(format!("{who} answered {status}: {}", api_error(&text)));
    }
    let bytes = read_bytes(response)?;
    Ok(Sound {
        extension: extension_for(&content_type, "").unwrap_or_else(|| sniff(&bytes).into()),
        bytes,
        model: String::new(),
    })
}

/// Fetch a result file. The key never goes along: result URLs are signed on their own.
fn download(url: &str, allow_http: bool) -> Result<Sound> {
    if !(url.starts_with("https://") || (allow_http && url.starts_with("http://"))) {
        return Err("The service pointed at an audio file that is not https".into());
    }
    let mut response = client()
        .get(url)
        .call()
        .map_err(|e| format!("Could not download the sound: {e}"))?;
    if response.status() != 200 {
        return Err(format!(
            "Downloading the sound failed with HTTP {}",
            response.status()
        ));
    }
    let content_type = content_type(&response);
    let bytes = read_bytes(&mut response)?;
    let path = url.split(['?', '#']).next().unwrap_or(url);
    Ok(Sound {
        extension: extension_for(&content_type, path).unwrap_or_else(|| sniff(&bytes).into()),
        bytes,
        model: String::new(),
    })
}

fn content_type(response: &ureq::http::Response<ureq::Body>) -> String {
    response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn read_bytes(response: &mut ureq::http::Response<ureq::Body>) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    response
        .body_mut()
        .as_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("The download stopped: {e}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The sound is larger than 200 MB".into());
    }
    if bytes.is_empty() {
        return Err("The service sent an empty sound".into());
    }
    Ok(bytes)
}

fn read_text(response: &mut ureq::http::Response<ureq::Body>) -> Result<String> {
    let mut text = String::new();
    response
        .body_mut()
        .as_reader()
        .take(4 * 1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|e| format!("Could not read the answer: {e}"))?;
    Ok(text)
}

/// The useful line of an API error body.
fn api_error(text: &str) -> String {
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let found = [
        &value["detail"]["message"],
        &value["detail"],
        &value["error"]["message"],
        &value["error"],
        &value["message"],
        &value["errors"][0],
    ]
    .into_iter()
    .find_map(|v| v.as_str().map(str::to_string));
    let line = found.unwrap_or_else(|| text.trim().to_string());
    line.chars().take(300).collect()
}

/// The first audio file URL in a JSON answer: an object with an audio content type, or any
/// `url` ending in an audio extension, searched depth first.
fn audio_url(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(url) = map.get("url").and_then(Value::as_str) {
                let typed = map
                    .get("content_type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.starts_with("audio/"));
                if typed || extension_for("", url).is_some() {
                    return Some(url.to_string());
                }
            }
            for key in ["audio", "audio_file", "audio_url", "output", "data"] {
                if let Some(found) = map.get(key).and_then(audio_url) {
                    return Some(found);
                }
                if let Some(url) = map.get(key).and_then(Value::as_str) {
                    if url.starts_with("http") {
                        return Some(url.to_string());
                    }
                }
            }
            map.values().find_map(audio_url)
        }
        Value::Array(items) => items.iter().find_map(audio_url),
        _ => None,
    }
}

/// The file extension for a content type, else for a URL path.
fn extension_for(content_type: &str, path: &str) -> Option<String> {
    let from_type = match content_type.split(';').next().unwrap_or("").trim() {
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => Some("wav"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/ogg" => Some("ogg"),
        "audio/opus" => Some("opus"),
        "audio/mp4" | "audio/aac" | "audio/x-m4a" => Some("m4a"),
        "audio/webm" => Some("webm"),
        "audio/aiff" | "audio/x-aiff" => Some("aiff"),
        _ => None,
    };
    from_type.map(str::to_string).or_else(|| {
        let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
        [
            "mp3", "wav", "flac", "ogg", "opus", "m4a", "aac", "webm", "aiff",
        ]
        .contains(&ext.as_str())
        .then_some(ext)
    })
}

/// Guess a format from the first bytes when nothing else says.
fn sniff(bytes: &[u8]) -> &'static str {
    match bytes {
        [b'R', b'I', b'F', b'F', ..] => "wav",
        [b'f', b'L', b'a', b'C', ..] => "flac",
        [b'O', b'g', b'g', b'S', ..] => "ogg",
        [b'F', b'O', b'R', b'M', ..] => "aiff",
        [_, _, _, _, b'f', b't', b'y', b'p', ..] => "m4a",
        _ => "mp3",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_answers_are_found_wherever_a_service_puts_them() {
        let fal = json!({"audio_file": {"url": "https://v3.fal.media/x/out.wav", "content_type": "audio/wav"}});
        assert_eq!(audio_url(&fal).unwrap(), "https://v3.fal.media/x/out.wav");
        let nested = json!({"output": [{"audio": "https://cdn.example/a.mp3?sig=1"}]});
        assert_eq!(
            audio_url(&nested).unwrap(),
            "https://cdn.example/a.mp3?sig=1"
        );
        assert!(audio_url(&json!({"image": {"url": "https://x/y.png"}})).is_none());
        assert_eq!(
            extension_for("audio/mpeg; charset=x", "").as_deref(),
            Some("mp3")
        );
        assert_eq!(extension_for("", "/a/b.FLAC").as_deref(), Some("flac"));
        assert_eq!(sniff(b"RIFF....WAVE"), "wav");
        assert_eq!(
            api_error(r#"{"detail":{"message":"quota exceeded"}}"#),
            "quota exceeded"
        );
        assert_eq!(
            api_error(r#"{"errors":["prompt too long"]}"#),
            "prompt too long"
        );
    }

    #[test]
    fn a_custom_endpoint_answers_with_audio_or_json() {
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (i, stream) in listener.incoming().take(2).enumerate() {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(request["kind"], "sound");
                let reply = if i == 0 {
                    let audio = b"RIFF0000WAVEfmt ";
                    let mut head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: audio/wav\r\ncontent-length: {}\r\n\r\n",
                        audio.len()
                    )
                    .into_bytes();
                    head.extend_from_slice(audio);
                    head
                } else {
                    let json = r#"{"audio":"T2dnUw==","model":"local-gen"}"#;
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{json}",
                        json.len()
                    )
                    .into_bytes()
                };
                stream.write_all(&reply).unwrap();
            }
        });
        let request = Request {
            service: Service::Custom,
            kind: GenKind::Sound,
            description: "a door".into(),
            prompt: "a door".into(),
            seconds: 2.0,
            instrumental: true,
            seed: None,
            name: "Door".into(),
            place: true,
            track_id: None,
            start_bar: None,
            root_note: 60,
            fit_seconds: None,
        };
        let url = format!("http://{address}/make");
        let first = custom(&url, None, &request).unwrap();
        assert_eq!(first.extension, "wav");
        let second = custom(&url, Some("k"), &request).unwrap();
        assert_eq!(second.bytes, b"OggS");
        assert_eq!(second.extension, "ogg");
        assert_eq!(second.model, "local-gen");
    }
}
