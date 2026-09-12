//! Native audio and document services. No webview or GUI dependencies.
pub mod audio;
pub mod device;
pub mod document;
pub mod dsp;
pub mod model;
pub mod render;
pub mod store;

pub type Result<T> = std::result::Result<T, String>;
