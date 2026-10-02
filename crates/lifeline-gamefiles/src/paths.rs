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

const GAME_LOG: &str = "Path of Exile 2/logs/Client.txt";

/// Where the log can be: every Steam library (from Steam's
/// `steamapps/libraryfolders.vdf`, so installs on other drives are found),
/// common library folders on each drive, and the standalone client's folders.
pub fn client_log_candidates() -> Vec<PathBuf> {
    let program_files: Vec<PathBuf> = ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect();
    let drives: Vec<PathBuf> = (b'C'..=b'Z').map(|d| PathBuf::from(format!("{}:\\", d as char))).collect();

    let mut libraries: Vec<PathBuf> = Vec::new();
    for steam in program_files.iter().map(|p| p.join("Steam")).chain(drives.iter().map(|d| d.join("Steam"))) {
        if let Ok(vdf) = std::fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
            libraries.extend(steam_library_paths(&vdf));
        }
        libraries.push(steam);
    }
    for d in &drives {
        libraries.push(d.join("SteamLibrary"));
        libraries.push(d.join("Games/SteamLibrary"));
        libraries.push(d.join("Games/Steam"));
    }

    let mut out: Vec<PathBuf> = libraries.iter().map(|l| l.join("steamapps/common").join(GAME_LOG)).collect();
    for root in program_files.iter().chain(drives.iter()) {
        out.push(root.join("Grinding Gear Games").join(GAME_LOG));
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.to_string_lossy().to_lowercase().replace('\\', "/")));
    out
}

/// The `"path"` entries of Steam's `libraryfolders.vdf`.
pub fn steam_library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("\"path\"")?.trim();
            let value = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(PathBuf::from(value.replace("\\\\", "\\")))
        })
        .collect()
}

pub fn find_client_log() -> Option<PathBuf> {
    client_log_candidates().into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_steam_library_paths() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n";
        assert_eq!(
            steam_library_paths(vdf),
            vec![PathBuf::from("C:\\Program Files (x86)\\Steam"), PathBuf::from("D:\\SteamLibrary")]
        );
    }

    #[test]
    fn finds_this_machines_log_when_installed() {
        // Covers other drives too; only asserts when a game is installed here.
        if let Some(p) = find_client_log() {
            assert!(p.ends_with("logs/Client.txt") || p.ends_with("logs\\Client.txt"));
        }
        assert!(client_log_candidates().len() > 20);
    }
}