//! Dense-session locate throughput; construction occurs outside the measurement.
use ondera_engine::{
    audio::Library,
    model::{Clip, ClipData, Note},
    render::Renderer,
    store,
};
use std::{collections::HashMap, time::Instant};
fn main() {
    let mut session = store::empty();
    let template = session.tracks[0].clone();
    session.tracks.clear();
    session.clips.clear();
    session.strips.clear();
    for track_index in 0..128 {
        let mut track = template.clone();
        track.id = format!("track-{track_index}");
        track.kind = "midi".into();
        session.tracks.push(track.clone());
        let count = 200_000 / 128 + usize::from(track_index < 200_000 % 128);
        session.clips.push(Clip {
            id: format!("clip-{track_index}"),
            name: "Dense score".into(),
            agent: false,
            track_id: track.id,
            start_bar: 0.0,
            length_bars: 100.0,
            data: ClipData::Midi {
                notes: (0..count)
                    .map(|n| Note {
                        id: format!("note-{n}"),
                        start: n as f64 * 0.25,
                        length: 0.2,
                        pitch: 48 + (track_index % 24) as u8,
                        velocity: 90,
                        agent: false,
                    })
                    .collect(),
                controllers: vec![],
            },
        });
    }
    let mut renderer = Renderer::new(session, &Library::new(), 48000, &HashMap::new()).unwrap();
    let mut times = vec![];
    for n in 0..50 {
        let start = Instant::now();
        renderer.locate(389.875 + f64::from(n % 2) * 0.25);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.total_cmp(b));
    println!(
        "{}",
        serde_json::json!({"tracks":128,"notes":200000,"locates":times.len(),"medianMs":times[times.len()/2],"worstMs":times.last(),"voiceOverflows":renderer.voice_overflows,"noteOverflows":renderer.note_overflows})
    );
}
