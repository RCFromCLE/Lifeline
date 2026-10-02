//! Names of the tools the app's MCP server exposes (PLAN.md §6.2). Agents and
//! the server share this list so a tool can't be granted under a typo'd name.
//! Claude sees each as `mcp__lifeline__<name>`.

use crate::mcp_tool_name;

// Live state and the player's plan.
pub const CHARACTER_STATE: &str = "character_state";
pub const BUILD_PLAN: &str = "build_plan";
pub const NEXT_STEPS: &str = "next_steps";
pub const CAMPAIGN_REWARDS: &str = "campaign_rewards";
pub const DEATH_JOURNAL: &str = "death_journal";
pub const LAST_COPIED_ITEM: &str = "last_copied_item";

// Versioned game data (tree export, RePoE, wiki cache).
pub const LOOKUP_GEM: &str = "lookup_gem";
pub const LOOKUP_SUPPORTS_FOR: &str = "lookup_supports_for";
pub const LOOKUP_BASE: &str = "lookup_base";
pub const LOOKUP_UNIQUE: &str = "lookup_unique";
pub const LOOKUP_MOD: &str = "lookup_mod";
pub const LOOKUP_PASSIVE: &str = "lookup_passive";
pub const AREA_INFO: &str = "area_info";
pub const SEARCH_GAME_DATA: &str = "search_game_data";
pub const WIKI: &str = "wiki";

// Checks and market.
pub const VALIDATE_BUILD: &str = "validate_build";
/// Turn a designed build (class, ascendancy, per-stage passives, skills,
/// gear) into the player's build: paths computed, gems checked, report back.
pub const DESIGN_BUILD: &str = "design_build";
pub const PRICE: &str = "price";
pub const TRADE_SEARCH: &str = "trade_search";
pub const TRADE_FIND_STAT: &str = "trade_find_stat";
/// Show rated listings as image cards with ±% badges and Travel buttons.
pub const RATE_LISTINGS: &str = "rate_listings";
/// Items the player recorded as equipped, per slot.
pub const EQUIPPED_ITEMS: &str = "equipped_items";
pub const FILTER_PREVIEW: &str = "filter_preview";

/// The only way any agent can change something outside the conversation:
/// it queues a proposal (write `.build` files, write a filter, open a trade
/// search, travel to a hideout, save a plan change) that the app shows as a
/// confirm card. Nothing runs until the player presses confirm.
pub const PROPOSE_ACTION: &str = "propose_action";

/// Read-only game-data lookups every analytical agent gets.
pub const GAME_DATA: &[&str] = &[
    LOOKUP_GEM,
    LOOKUP_SUPPORTS_FOR,
    LOOKUP_BASE,
    LOOKUP_UNIQUE,
    LOOKUP_MOD,
    LOOKUP_PASSIVE,
    AREA_INFO,
    SEARCH_GAME_DATA,
    WIKI,
];

pub const ALL: &[&str] = &[
    CHARACTER_STATE,
    BUILD_PLAN,
    NEXT_STEPS,
    CAMPAIGN_REWARDS,
    DEATH_JOURNAL,
    LAST_COPIED_ITEM,
    LOOKUP_GEM,
    LOOKUP_SUPPORTS_FOR,
    LOOKUP_BASE,
    LOOKUP_UNIQUE,
    LOOKUP_MOD,
    LOOKUP_PASSIVE,
    AREA_INFO,
    SEARCH_GAME_DATA,
    WIKI,
    VALIDATE_BUILD,
    DESIGN_BUILD,
    PRICE,
    TRADE_SEARCH,
    TRADE_FIND_STAT,
    RATE_LISTINGS,
    EQUIPPED_ITEMS,
    FILTER_PREVIEW,
    PROPOSE_ACTION,
];

/// Tools the app's MCP server implements today; the rest of [`ALL`] arrives
/// with the game-data server (PLAN.md §13 M3).
pub const IMPLEMENTED: &[&str] = &[
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
    TRADE_FIND_STAT,
    TRADE_SEARCH,
    RATE_LISTINGS,
    EQUIPPED_ITEMS,
    PRICE,
    DESIGN_BUILD,
    PROPOSE_ACTION,
];

/// Fully qualified names (`mcp__lifeline__…`) for a set of tools.
pub fn qualified(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| mcp_tool_name(n)).collect()
}
