#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod chrome;
mod control;
mod editor;
mod native;
mod plugins;
mod theme;
mod timeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut screenshot = None;
    let mut control = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--validate" => {
                let file = args
                    .next()
                    .ok_or("Usage: ondera --validate session.ondera")?;
                let (s, _) = ondera_engine::document::load(std::path::Path::new(&file))?;
                println!(
                    "Valid session: {} ({} tracks, {} clips)",
                    s.name,
                    s.tracks.len(),
                    s.clips.len()
                );
                return Ok(());
            }
            "--bounce" => {
                let file = args
                    .next()
                    .ok_or("Usage: ondera --bounce session.ondera output.wav")?;
                let target = args.next().ok_or("Missing WAV output path")?;
                let (s, library) = ondera_engine::document::load(std::path::Path::new(&file))?;
                ondera_engine::render::bounce(&s, &library, std::path::Path::new(&target), 48000)?;
                println!("Exported {target}");
                return Ok(());
            }
            "--scan-plugin" => {
                // Child process used by the scanner: probe one bundle and print JSON.
                let format = args.next().ok_or("Missing plugin format")?;
                let bundle = args.next().ok_or("Missing plugin path")?;
                let format = ondera_engine::plugin::Format::parse(&format!("{format}:x"))
                    .map(|(f, _)| f)
                    .ok_or("Unknown plugin format")?;
                let result: std::result::Result<Vec<ondera_engine::plugin::Descriptor>, String> =
                    ondera_engine::host::scan::probe(format, std::path::Path::new(&bundle));
                println!("{}", serde_json::to_string(&result)?);
                return Ok(());
            }
            "--scan-plugins" => {
                let cache =
                    ondera_engine::host::scan::scan_all(|path| eprintln!("Scanning {path}"));
                for d in cache.descriptors() {
                    println!(
                        "{:<5} {:<40} {:<24} {}",
                        d.format.label(),
                        d.name,
                        d.vendor,
                        d.id
                    );
                }
                for e in cache.entries.iter().filter(|e| e.error.is_some()) {
                    eprintln!("{}: {}", e.path, e.error.clone().unwrap_or_default());
                }
                return Ok(());
            }
            "--plugins" => {
                for d in ondera_engine::host::scan::installed() {
                    println!(
                        "{:<6} {:<40} {:<24} {}",
                        d.format.label(),
                        d.name,
                        d.vendor,
                        d.id
                    );
                }
                return Ok(());
            }
            "--screenshot" => {
                screenshot = Some(std::path::PathBuf::from(
                    args.next().ok_or("Missing screenshot path")?,
                ))
            }
            "--no-control" => control = false,
            "--help" | "-h" => {
                println!("Ondera — native Rust DAW\n  ondera [session.ondera]\n  ondera --validate session.ondera\n  ondera --bounce session.ondera output.wav\n  ondera --scan-plugins\n  ondera --plugins\n  ondera --screenshot image.png\n  ondera --no-control   disable CLI / MCP connections");
                return Ok(());
            }
            _ => {
                if arg.starts_with('-') {
                    return Err(format!("Unknown option: {arg}").into());
                }
                path = Some(std::path::PathBuf::from(arg));
            }
        }
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 1000.0])
            .with_min_inner_size([1120.0, 760.0])
            .with_app_id("org.ondera.desktop"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Ondera",
        options,
        Box::new(move |cc| Ok(Box::new(app::Ondera::new(cc, path, screenshot, control)))),
    )?;
    Ok(())
}
