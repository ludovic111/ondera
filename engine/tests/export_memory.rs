//! A long FLAC export streams: the heap never holds the song. Alone in this binary because
//! it measures the whole process. The twenty-minute run takes minutes in a debug build, so it
//! is ignored by default: `cargo test -p ondera-engine --test export_memory -- --ignored`.
use ondera_engine::{audio, export, model::*, store};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
struct Peak;
unsafe impl GlobalAlloc for Peak {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Peak = Peak;

#[test]
fn a_ninety_second_flac_export_keeps_memory_bounded() {
    export_stays_bounded(45);
}
#[test]
#[ignore = "minutes in a debug build"]
fn a_twenty_minute_flac_export_keeps_memory_bounded() {
    export_stays_bounded(600);
}

/// One bar is two seconds at 120 in 4/4. The heap may grow by a fixed working set (the
/// renderer's buffers, one FLAC block, the two-second source resampled) and no more, however
/// long the song is.
fn export_stays_bounded(bars: usize) {
    let mut session = store::empty();
    session.tracks.retain(|t| t.kind == "audio");
    session.tracks.truncate(1);
    session.transport.tempo = 120.0;
    let mut seed = 1u32;
    let noise: Vec<[f32; 2]> = (0..48000 * 2)
        .map(|i| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let hiss = (seed >> 8) as f32 / 16_777_216.0 - 0.5;
            let tone = (i as f32 * 0.02).sin() * 0.4;
            [tone + hiss * 0.05, tone * 0.7 - hiss * 0.05]
        })
        .collect();
    let buffer = Arc::new(audio::AudioBuffer::new(48000, noise).unwrap());
    session.sources.insert(
        "src".into(),
        Source {
            id: "src".into(),
            name: "Noise".into(),
            sample_rate: 48000,
            channels: 2,
            file_name: Some("noise.wav".into()),
            duration_seconds: buffer.duration(),
            origin: "file".into(),
            seed: None,
            wave_kind: None,
        },
    );
    for bar in 0..bars {
        session.clips.push(Clip {
            id: format!("clip-{bar}"),
            name: "Noise".into(),
            agent: false,
            track_id: session.tracks[0].id.clone(),
            start_bar: bar as f64,
            length_bars: 1.0,
            data: ClipData::audio("src", 0.0),
        });
    }
    let library = audio::Library::from([("src".into(), buffer)]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("long.flac");
    let options = export::ExportOptions {
        tail_seconds: 0.0,
        ..Default::default()
    };
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let report = export::mix(&session, &library, &path, &options).unwrap();
    let grown = PEAK.load(Ordering::Relaxed) - before;
    assert_eq!(report.frames, bars as u64 * 2 * 48000);
    let raw = report.frames as usize * 2 * 3;
    eprintln!(
        "heap grew {} KiB for {} KiB of audio",
        grown >> 10,
        raw >> 10
    );
    assert!(
        grown < 8 * 1024 * 1024,
        "export grew the heap by {} MiB for {} MiB of audio",
        grown >> 20,
        raw >> 20
    );
    let size = std::fs::metadata(&path).unwrap().len() as usize;
    assert!(size > raw / 20 && size < raw, "{size} bytes for {raw} raw");
}
