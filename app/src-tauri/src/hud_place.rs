//! Where the HUD sits: the top-left corner of the screen the game is on
//! (inside the taskbar-free work area), in that screen's own pixels, so it
//! lands in the same spot at any resolution or Windows display scaling.

use tauri::{PhysicalPosition, WebviewWindow};

/// Gap from the screen's corner, in logical pixels.
const MARGIN: f64 = 8.0;

/// Moves the HUD to the top-left of the game's screen (or the primary
/// screen when the game isn't open).
pub fn pin_top_left(window: &WebviewWindow) {
    if let Some((x, y)) = game_screen_corner().or_else(|| primary_corner(window)) {
        let margin = (MARGIN * window.scale_factor().unwrap_or(1.0)).round() as i32;
        let _ = window.set_position(PhysicalPosition::new(x + margin, y + margin));
    }
}

fn primary_corner(window: &WebviewWindow) -> Option<(i32, i32)> {
    let m = window.primary_monitor().ok().flatten()?;
    Some((m.position().x, m.position().y))
}

/// Top-left of the work area of the monitor showing the game window.
#[cfg(windows)]
fn game_screen_corner() -> Option<(i32, i32)> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    unsafe {
        let game = FindWindowW(w!("POEWindowClass"), PCWSTR::null()).ok()?;
        let monitor = MonitorFromWindow(game, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(monitor, &mut info).as_bool().then_some((info.rcWork.left, info.rcWork.top))
    }
}

#[cfg(not(windows))]
fn game_screen_corner() -> Option<(i32, i32)> {
    None
}
