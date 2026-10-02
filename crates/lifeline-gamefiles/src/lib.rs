//! Integration with files the Path of Exile 2 client itself reads and writes:
//! the in-game Build Planner, the client log and the production config.
//! Every format here was verified against real files from a 0.5 install
//! (PLAN.md §3).

pub mod build_planner;
pub mod client_log;
pub mod config_ini;
pub mod paths;

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error on {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub(crate) fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}
