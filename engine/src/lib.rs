//! Native audio and document services. No webview or GUI dependencies.
pub mod audio;
pub mod control;
pub mod device;
pub mod document;
pub mod dsp;
pub mod host;
pub mod midi;
pub mod model;
pub mod plugin;
pub mod render;
pub mod stock;
pub mod store;

pub type Result<T> = std::result::Result<T, String>;
