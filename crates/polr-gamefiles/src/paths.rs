//! Where the client keeps its files on Windows.

use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "poe2_production_Config.ini";
pub const BUILD_PLANNER_DIR: &str = "BuildPlanner";

/// `Documents/My Games/Path of Exile 2`. Uses the shell's Documents folder so
/// OneDrive-redirected Documents (`C:/Users/<u>/OneDrive/Documents`) work.
/// Item filters (`*.filter`) live directly in this folder.
pub fn user_dir() -> Option<PathBuf> {
    dirs::document_dir().map(|d| d.join("My Games").join("Path of Exile 2"))
}

pub fn build_planner_dir(user_dir: &Path) -> PathBuf {
    user_dir.join(BUILD_PLANNER_DIR)
}

pub fn config_file(user_dir: &Path) -> PathBuf {
    user_dir.join(CONFIG_FILE)
}

/// Default install locations. Steam libraries on other drives are listed in
/// `Steam/steamapps/libraryfolders.vdf` (not parsed yet); the user can always
/// point the app at the log manually.
pub fn client_log_candidates() -> Vec<PathBuf> {
    let Some(pf86) = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from) else {
        return Vec::new();
    };
    vec![
        pf86.join("Steam/steamapps/common/Path of Exile 2/logs/Client.txt"),
        pf86.join("Grinding Gear Games/Path of Exile 2/logs/Client.txt"),
    ]
}

pub fn find_client_log() -> Option<PathBuf> {
    client_log_candidates().into_iter().find(|p| p.is_file())
}
