//! Minimal editor for `poe2_production_Config.ini`.
//!
//! The real file starts with a UTF-8 BOM and uses LF line endings; both are
//! preserved, as is every line we don't touch. The client rewrites this file
//! while running, so callers must only save while the game is closed
//! (PLAN.md §8.3).

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

const UI: &str = "UI";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigIni {
    bom: bool,
    newline: &'static str,
    trailing_newline: bool,
    lines: Vec<String>,
}

enum Location {
    Key(usize),
    SectionEnd(usize),
    Missing,
}

impl ConfigIni {
    pub fn parse(text: &str) -> Self {
        let (bom, text) = match text.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        Self {
            bom,
            newline: if text.contains("\r\n") { "\r\n" } else { "\n" },
            trailing_newline: text.ends_with('\n'),
            lines: text.lines().map(str::to_owned).collect(),
        }
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        match self.locate(section, key) {
            Location::Key(i) => self.lines[i].split_once('=').map(|(_, v)| v),
            _ => None,
        }
    }

    /// Replaces the key in place, or appends it to the section (creating the
    /// section at the end of the file if needed).
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let entry = format!("{key}={value}");
        match self.locate(section, key) {
            Location::Key(i) => self.lines[i] = entry,
            Location::SectionEnd(i) => self.lines.insert(i, entry),
            Location::Missing => {
                self.lines.push(format!("[{section}]"));
                self.lines.push(entry);
            }
        }
    }

    fn locate(&self, section: &str, key: &str) -> Location {
        let mut in_section = false;
        let mut end = None;
        for (i, line) in self.lines.iter().enumerate() {
            if let Some(name) = section_name(line) {
                if in_section {
                    break;
                }
                in_section = name == section;
                if in_section {
                    end = Some(i + 1);
                }
                continue;
            }
            if in_section {
                if line.split_once('=').is_some_and(|(k, _)| k == key) {
                    return Location::Key(i);
                }
                if !line.trim().is_empty() {
                    end = Some(i + 1);
                }
            }
        }
        end.map_or(Location::Missing, Location::SectionEnd)
    }

    /// File name of the selected item filter, relative to the My Games folder.
    pub fn item_filter(&self) -> Option<&str> {
        self.get(UI, "item_filter").filter(|v| !v.is_empty())
    }

    pub fn set_item_filter(&mut self, file_name: &str) {
        self.set(UI, "item_filter", file_name);
    }

    /// Which Build Planner file each character is following.
    pub fn active_builds(&self) -> Result<Vec<ActiveBuild>, serde_json::Error> {
        match self.get(UI, "active_builds") {
            Some(v) if !v.trim().is_empty() => serde_json::from_str(v),
            _ => Ok(Vec::new()),
        }
    }

    pub fn set_active_builds(&mut self, builds: &[ActiveBuild]) -> Result<(), serde_json::Error> {
        let json = serde_json::to_string(builds)?;
        self.set(UI, "active_builds", &json);
        Ok(())
    }
}

impl fmt::Display for ConfigIni {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.bom {
            f.write_str("\u{feff}")?;
        }
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                f.write_str(self.newline)?;
            }
            f.write_str(line)?;
        }
        if self.trailing_newline {
            f.write_str(self.newline)?;
        }
        Ok(())
    }
}

fn section_name(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']')
}

/// One entry of `[UI] active_builds`, e.g.
/// `{"character":"Name","path":"file:C:/Users/.../BuildPlanner/Act 3 - X.build"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveBuild {
    pub character: String,
    /// `file:` URL with forward slashes, as the client writes it.
    pub path: String,
}

impl ActiveBuild {
    pub fn for_file(character: &str, file: &Path) -> Self {
        let path = file.to_string_lossy().replace('\\', "/");
        Self {
            character: character.to_owned(),
            path: format!("file:{path}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}[LOGIN]\ngateway_id=America\n\n[UI]\nalways_highlight=true\nitem_filter=NeverSink's filter 2 - 1-REGULAR.filter\nactive_builds=[{\"character\":\"ExileOne\",\"path\":\"file:C:/Users/x/Documents/My Games/Path of Exile 2/BuildPlanner/Act 3 - X.build\"}]\n\n[GENERAL]\ndisable_low_life_warning=false\n";

    #[test]
    fn untouched_file_round_trips_byte_for_byte() {
        assert_eq!(ConfigIni::parse(SAMPLE).to_string(), SAMPLE);
    }

    #[test]
    fn reads_typed_values() {
        let c = ConfigIni::parse(SAMPLE);
        assert_eq!(c.item_filter(), Some("NeverSink's filter 2 - 1-REGULAR.filter"));
        let builds = c.active_builds().unwrap();
        assert_eq!(builds[0].character, "ExileOne");
        assert_eq!(c.get("LOGIN", "always_highlight"), None);
    }

    #[test]
    fn set_replaces_in_place_and_appends_to_section() {
        let mut c = ConfigIni::parse(SAMPLE);
        c.set_item_filter("Lifeline.filter");
        c.set("UI", "new_key", "1");
        c.set("NEWSECTION", "k", "v");
        let out = c.to_string();
        assert!(out.contains("item_filter=Lifeline.filter\n"));
        assert!(out.contains("active_builds=[") && out.contains("\nnew_key=1\n\n[GENERAL]"));
        assert!(out.ends_with("[NEWSECTION]\nk=v\n"));
        assert!(out.starts_with('\u{feff}'));
    }

    #[test]
    fn active_build_paths_use_file_urls() {
        let b = ActiveBuild::for_file("ExileOne", Path::new(r"C:\Users\x\BuildPlanner\Act 1.build"));
        assert_eq!(b.path, "file:C:/Users/x/BuildPlanner/Act 1.build");
    }
}
