//! Reproducible offline callback-cost probe, not a hardware latency certification.
//! `cargo run --release -p ondera-engine --example stress`
use ondera_engine::{
    control::{self, Headless},
    render, Result,
};
use serde_json::{json, Value};
use std::time::Instant;

fn scenario(tracks: usize) -> Result<Value> {
    let mut host = Headless::new();
    let initial: Vec<_> = host
        .store
        .session()
        .tracks
        .iter()
        .map(|track| track.id.clone())
        .collect();
    for id in initial {
        control::call(&mut host, "track.remove", &json!({"trackId": id}), false)?;
    }
    control::call(
        &mut host,
        "transport.setCycle",
        &json!({"enabled":false}),
        false,
    )?;
    control::call(
        &mut host,
        "master.setVolume",
        &json!({"volume":0.25}),
        false,
    )?;
    for index in 0..tracks {
        let track = control::call(
            &mut host,
            "track.add",
            &json!({"kind":"midi", "name":format!("Voice {}",index+1), "instrument":"E-Piano Mk I"}),
            false,
        )?;
        let id = track["id"].as_str().ok_or("Missing track ID")?;
        let notes: Vec<_> = (0..16).flat_map(|beat| (0..4).map(move |voice| json!({"start":beat as f64*0.5,"length":0.45,"pitch":48+(index%12)+voice*4,"velocity":70}))).collect();
        control::call(
            &mut host,
            "clip.create",
            &json!({"trackId":id,"startBar":0,"lengthBars":4,"notes":notes}),
            false,
        )?;
        for (slot, effect) in [(0, "stock:Channel EQ"), (1, "stock:Ondera Comp")] {
            control::call(
                &mut host,
                "strip.setPlugin",
                &json!({"trackId":id,"slot":slot,"pluginId":effect}),
                false,
            )?;
        }
        control::call(
            &mut host,
            "automation.create",
            &json!({"target":"trackVolume","trackId":id,"points":[{"beat":0,"value":0.2},{"beat":4,"value":0.4},{"beat":8,"value":0.1}]}),
            false,
        )?;
        control::call(
            &mut host,
            "track.setPan",
            &json!({"trackId":id,"pan":if index%2==0 {-25} else {25}}),
            false,
        )?;
    }
    let preparation = Instant::now();
    let (mut renderer, mut rack) = render::offline(host.store.session(), &host.library, 48000)?;
    let preparation_ms = preparation.elapsed().as_secs_f64() * 1000.0;
    renderer.playing = true;
    let mut block = [[0.0f32; 2]; 128];
    let blocks = 5 * 48000 / 128;
    let mut times = Vec::with_capacity(blocks);
    let started = Instant::now();
    let mut energy = 0.0;
    for _ in 0..blocks {
        let tick = Instant::now();
        renderer.render(&mut rack, &mut block);
        times.push(tick.elapsed().as_secs_f64() * 1000.0);
        for frame in block {
            if !frame.iter().all(|value| value.is_finite()) {
                return Err("Non-finite output".into());
            }
            energy += frame[0] as f64 * frame[0] as f64 + frame[1] as f64 * frame[1] as f64;
        }
    }
    let elapsed = started.elapsed().as_secs_f64();
    times.sort_by(f64::total_cmp);
    Ok(
        json!({"tracks":tracks,"instruments":tracks,"trackEffects":tracks*2,"automationLanes":tracks,
        "sampleRate":48000,"blockFrames":128,"renderedSeconds":5,"preparationMs":preparation_ms,
        "elapsedSeconds":elapsed,"realtimeFactor":5.0/elapsed,"medianBlockMs":times[blocks/2],
        "p99BlockMs":times[blocks*99/100],"worstBlockMs":times[blocks-1],"blockBudgetMs":128.0/48.0,
        "overBudgetBlocks":times.iter().filter(|value|**value>128.0/48.0).count(),
        "voiceOverflows":renderer.voice_overflows,"rms":(energy/(5.0*48000.0*2.0)).sqrt()}),
    )
}
fn main() -> Result<()> {
    let results: Vec<_> = [4, 8, 16, 32]
        .into_iter()
        .map(scenario)
        .collect::<Result<_>>()?;
    println!("{}",serde_json::to_string_pretty(&json!({"scenario":"Optimized offline processing on this machine, 4-note stock chords per track with two inserts and volume automation. Excludes device drivers and third-party plugin costs.","results":results})).map_err(|error|error.to_string())?);
    Ok(())
}
