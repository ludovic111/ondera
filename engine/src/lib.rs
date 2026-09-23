//! Native audio and document services. No webview or GUI dependencies.
pub mod audio;
pub mod automation;
pub mod control;
pub mod control_app;
pub mod control_automation;
pub mod control_controllers;
pub mod control_edit;
pub mod control_media;
pub mod control_plugins;
pub mod controllers;
pub mod device;
pub mod document;
pub mod dsp;
pub mod export;
pub mod flac;
pub mod host;
pub mod midi;
pub mod midi_file;
pub mod model;
pub mod plugin;
pub mod preset;
pub mod recovery;
pub mod render;
pub mod session_file;
pub mod settings;
pub mod stock;
pub mod store;

pub type Result<T> = std::result::Result<T, String>;

pub mod takes;

mod midi_tools;

mod rhythm;
