//! Only one Lifeline at a time: the copy that starts closes every other
//! running Lifeline (an older installed version, a dev build) first, so the
//! newest launch wins and two copies never fight over hotkeys, the HUD,
//! the game log or the tool server.

/// Closes other running `Lifeline.exe` processes and waits briefly for them
/// to exit. Returns how many were closed.
#[cfg(windows)]
pub fn close_other_copies() -> usize {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };

    let me = std::process::id();
    let mut closed = 0;
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return 0 };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if entry.th32ProcessID != me && name.eq_ignore_ascii_case("Lifeline.exe") {
                if let Ok(process) = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, entry.th32ProcessID) {
                    if TerminateProcess(process, 0).is_ok() {
                        // Let it release hotkeys and the tool server's port.
                        WaitForSingleObject(process, 3000);
                        closed += 1;
                    }
                    let _ = CloseHandle(process);
                }
            }
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    closed
}

#[cfg(not(windows))]
pub fn close_other_copies() -> usize {
    0
}
