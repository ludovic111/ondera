//! Ondera native plugin SDK.
//!
//! An Ondera plugin is a plain Rust type that implements [`Plugin`]: it declares an
//! [`Info`] block, a list of [`ParamSpec`]s, and renders stereo audio in `process`. The
//! same type can be linked statically (Ondera's own stock library does this) or built as a
//! dynamic library that Ondera loads at runtime through the C ABI in [`ffi`]:
//!
//! ```ignore
//! use ondera_plugin::{prelude::*, export_plugins};
//!
//! struct Gain { gain: f32 }
//! impl Plugin for Gain {
//!     const INFO: Info = Info::effect("com.example.gain", "Gain", "Example", "Utility");
//!     fn params() -> Vec<ParamSpec> { vec![param("Gain", -60.0, 12.0, 0.0, "dB")] }
//!     fn new(_rate: f64) -> Self { Self { gain: 1.0 } }
//!     fn set_param(&mut self, index: usize, value: f64) {
//!         if index == 0 { self.gain = db_to_gain(value); }
//!     }
//!     fn process(&mut self, audio: &mut [[f32; 2]], _notes: &[NoteEvent], _ctx: &ProcessContext) {
//!         for frame in audio { frame[0] *= self.gain; frame[1] *= self.gain; }
//!     }
//! }
//! export_plugins!(Gain);
//! ```
//!
//! Build it as a `cdylib`, drop the library into an Ondera plugin directory and rescan.
//! Parameters are document state in Ondera: they undo, save and automate without any work
//! on the plugin side. `process` runs on the audio thread and must not allocate, block or log.
//!
//! A plugin that panics is contained: the host stops calling it, an effect passes audio
//! through, an instrument falls silent, and the session keeps playing. Test a plugin the way
//! the host runs it with [`testing::Bench`].

pub mod dsp;
pub mod ffi;
mod plugin;
pub mod testing;

pub use plugin::*;

/// The ABI version exported by [`export_plugins!`]. Ondera refuses libraries built for
/// another version instead of guessing at their layout.
pub const ABI_VERSION: u32 = 1;
/// Largest block handed to `process`. Hosts split longer buffers.
pub const MAX_BLOCK: usize = 256;
/// The symbol Ondera looks up in a plugin library.
pub const ENTRY_SYMBOL: &str = "ondera_plugin_entry";

/// Everything a plugin usually needs.
pub mod prelude {
    pub use crate::dsp::{coef, db, db_to_gain, Biquad, Delay, Smoother, Svf};
    pub use crate::{
        choice, hz, param, switch, Info, Kind, NoteEvent, ParamChange, ParamSpec, Plugin,
        ProcessContext, MAX_BLOCK,
    };
}

/// Number of type arguments, for the export macros.
#[doc(hidden)]
#[macro_export]
macro_rules! count {
    ($($plugin:ty),* $(,)?) => {
        <[&str]>::len(&[$(stringify!($plugin)),*])
    };
}

/// A static table of vtables for plugins linked into the same binary.
#[macro_export]
macro_rules! plugin_table {
    ($($plugin:ty),+ $(,)?) => {
        [$($crate::ffi::vtable::<$plugin>()),+]
    };
}

/// Export the listed plugin types from a `cdylib` under the symbol Ondera scans for.
#[macro_export]
macro_rules! export_plugins {
    ($($plugin:ty),+ $(,)?) => {
        #[doc(hidden)]
        pub mod __ondera_export {
            use super::*;
            pub static TABLES: [$crate::ffi::PluginVTable; $crate::count!($($plugin),+)] =
                $crate::plugin_table!($($plugin),+);
            pub unsafe extern "C" fn plugin(index: u32) -> *const $crate::ffi::PluginVTable {
                TABLES
                    .get(index as usize)
                    .map_or(::std::ptr::null(), |table| table as *const _)
            }
            pub static ENTRY: $crate::ffi::Entry = $crate::ffi::Entry {
                abi_version: $crate::ABI_VERSION,
                plugin_count: TABLES.len() as u32,
                plugin,
            };
        }
        #[no_mangle]
        pub extern "C" fn ondera_plugin_entry() -> *const $crate::ffi::Entry {
            &__ondera_export::ENTRY
        }
    };
}
