//! Path of Building (PoE2) import: build codes and share links, the build
//! XML, and splitting a build's specs into playthrough stages.

pub mod code;
pub mod model;
pub mod stages;

pub use code::{decode, encode, resolve, BuildSource};
pub use model::{parse_xml, Gem, ItemSet, PobBuild, SkillGroup, SkillSet, SlotItem, TreeSpec};
pub use stages::{classify_title, estimate_level, resolve_stage, ActProgress, Stage, StageHint, POE2_0_5_ACTS};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a valid build code: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("build code did not decompress: {0}")]
    Inflate(std::io::Error),
    #[error("build XML is invalid: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("not a Path of Building 2 export (root element <{0}>)")]
    WrongRoot(String),
}
