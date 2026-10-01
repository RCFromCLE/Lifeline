//! Versioned game data (PLAN.md §4). So far: GGG's official passive-tree
//! export, the only in-game data GGG publishes outside its APIs.

pub mod tree;

pub use tree::{PassiveTree, TreeNode, TREE_EXPORT_URL};
