use ondera_engine::{
    audio::{prepare_sources, Library},
    render::Renderer,
    store,
};
fn main() {
    let session = store::demo();
    let mut library = Library::new();
    prepare_sources(&session, &mut library).unwrap();
    let mut renderer = Renderer::new(session, &library, 48000).unwrap();
    renderer.playing = true;
    let start = std::time::Instant::now();
    let blocks = 48000 * 30 / 128;
    let mut durations = Vec::with_capacity(blocks);
    let mut peak = 0.0_f32;
    for _ in 0..blocks {
        let at = std::time::Instant::now();
        renderer.begin_block();
        for _ in 0..128 {
            let f = std::hint::black_box(renderer.next_frame());
            peak = peak.max(f[0].abs()).max(f[1].abs());
        }
        durations.push(at.elapsed().as_secs_f64());
    }
    let total = start.elapsed().as_secs_f64();
    durations.sort_by(f64::total_cmp);
    println!("30 s demo, 48 kHz, 128 frames: {:.3} s render ({:.1}x realtime), average DSP {:.2}%, p99 block {:.3} ms / 2.667 ms budget, peak {:.4}, voice overflows {}",total,30.0/total,total/30.0*100.0,durations[durations.len()*99/100]*1000.0,peak,renderer.voice_overflows);
}
