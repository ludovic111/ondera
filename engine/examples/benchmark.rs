//! Offline render throughput: 30 seconds of the demo at 48 kHz in 128-frame blocks.
use ondera_engine::{audio, render, store};
use std::time::Instant;

fn main() {
    let session = store::demo();
    let mut library = audio::Library::new();
    audio::prepare_sources(&session, &mut library).unwrap();
    let (mut renderer, mut rack) = render::offline(&session, &library, 48000).unwrap();
    renderer.playing = true;
    renderer.locate(0.0);
    let blocks = 30 * 48000 / 128;
    let mut worst = 0.0f64;
    let mut block = [[0.0f32; 2]; 128];
    let started = Instant::now();
    let mut acc = 0.0f64;
    for _ in 0..blocks {
        let t = Instant::now();
        renderer.render(&mut rack, &mut block);
        for f in &block {
            acc += (f[0] + f[1]) as f64;
        }
        worst = worst.max(t.elapsed().as_secs_f64());
    }
    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "Rendered 30 s in {elapsed:.3} s ({:.1}x real time), worst block {:.3} ms of {:.3} ms budget, checksum {acc:.3}, voice overflows {}",
        30.0 / elapsed,
        worst * 1000.0,
        128.0 / 48.0,
        renderer.voice_overflows
    );
}
