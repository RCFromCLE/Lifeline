//! Versioned game data (PLAN.md §4). So far: GGG's official passive-tree
//! export, the only in-game data GGG publishes outside its APIs.

pub mod game;
pub mod tree;

pub use game::{GameData, REPOE_BASE, REPOE_FILES};
pub use tree::{AscendancyInfo, ClassInfo, Miss, PassiveTree, TreeEdge, TreeNode, TREE_EXPORT_URL};
