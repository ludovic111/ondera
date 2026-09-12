//! Native top-level windows that host external plugin editors. macOS builds
//! create an `NSWindow`; other platforms report that native editors are not
//! wired yet and fall back to the generic parameter panel.

use ondera_engine::plugin::ParentWindow;

#[cfg(target_os = "macos")]
mod imp {
    use super::ParentWindow;
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSBackingStoreType, NSView, NSWindow, NSWindowStyleMask};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

    pub struct NativeWindow {
        window: Retained<NSWindow>,
        content: Retained<NSView>,
    }
    impl NativeWindow {
        pub fn new(title: &str, width: u32, height: u32) -> Result<Self, String> {
            let mtm = MainThreadMarker::new().ok_or("Plugin windows open on the main thread")?;
            let rect = NSRect::new(
                NSPoint::new(120.0, 120.0),
                NSSize::new(width.max(1) as f64, height.max(1) as f64),
            );
            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable;
            // SAFETY: standard AppKit window creation on the main thread; the
            // window is kept alive by `Retained` and never released on close.
            let window = unsafe {
                let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    rect,
                    style,
                    NSBackingStoreType::Buffered,
                    false,
                );
                window.setReleasedWhenClosed(false);
                window
            };
            window.setTitle(&NSString::from_str(title));
            let content = window.contentView().ok_or("Window without content view")?;
            window.center();
            window.makeKeyAndOrderFront(None);
            Ok(Self { window, content })
        }
        pub fn parent(&self) -> ParentWindow {
            ParentWindow::Cocoa(Retained::as_ptr(&self.content) as *mut std::ffi::c_void)
        }
        pub fn set_size(&self, width: u32, height: u32) {
            self.window
                .setContentSize(NSSize::new(width.max(1) as f64, height.max(1) as f64));
        }
        pub fn is_open(&self) -> bool {
            self.window.isVisible()
        }
        pub fn focus(&self) {
            self.window.makeKeyAndOrderFront(None);
        }
        pub fn close(&self) {
            self.window.orderOut(None);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::ParentWindow;
    pub struct NativeWindow;
    impl NativeWindow {
        pub fn new(_title: &str, _width: u32, _height: u32) -> Result<Self, String> {
            Err("Native plugin windows are not available on this platform yet; use the parameter panel.".into())
        }
        pub fn parent(&self) -> ParentWindow {
            ParentWindow::X11(0)
        }
        pub fn set_size(&self, _width: u32, _height: u32) {}
        pub fn is_open(&self) -> bool {
            false
        }
        pub fn focus(&self) {}
        pub fn close(&self) {}
    }
}
pub use imp::NativeWindow;
