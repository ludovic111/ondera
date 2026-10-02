//! Window captures for `ui.screenshot` and `--screenshot`.

use super::daw::Daw;
use gpui::{App, Entity, Window};

/// Capture the window and hand the image to whoever asked: a live `ui.screenshot` job, or
/// the `--screenshot` run, which then quits.
pub fn capture(window: &mut Window, daw: Entity<Daw>, cx: &mut App) {
    let image = grab(window);
    daw.update(cx, |daw, cx| {
        let app = &mut daw.app;
        match image {
            Ok(image) => {
                if !app.deliver_screenshot(&image) {
                    if let Some(path) = app.screenshot.take() {
                        match image.save(&path) {
                            Ok(()) => app.closing = true,
                            Err(e) => app.error = Some(e.to_string()),
                        }
                    }
                }
            }
            Err(error) => {
                app.screenshot = None;
                app.finish_live(
                    |wait| matches!(wait, crate::control::LiveWait::Screenshot { .. }),
                    Err(error),
                );
            }
        }
        cx.notify();
    });
}

#[cfg(target_os = "macos")]
fn grab(window: &mut Window) -> Result<image::RgbaImage, String> {
    mac::grab(window)
}
#[cfg(not(target_os = "macos"))]
fn grab(_window: &mut Window) -> Result<image::RgbaImage, String> {
    Err("Window capture is available on macOS".into())
}

#[cfg(target_os = "macos")]
mod mac {
    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }
    type CGImageRef = *const std::ffi::c_void;
    type CFDataRef = *const std::ffi::c_void;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        static CGRectNull: CGRect;
        fn CGWindowListCreateImage(
            bounds: CGRect,
            options: u32,
            window: u32,
            image_options: u32,
        ) -> CGImageRef;
        fn CGImageGetWidth(image: CGImageRef) -> usize;
        fn CGImageGetHeight(image: CGImageRef) -> usize;
        fn CGImageGetBytesPerRow(image: CGImageRef) -> usize;
        fn CGImageGetBitsPerPixel(image: CGImageRef) -> usize;
        fn CGImageGetDataProvider(image: CGImageRef) -> *const std::ffi::c_void;
        fn CGDataProviderCopyData(provider: *const std::ffi::c_void) -> CFDataRef;
        fn CGImageRelease(image: CGImageRef);
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDataGetLength(data: CFDataRef) -> isize;
        fn CFDataGetBytePtr(data: CFDataRef) -> *const u8;
        fn CFRelease(object: *const std::ffi::c_void);
    }
    const INCLUDING_WINDOW: u32 = 1 << 3;
    const BOUNDS_IGNORE_FRAMING: u32 = 1 << 0;
    const BEST_RESOLUTION: u32 = 1 << 3;

    pub fn grab(window: &mut Window) -> Result<image::RgbaImage, String> {
        let handle = window.window_handle().map_err(|e| e.to_string())?;
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return Err("Not an AppKit window".into());
        };
        // SAFETY: the handle's NSView belongs to this live window and is only read on the
        // main thread, for its window number.
        let number = unsafe {
            let view: &objc2_app_kit::NSView = appkit.ns_view.cast().as_ref();
            view.window()
                .ok_or("The view has no window")?
                .windowNumber()
        };
        // SAFETY: CoreGraphics calls on a window this process owns; every object created
        // here is released before returning.
        unsafe {
            let image = CGWindowListCreateImage(
                CGRectNull,
                INCLUDING_WINDOW,
                number as u32,
                BOUNDS_IGNORE_FRAMING | BEST_RESOLUTION,
            );
            if image.is_null() {
                return Err("The window could not be captured".into());
            }
            let (width, height) = (CGImageGetWidth(image), CGImageGetHeight(image));
            let stride = CGImageGetBytesPerRow(image);
            let bpp = CGImageGetBitsPerPixel(image);
            let data = CGDataProviderCopyData(CGImageGetDataProvider(image));
            let result = if data.is_null() || bpp != 32 {
                Err("Unexpected capture format".to_string())
            } else {
                let len = CFDataGetLength(data) as usize;
                let bytes = std::slice::from_raw_parts(CFDataGetBytePtr(data), len);
                let mut out = image::RgbaImage::new(width as u32, height as u32);
                for y in 0..height {
                    for x in 0..width {
                        let i = y * stride + x * 4;
                        // Window captures are BGRA, premultiplied; the window is opaque
                        // where it matters, so alpha is kept as is.
                        let (b, g, r, a) = (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]);
                        out.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, a]));
                    }
                }
                Ok(out)
            };
            if !data.is_null() {
                CFRelease(data);
            }
            CGImageRelease(image);
            result
        }
    }
}
