//! The few window behaviours GPUI leaves to the app on macOS.

use gpui::Window;

/// Move the window with the pointer from our own title bar (the native one is hidden).
pub fn start_window_drag(window: &mut Window) {
    #[cfg(target_os = "macos")]
    mac::drag(window);
    #[cfg(not(target_os = "macos"))]
    window.start_window_move();
}

#[cfg(target_os = "macos")]
mod mac {
    use gpui::Window;
    use objc2_app_kit::{NSApplication, NSView};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn drag(window: &mut Window) {
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return;
        };
        let Some(mtm) = objc2::MainThreadMarker::new() else {
            return;
        };
        // SAFETY: the view belongs to this live window; AppKit is used on the main thread,
        // with the event AppKit is dispatching right now.
        unsafe {
            let view: &NSView = appkit.ns_view.cast().as_ref();
            let Some(ns_window) = view.window() else {
                return;
            };
            if let Some(event) = NSApplication::sharedApplication(mtm).currentEvent() {
                ns_window.performWindowDragWithEvent(&event);
            }
        }
    }
}
