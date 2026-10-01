//! The only game input the app sends: one Ctrl+Alt+C on a user hotkey press,
//! and only while Path of Exile 2 is the foreground window (PLAN.md §2, §9.4).

use std::time::{Duration, Instant};

#[cfg(windows)]
mod win {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
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

    pub fn modifiers_down() -> bool {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0)
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    pub fn send_ctrl_alt_c() {
        let c = VIRTUAL_KEY(b'C' as u16);
        let inputs = [
            key(VK_CONTROL, false),
            key(VK_MENU, false),
            key(c, false),
            key(c, true),
            key(VK_MENU, true),
            key(VK_CONTROL, true),
        ];
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

/// Copies the item under the cursor/controller selection from PoE2 and
/// returns its text, restoring whatever was on the clipboard before.
#[cfg(windows)]
pub fn copy_hovered_item() -> Result<String, String> {
    if !win::foreground_is_poe() {
        return Err(
            "Path of Exile 2 isn't the active window. Hover an item in game, then press the item-check hotkey.".into(),
        );
    }
    // The hotkey's own modifiers are still held when it fires; wait for the
    // player to release them so the game sees exactly Ctrl+Alt+C.
    let deadline = Instant::now() + Duration::from_millis(1500);
    while win::modifiers_down() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let previous = clipboard.get_text().ok();
    let _ = clipboard.set_text(String::new());
    win::send_ctrl_alt_c();

    let mut item = None;
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(25));
        if let Ok(text) = clipboard.get_text() {
            if text.starts_with("Item Class:") {
                item = Some(text);
                break;
            }
        }
    }
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    item.ok_or_else(|| "No item text came back. Hover (or select with the controller) an item and try again.".into())
}

#[cfg(not(windows))]
pub fn copy_hovered_item() -> Result<String, String> {
    Err("Item copy is only implemented on Windows.".into())
}
