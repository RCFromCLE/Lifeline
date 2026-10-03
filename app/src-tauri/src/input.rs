//! The only game input the app sends: the game's own "copy item" shortcut
//! (Ctrl+Alt+C, or Ctrl+C if that brings nothing back) on a user hotkey
//! press, and only while Path of Exile 2 is the foreground window
//! (PLAN.md §2, §9.4).

use std::time::{Duration, Instant};

#[cfg(windows)]
mod win {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowTextW};

    pub fn foreground_is_poe() -> bool {
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut class = [0u16; 128];
            let n = GetClassNameW(hwnd, &mut class) as usize;
            let class = String::from_utf16_lossy(&class[..n.min(class.len())]);
            let mut title = [0u16; 256];
            let n = GetWindowTextW(hwnd, &mut title) as usize;
            let title = String::from_utf16_lossy(&title[..n.min(title.len())]);
            class == "POEWindowClass" || title.starts_with("Path of Exile 2")
        }
    }

    pub fn game_running() -> bool {
        use windows::core::{w, PCWSTR};
        use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
        unsafe {
            FindWindowW(w!("POEWindowClass"), PCWSTR::null())
                .or_else(|_| FindWindowW(PCWSTR::null(), w!("Path of Exile 2")))
                .is_ok_and(|h| !h.is_invalid())
        }
    }

    pub fn foreground() -> super::Foreground {
        use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == std::process::id() {
                super::Foreground::Ours
            } else if foreground_is_poe() {
                super::Foreground::Game
            } else {
                super::Foreground::Other
            }
        }
    }

    pub fn left_button_down() -> bool {
        unsafe { GetAsyncKeyState(windows::Win32::UI::Input::KeyboardAndMouse::VK_LBUTTON.0 as i32) < 0 }
    }

    pub fn modifiers_down() -> bool {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0)
    }

    /// A key event carrying the hardware scan code: games that read raw
    /// input ignore events with only a virtual-key code.
    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) } as u16;
        let mut flags = KEYEVENTF_SCANCODE;
        if up {
            flags |= KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Ctrl+C, or Ctrl+Alt+C with `alt`. Returns how many events Windows took.
    pub fn send_copy(alt: bool) -> u32 {
        let c = VIRTUAL_KEY(b'C' as u16);
        let mut inputs = vec![key(VK_CONTROL, false)];
        if alt {
            inputs.push(key(VK_MENU, false));
        }
        inputs.extend([key(c, false), key(c, true)]);
        if alt {
            inputs.push(key(VK_MENU, true));
        }
        inputs.push(key(VK_CONTROL, true));
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) }
    }

    /// Whether the game runs as administrator (Windows then drops key
    /// presses from normal apps like this one). None if unknown.
    pub fn game_elevated() -> Option<bool> {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
        use windows::Win32::System::Threading::{OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION};
        use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut token = HANDLE::default();
            // An elevated process's token can't be opened from a normal app.
            let result = if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_ok() {
                let mut info = TOKEN_ELEVATION::default();
                let mut len = 0u32;
                let ok = GetTokenInformation(
                    token,
                    TokenElevation,
                    Some(std::ptr::from_mut(&mut info).cast()),
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut len,
                )
                .is_ok();
                let _ = CloseHandle(token);
                ok.then_some(info.TokenIsElevated != 0)
            } else {
                Some(true)
            };
            let _ = CloseHandle(process);
            result
        }
    }
}

/// Which kind of window has focus; the HUD only shows over the game or this app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Foreground {
    Game,
    Ours,
    Other,
}

/// Whether PoE2 has a window open (the HUD never shows without the game).
pub fn game_running() -> bool {
    #[cfg(windows)]
    return win::game_running();
    #[cfg(not(windows))]
    true
}

pub fn foreground() -> Foreground {
    #[cfg(windows)]
    return win::foreground();
    #[cfg(not(windows))]
    Foreground::Game
}

/// Whether Path of Exile 2 is the window in front.
pub fn game_in_front() -> bool {
    #[cfg(windows)]
    return win::foreground_is_poe();
    #[cfg(not(windows))]
    false
}

/// True while the left mouse button is held (keeps the HUD clickable mid-drag).
pub fn left_button_down() -> bool {
    #[cfg(windows)]
    return win::left_button_down();
    #[cfg(not(windows))]
    false
}

/// Until when Lifeline itself is using the clipboard (copying an item and
/// putting the old text back), so Ctrl+C mode ignores those changes.
static OWN_COPY_UNTIL: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

pub fn copying_now() -> bool {
    OWN_COPY_UNTIL.lock().unwrap().is_some_and(|t| Instant::now() < t)
}

fn mark_own_copy(for_ms: u64) {
    *OWN_COPY_UNTIL.lock().unwrap() = Some(Instant::now() + Duration::from_millis(for_ms));
}

/// Whether clipboard text is a copied Path of Exile item.
pub fn is_item_text(text: &str) -> bool {
    text.starts_with("Item Class:") || (text.starts_with("Rarity:") && text.contains("--------"))
}

/// Waits up to `ms` for item text to land on the clipboard.
#[cfg(windows)]
fn wait_for_item(clipboard: &mut arboard::Clipboard, ms: u64) -> Option<String> {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
        if let Ok(text) = clipboard.get_text() {
            if is_item_text(&text) {
                return Some(text);
            }
        }
    }
    None
}

/// Copies the item under the cursor and returns its text, restoring the
/// clipboard. `hotkey` names the key the player pressed (for messages);
/// `log` records each step in debug.log so a failure can be diagnosed.
#[cfg(windows)]
pub fn copy_hovered_item(hotkey: &str, log: &dyn Fn(&str)) -> Result<String, String> {
    if !win::foreground_is_poe() {
        log("copy: game not in front");
        return Err(format!(
            "Path of Exile 2 isn't the active window. In game, point at an item, then press {hotkey}."
        ));
    }
    // The hotkey's own modifiers are still held when it fires; wait for the
    // player to release them so the game sees exactly the copy shortcut.
    let deadline = Instant::now() + Duration::from_millis(2000);
    while win::modifiers_down() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if win::modifiers_down() {
        log("copy: modifiers still held after 2s");
    }
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    mark_own_copy(5000);
    let previous = clipboard.get_text().ok();
    let _ = clipboard.set_text(String::new());
    let sent = win::send_copy(true);
    let mut item = wait_for_item(&mut clipboard, 1500);
    log(&format!("copy: Ctrl+Alt+C sent ({sent} events), item: {}", item.is_some()));
    if item.is_none() {
        let sent = win::send_copy(false);
        item = wait_for_item(&mut clipboard, 1000);
        log(&format!("copy: Ctrl+C sent ({sent} events), item: {}", item.is_some()));
    }
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    // Long enough for Ctrl+C mode's next look at the clipboard.
    mark_own_copy(800);
    if let Some(item) = item {
        return Ok(item);
    }
    let elevated = win::game_elevated();
    log(&format!("copy: nothing came back; game elevated: {elevated:?}"));
    if elevated == Some(true) {
        return Err("Path of Exile 2 is running as administrator, so Windows blocks Lifeline's key press. \
                    Use Record my gear (Play tab), or start the game normally."
            .into());
    }
    Err(format!(
        "No item came back. Open your inventory, point at the item, then press {hotkey}. \
         Or use Record my gear on the Play tab."
    ))
}

#[cfg(not(windows))]
pub fn copy_hovered_item(_hotkey: &str, _log: &dyn Fn(&str)) -> Result<String, String> {
    Err("Item copy is only implemented on Windows.".into())
}
