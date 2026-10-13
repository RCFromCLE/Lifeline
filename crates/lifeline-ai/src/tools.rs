//! Names of the tools the app's MCP server exposes. Agents and the server
//! share this list so a tool can't be granted under a typo'd name. Claude
//! sees each as `mcp__lifeline__<name>`.

use crate::mcp_tool_name;

// The player's character and the build being followed.
pub const CHARACTER_STATE: &str = "character_state";
pub const BUILD_PLAN: &str = "build_plan";

// Versioned game data (tree export, RePoE, wiki).
pub const LOOKUP_GEM: &str = "lookup_gem";
pub const LOOKUP_SUPPORTS_FOR: &str = "lookup_supports_for";
pub const LOOKUP_BASE: &str = "lookup_base";
pub const LOOKUP_UNIQUE: &str = "lookup_unique";
pub const LOOKUP_MOD: &str = "lookup_mod";
pub const LOOKUP_PASSIVE: &str = "lookup_passive";
pub const AREA_INFO: &str = "area_info";
pub const SEARCH_GAME_DATA: &str = "search_game_data";

/// Save a designed build (class, ascendancy, per-stage passives, skills,
/// gear goals): paths computed, gems checked, report back. Only available
/// while the player is creating a build; the result goes to My builds.
pub const DESIGN_BUILD: &str = "design_build";

/// Read-only game-data lookups every agent gets.
pub const GAME_DATA: &[&str] = &[
    LOOKUP_GEM,
    LOOKUP_SUPPORTS_FOR,
    LOOKUP_BASE,
    LOOKUP_UNIQUE,
    LOOKUP_MOD,
    LOOKUP_PASSIVE,
    AREA_INFO,
    SEARCH_GAME_DATA,
];

/// Every tool the server implements.
pub const ALL: &[&str] = &[
    CHARACTER_STATE,
    BUILD_PLAN,
    LOOKUP_GEM,
    LOOKUP_SUPPORTS_FOR,
    LOOKUP_BASE,
    LOOKUP_UNIQUE,
    LOOKUP_MOD,
    LOOKUP_PASSIVE,
    AREA_INFO,
    SEARCH_GAME_DATA,
    DESIGN_BUILD,
];

/// Fully qualified names (`mcp__lifeline__…`) for a set of tools.
pub fn qualified(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| mcp_tool_name(n)).collect()
}
