//! Native audio and document services. No webview or GUI dependencies.
pub mod audio;
pub mod automation;
pub mod control;
pub mod control_automation;
pub mod control_media;
pub mod device;
pub mod document;
pub mod dsp;
pub mod export;
pub mod host;
pub mod midi;
pub mod midi_file;
pub mod model;
pub mod plugin;
pub mod render;
pub mod session_file;
pub mod stock;
pub mod store;

pub type Result<T> = std::result::Result<T, String>;
