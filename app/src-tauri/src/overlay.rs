//! The overlay: a small bar over the game with the next passive to take and a
//! Skills button that opens the skills and supports for where you are. The
//! window hugs what it shows (the page reports its size), so it only covers
//! that bar. It never takes focus, so the game keeps its input, and it only
//! shows while Path of Exile 2 is the window in front. The player can unlock
//! it, drag it anywhere and lock it; it stays where it was left.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, LogicalPosition, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::AppState;

pub const LABEL: &str = "overlay";

/// Starting size; the page resizes the window to its content (`fit`).
const SIZE: (f64, f64) = (360.0, 48.0);
/// Gap from the corner of the game's screen, in logical pixels.
const MARGIN: f64 = 10.0;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Lifeline next passive")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()?;
    place(&window);
    Ok(())
}

/// Unlocks the overlay for dragging, or locks it where it is now and
/// remembers that spot.
pub fn set_locked(app: &AppHandle, locked: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.overlay_unlocked.store(!locked, Ordering::SeqCst);
    if !locked {
        return Ok(());
    }
    let w = app.get_webview_window(LABEL).ok_or("The overlay isn't open.")?;
    let scale = w.scale_factor().map_err(|e| e.to_string())?;
    let p = w.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let mut s = state.settings.lock().unwrap();
    s.overlay_pos = Some((p.x, p.y));
    s.save(&state.data_dir)
}

/// Where the player left it (if that spot is still on a connected screen),
/// else the top-left corner of the game's screen.
fn place(window: &WebviewWindow) {
    let saved = window.state::<AppState>().settings.lock().unwrap().overlay_pos;
    let on_screen = |(x, y): (f64, f64)| {
        window.available_monitors().unwrap_or_default().iter().any(|m| {
            let s = m.scale_factor();
            let (mx, my) = (m.position().x as f64 / s, m.position().y as f64 / s);
            let (mw, mh) = (m.size().width as f64 / s, m.size().height as f64 / s);
            x + 40.0 >= mx && x < mx + mw - 40.0 && y + 10.0 >= my && y < my + mh - 20.0
        })
    };
    match saved.filter(|&p| on_screen(p)) {
        Some((x, y)) => {
            let _ = window.set_position(LogicalPosition::new(x, y));
        }
        None => pin_top_left(window),
    }
}

/// The page's content size (logical pixels): the window takes exactly that.
pub fn fit(app: &AppHandle, width: f64, height: f64) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_size(tauri::LogicalSize::new(width.clamp(40.0, 640.0).ceil(), height.clamp(24.0, 720.0).ceil()));
    }
}

/// Shows the overlay only while the game is in front (and the setting is
/// on); checked a few times a second.
pub fn spawn_visibility(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let Some(w) = app.get_webview_window(LABEL) else { continue };
        let state = app.state::<AppState>();
        let enabled = state.settings.lock().unwrap().overlay;
        // While unlocked it stays up even if the drag briefly takes focus.
        let unlocked = state.overlay_unlocked.load(Ordering::SeqCst);
        let want = enabled && (unlocked || game_in_front());
        if want != w.is_visible().unwrap_or(false) {
            if want {
                // The game may have moved screens, or changed resolution.
                if !unlocked {
                    place(&w);
                }
                let _ = w.show();
            } else {
                let _ = w.hide();
            }
        }
    });
}

/// Moves the overlay to the top-left of the game's screen (inside the
/// taskbar-free work area), so it lands in the same spot at any resolution
/// or Windows display scaling. The primary screen when the game isn't open.
fn pin_top_left(window: &WebviewWindow) {
    let corner = game_screen_corner().or_else(|| {
        let m = window.primary_monitor().ok().flatten()?;
        Some((m.position().x, m.position().y))
    });
    if let Some((x, y)) = corner {
        let margin = (MARGIN * window.scale_factor().unwrap_or(1.0)).round() as i32;
        let _ = window.set_position(PhysicalPosition::new(x + margin, y + margin));
    }
}

/// Whether the foreground window is Path of Exile 2's.
#[cfg(windows)]
fn game_in_front() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowTextW};
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut class = [0u16; 128];
        let n = GetClassNameW(hwnd, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..n.min(class.len())]);
        let mut title = [0u16; 256];
        let n = GetWindowTextW(hwnd, &mut title) as usize;
        let title = String::from_utf16_lossy(&title[..n.min(title.len())]);
        is_game_window(&class, &title)
    }
}

#[cfg(not(windows))]
fn game_in_front() -> bool {
    false
}

/// The game's own window class, or its exact title (a browser tab about the
/// game doesn't count).
fn is_game_window(class: &str, title: &str) -> bool {
    class == "POEWindowClass" || title.trim() == "Path of Exile 2"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_game_window() {
        assert!(is_game_window("POEWindowClass", ""));
        assert!(is_game_window("", "Path of Exile 2"));
        assert!(!is_game_window("Chrome_WidgetWin_1", "Path of Exile 2 build guide - Google Chrome"));
        assert!(!is_game_window("Chrome_WidgetWin_1", "Lifeline"));
    }
}
