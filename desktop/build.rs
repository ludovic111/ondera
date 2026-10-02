//! Windows: put the app icon in the executable (Explorer, the taskbar, the installer).
fn main() {
    println!("cargo:rerun-if-changed=icons/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("icons/icon.ico");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=The app icon was not embedded: {error}");
        }
    }
}
