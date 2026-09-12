//! Instantiate one plugin from the scan cache, list its parameters, process a
//! second of audio and round-trip its state. Usage:
//! `cargo run --release -p ondera-engine --example probe -- "clap:com.example.id"`
use ondera_engine::{
    host,
    plugin::{NoteEvent, ProcessContext, Rack, MAX_BLOCK},
};

fn main() {
    let id = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: probe <plugin id>   (see `ondera --plugins`)");
        std::process::exit(2);
    });
    let started = std::time::Instant::now();
    let mut instance = host::instantiate(&id, &id, 48000).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let desc = instance.editor.descriptor().clone();
    println!(
        "{} by {} [{}] instrument={} effect={} loaded in {:?}",
        desc.name,
        desc.vendor,
        desc.format.label(),
        desc.instrument,
        desc.effect,
        started.elapsed()
    );
    let params = instance.editor.params().to_vec();
    println!(
        "{} parameters, latency {} samples, gui {}",
        params.len(),
        instance.editor.latency(),
        instance.editor.has_gui()
    );
    for p in params.iter().take(12) {
        let v = instance.editor.value(p.id).unwrap_or(p.default);
        println!(
            "  #{:<6} {:<28} {:>10.4} .. {:<10.4} = {:<10.4} {}",
            p.id,
            p.name,
            p.min,
            p.max,
            v,
            instance.editor.text(p.id, v)
        );
    }
    let mut rack = Rack::new(1);
    rack.mount(0, instance.processor.take().unwrap());
    let ctx = ProcessContext {
        playing: true,
        tempo: 120.0,
        numerator: 4,
        denominator: 4,
        ..Default::default()
    };
    let mut block = [[0.0f32; 2]; MAX_BLOCK];
    let mut energy_in = 0.0f64;
    let mut energy_out = 0.0f64;
    let mut phase = 0.0f64;
    let notes = [NoteEvent {
        frame: 0,
        on: true,
        pitch: 60,
        velocity: 100,
        channel: 0,
    }];
    let t = std::time::Instant::now();
    for i in 0..(48000 / MAX_BLOCK) {
        for f in &mut block {
            let v = if desc.instrument {
                0.0
            } else {
                phase = (phase + 220.0 / 48000.0).fract();
                ((phase * std::f64::consts::TAU).sin() * 0.25) as f32
            };
            *f = [v, v];
            energy_in += (v * v) as f64;
        }
        rack.process(0, &mut block, if i == 0 { &notes } else { &[] }, &ctx);
        for f in &block {
            assert!(f[0].is_finite() && f[1].is_finite(), "non-finite output");
            energy_out += (f[0] * f[0] + f[1] * f[1]) as f64 / 2.0;
        }
    }
    println!(
        "processed 1 s in {:?}: rms in {:.4}, rms out {:.4}",
        t.elapsed(),
        (energy_in / 48000.0).sqrt(),
        (energy_out / 48000.0).sqrt()
    );
    match instance.editor.save() {
        Some(state) => {
            println!("state: {} bytes", state.len());
            match instance.editor.load(&state) {
                Ok(()) => println!("state restored"),
                Err(e) => println!("state restore failed: {e}"),
            }
        }
        None => println!("no state"),
    }
    for p in rack.drain() {
        drop(p);
    }
    drop(instance);
    println!("destroyed cleanly");
}
