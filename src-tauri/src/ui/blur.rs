// Request KDE compositor blur behind the window.
// Works on X11 native and XWayland sessions.
// On Wayland-native (non-XWayland) or non-KDE DEs this is a no-op.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub fn request_blur(cc: &eframe::CreationContext<'_>) {
    let wh = match cc.window_handle() {
        Ok(h) => h,
        Err(_) => return,
    };

    let window_id = match wh.as_raw() {
        RawWindowHandle::Xcb(h) => h.window.get(),
        RawWindowHandle::Xlib(h) => h.window as u32,
        _ => return, // Wayland-native or unsupported
    };

    // Set _KDE_NET_WM_BLUR_BEHIND_REGION via xprop.
    // Value "0" means "blur entire window region".
    let _ = std::process::Command::new("xprop")
        .args([
            "-id",
            &window_id.to_string(),
            "-format",
            "_KDE_NET_WM_BLUR_BEHIND_REGION",
            "32c",
            "-set",
            "_KDE_NET_WM_BLUR_BEHIND_REGION",
            "0",
        ])
        .spawn();
}
