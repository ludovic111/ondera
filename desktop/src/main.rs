#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod chrome;
mod editor;
mod theme;
mod timeline;
mod update;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    update::cleanup();
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut screenshot = None;
    let mut check_updates = std::env::var_os("ONDERA_NO_UPDATE").is_none_or(|v| v.is_empty());
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
            "--screenshot" => {
                screenshot = Some(std::path::PathBuf::from(
                    args.next().ok_or("Missing screenshot path")?,
                ))
            }
            "--update" => {
                match update::check()? {
                    None => println!("Ondera {} is up to date", update::current_version()),
                    Some(release) => {
                        println!("Downloading Ondera {}…", release.version);
                        let target = update::install(&release)?;
                        println!(
                            "Installed Ondera {} at {}",
                            release.version,
                            target.display()
                        );
                    }
                }
                return Ok(());
            }
            "--no-update-check" => check_updates = false,
            "--help" | "-h" => {
                println!("Ondera — native Rust DAW\n  ondera [session.ondera]\n  ondera --validate session.ondera\n  ondera --bounce session.ondera output.wav\n  ondera --screenshot image.png\n  ondera --update            install the latest GitHub release\n  ondera --no-update-check   skip the startup update check (or set ONDERA_NO_UPDATE=1)");
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
        Box::new(move |cc| {
            Ok(Box::new(app::Ondera::new(
                cc,
                path,
                screenshot,
                check_updates,
            )))
        }),
    )?;
    Ok(())
}
