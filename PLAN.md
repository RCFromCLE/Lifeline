# PathOfLeastResistance — Plan

*An AI-assisted, hardcore-first, controller-first playthrough companion for Path of Exile 2, written in Rust.*

Plan date: **2026-10-01**. Game state at that date: PoE2 Early Access **0.5.5d** (league *Forbidden Rites*, plus *Runes of Aldur*), with **1.0 releasing 2026-12-11**. Every game fact below comes from a cited source, or is marked **(verified locally)** when it was checked against this PC's own PoE2 install. Anything not confirmed is marked **UNVERIFIED** and listed in §12 with how to settle it.

---

## 0. What it is, in one screen

You paste a build link. The app:

1. **Digests the build into stages:** Act 1 → Act 2 → Act 3 → Act 4 → Interludes → Endgame. Each stage gets its passives, skills and supports, gear targets and hardcore safety goals.
2. **Writes those stages into PoE2's own in-game Build Planner** as official `.build` files, so the game highlights your next passives and shows your gems and gear notes. No alt-tabbing.
3. **Generates a matching item filter.** It is your chosen NeverSink strictness plus build- and act-specific highlights that change automatically by area level.
4. **Watches the game log** to know your character, level, zone, act, deaths, quest rewards and every passive you allocate. It then tells you what's next and when you drift from the plan.
5. **Lets you talk or type to Claude.** Ask "does this ring fit?", "which supports for Act 3?", "find me boots with movement speed and cold res". Claude uses your own Claude subscription. Every answer is grounded in current game data via tools.
6. **Shops for you.** Claude searches the trade market, ranks listings for your build and budget, and a single press of **Travel** takes you to the seller's hideout (§7). On controller it can also tell you exactly what to search in the in-game Market.
7. **Can create new builds** for you, validated against real game data and exported to the in-game planner.
8. **Runs a team of specialist AI agents** behind the companion (§6.8): build architect, build auditor, gear appraiser, market scout, hardcore safety officer, route coach, loot-filter smith, fact checker and patch analyst. Background jobs audit imported builds, debrief deaths, analyse new patches and recap sessions without being asked.

**Controller first (PS5 DualSense), mouse/keyboard fully supported.** There is a button combo to summon the overlay, push-to-talk voice, a D-pad-navigable UI and a one-press item check. **Hardcore first:** resistance-penalty tracking, permanent-buff checklist, boss prep and a death journal.

---

## 1. Goals, non-goals, principles

**Goals**
- Get one hardcore character safely from level 1 to endgame with less friction and more fun.
- Use the game's own systems wherever they exist: Build Planner, item filters, in-game Market, log file. Don't reinvent them.
- Never guess about the game. The AI must look facts up, cite them, and say "I couldn't confirm that".

**Non-goals (v1)**
- A damage calculator (Path of Building already is one; we import its output). See S7 in §12 for a possible PoB-headless spike.
- Anything that reads game memory, injects into the client, or automates gameplay.
- Mobile apps. A LAN "second screen" in a phone browser is planned instead (§9.6).

**Principles**
1. **Grounded.** Game facts come from versioned local data (GGG tree export, RePoE, poe2wiki) or a cited web fetch. Data is tagged with patch version.
2. **Inside GGG's hard lines** (§2). The explicit bans — memory reading, client injection, input automation beyond one action per press — are never crossed. Grey areas are enabled; the owner accepted them on 2026-10-01 (§2.1).
3. **Hardcore lens on everything.** Defences before damage; flag one-shot risks.
4. **Controller-first, input-agnostic.** Every action is reachable by pad, keyboard or mouse; the same action map drives all three.
5. **Data-driven campaign.** 1.0 adds Acts 5–6 and removes the Interludes in ten weeks. Acts, areas and rewards live in data, not code.

---

## 2. GGG's rules and how the app complies

Sources: GGG developer docs (pathofexile.com/developer/docs), Terms of Use §7, forum thread 3328601 (POESESSID).

| GGG rule (quoted or summarised) | What the app does |
|---|---|
| "Reading the game's log files is okay as long as the user is aware of what you are doing with that data." | Reads `Client.txt` and shows a plain-language "what we read" screen on first run. Chat lines are not parsed unless you opt in. |
| Build Planner and item filters: "Some features of the game can use files that can be created manually or by tools." | Writes `.build` and `.filter` files only into their documented folders, atomically, and never deletes yours. |
| Macros must be "invoked manually by the user (automated invocations such as … timers, reacting to file changes, or from reading the screen are not allowed)", have "one set function", and perform "only … one action that interacts with the game". | Every game input comes from **one press of one combo** and does **one thing**: Ctrl+Alt+C to copy an item, or one chat command (`/hideout`, `/itemfilter <name>`). Nothing is ever fired from log events, timers or screen reading. |
| "Executable apps that interact with the game or game files … will result in immediate account termination." ToS 7(b): don't "modify or adapt … the game client or its data". | **Hard line:** no memory reading, no injection, no client patching. Config-file edits are a grey area (§2.1). |
| "It is against our Terms of Use (section 7i) to reverse-engineer endpoints outside of this documentation." | Grey area, accepted (§2.1). trade2 search, fetch and hideout travel run inside the official site's own logged-in session, at the site's rate limits (§7). |
| POESESSID "gives the recipient almost complete access to your Path of Exile account"; "The secure way of granting tools access to your data is via OAuth." | The app never reads or stores the cookie. You log in to pathofexile.com inside the app's trade window; the session stays in that window's WebView2 profile (§7). OAuth (character import, filter upload) is designed in, but **GGG is "currently unable to process new applications"**. |
| Required notice: "This product isn't affiliated with or endorsed by Grinding Gear Games in any way." | Shown in About and on first run. |
| Overlays are not explicitly addressed. | Overlay windows are separate, non-injecting OS windows, the same approach as Exiled Exchange 2 and Exile-UI. Display only. |

### 2.1 Decisions taken (2026-10-01)

- **Grey areas are on.** The owner: "any grey areas you want I am not worried about".
  - In-app trade search, ranking and **Travel to Hideout** (§7).
  - Config-file edits while the game is closed, with a backup (§8.3).
  - Screenshot/OCR item reading as a controller fallback (§9.4).
  - poe.ninja economy data fetched directly, cached.
- **Hard lines stay hard.** These are explicit bans, not grey areas, and they end accounts:
  - no memory reading or injection
  - no input fired by anything but a press
  - never more than one game action per press
- **League** is asked during first-run setup from the live league list. The default is **HC Forbidden Rites** (§3.5).
- **Voice** runs on the GPU by default (RTX 5090), with CPU fallback and a setting to force either (§6.5).

**Anthropic's rule for the Claude side** (code.claude.com Agent SDK docs): "Unless previously approved, Anthropic does not allow third party developers to offer claude.ai login or rate limits for their products." The app never logs into Claude or handles Claude credentials. It launches **your own installed Claude Code CLI, which you logged into yourself**, for your personal use. If the app is ever distributed to other people, it must switch to the API-key backend (§6.7) or get Anthropic's approval.

---

## 3. Ground truth: game integration points

### 3.1 In-game Build Planner (`.build`) — the core output

**Official spec:** pathofexile.com/developer/docs/game, "Build Object (Version 1 – Experimental)". Added in 0.5.0 ("Added support for Build Guides"). The 0.5.3 `link` field shows a button in game, but only for whitelisted domains.

- **Folder:** `Documents\My Games\Path of Exile 2\BuildPlanner\*.build`. **Verified locally:** on this PC Documents is redirected to `C:\Users\rudyc\OneDrive\Documents`, so the app resolves the Known Folder rather than `%USERPROFILE%\Documents`.
- **Root fields:** `name`, optional `author`, `link`, `description`, `ascendancy` (e.g. `Huntress2`), `passives[]`, `skills[]`, `inventory_slots[]`.
- **Passives:** a string id, or `{id, level_interval?, weapon_set? (0–2), additional_text?}`. Ids are `PassiveSkills` ids such as `strength89`.
- **Skills:** `{id: "Metadata/Items/Gems/SkillGem…", level_interval?, additional_text?, support_skills?[]}`. Supports have the same shape.
- **Inventory slots:** `{inventory_id ("Weapon1"…), slot_x?, slot_y?, level_interval?, unique_name?, additional_text?}`.
- **Level intervals:** `level_interval` is `[min,max]` or a single uint. The meaning of the single uint is **UNVERIFIED**.
- **Markup in `additional_text`:** `<tag>{text}`. Tags are `r b i u s m l`, colour names, or `rgb(r,g,b)`.
- **"Editing or creating builds within Path Of Exile 2 is currently not supported."** The game only imports, so tools like this one are the intended authors. Meta gems: the docs say unsupported, but 0.5.3 fixed meta-gem `.build` files (conflict; test).
- **Verified locally:** nine Mobalytics-exported files are already on this PC. They are named "Act 1", "Act 2", "Act 3", "Act 3 SWAP", "Act 4", "Interludes", "Endgame", "Red Maps" and "Zoo" (a Spirit Walker guide). This is exactly the per-stage shape we generate.
  - Observed details: names are cut to 40 characters, `weapon_set` values are 1 and 2, both `Metadata/Items/Gems/` and `Metadata/Items/Gem/` id prefixes appear, and some passive ids repeat (meaning unknown).
- **Verified locally:** the client logs `[BuildPlanner] Successfully loaded build 'file:C:/…/X.build'` for each file, which gives us a free validation signal. `poe2_production_Config.ini [UI] active_builds=[{"character":…,"path":"file:…"}]` records which build each character follows.
- **Also available:** builds can be uploaded and subscribed to on pathofexile2.com (`/my-account/builds`), with auto-update on PC and console. There is no public API, so v1 writes local files only.
- **Implemented:** `crates/polr-gamefiles/src/build_planner.rs`. It reads both the string and object forms, round-trips lossless JSON, writes atomically, and provides a markup helper.

### 3.2 Client log (`Client.txt`) — knowing where you are

Path: `<install>\logs\Client.txt`. **Verified locally (Steam):** `C:\Program Files (x86)\Steam\steamapps\common\Path of Exile 2\logs\Client.txt` (13 MB, 111k lines). The standalone path `C:\Program Files (x86)\Grinding Gear Games\Path of Exile 2\logs\Client.txt` comes from EE2/Sidekick (not verified here). The robust approach is to find the running game process and use its directory.

Line prefix: `YYYY/MM/DD HH:MM:SS <ms> <hex> [LEVEL Client <pid>] <body>`. Every event below was **verified locally**:

| Event | Body | Use |
|---|---|---|
| Area generated | `Generating level 15 area "G1_town" with seed 1` | Area id → act (`G1_…`–`G4_…`), and the monster level, which drives resistance penalties |
| Scene | `[SCENE] Set Source [Clearfell Encampment]` (ignore `(unknown)`/`(null)`) | Display name |
| Level up | `: Name (Monk) is now level 12` | Level, class/ascendancy |
| Death | `: Name has been slain.` (also logged for party members) | Hardcore death journal |
| Passive (un)allocated | `Successfully allocated passive skill id: AscendancyWarrior3Notable4, name: Coal Stoker` | **Same ids as `.build`**, so plan-vs-actual diff happens live |
| Quest points | `: You have received 2 Passive Skill Points.` / `… 2 Weapon Set Passive Skill Points.` | Quest-reward tracking |
| Permanent buff | `: Name has received 30% increased [Charm] Charges gained.` / `has lost …` | Ticks off the campaign-buff checklist automatically |
| Planner load | `[BuildPlanner] Successfully loaded build '…'` | Confirms the game accepted our file |
| Other | `has joined/left the area`, `Trade accepted.`, `AFK mode is now ON/OFF` | Context |

- **Area id → name → level map:** built from this PC's own log for Acts 1–4 (e.g. `G1_1` The Riverbank 1 … `G4_town` Kingsmarch 53), plus hideouts (`Hideout*`, level 65) and trials (`G2_13`/`Sanctum_*` Trial of the Sekhemas, `G3_10` Trial of Chaos, `G4_4_3` Trial of the Ancestors).
- **Interlude area ids:** not in this log; **UNVERIFIED** (derive from RePoE `world_areas`).
- **Also observed:** "You have entered …" (PoE1) does not occur. `output_all_dialogue_to_chat=true` puts NPC dialogue in the log, which is a possible future boss-encounter signal.
- **Implemented:** `crates/polr-gamefiles/src/client_log.rs` — parser, a polling tailer that survives truncation, and `campaign_act()`.

### 3.3 Game config (`poe2_production_Config.ini`)

- **File format, verified locally:** UTF-8 **BOM**, LF line endings, INI sections.
- **Keys we read:**
  - `[UI] item_filter` / `item_filter_loaded_successfully` (filter selection plus a "did it parse" signal)
  - `[UI] active_builds`
  - `[GENERAL] user_input_mode=gamepad` (verified locally: you play in controller mode)
  - `[ACTION_KEYS]`: Windows virtual-key codes, e.g. `open_market_panel=191` is `/`
  - `[DISPLAY] borderless_windowed_fullscreen=true` (needed for overlays; verified locally)
- **Write policy:** §8.3.
- **Implemented:** `crates/polr-gamefiles/src/config_ini.rs`, a byte-preserving editor (BOM, line endings, untouched lines).

### 3.4 Item filters

- **Location:** `.filter` files go directly in `Documents\My Games\Path of Exile 2\` (not `OnlineFilters\`).
- **Verified locally:** NeverSink 0.10.4 (all 7 strictnesses) is installed and selected (`1-REGULAR`).
- **Syntax:** GGG's shared doc is pathofexile.com/item-filter/about; PoE2-only entries are ribboned. Proven-loadable in PoE2 (from NeverSink 0.10.4 plus `item_filter_loaded_successfully`):
  - Conditions: `Class, BaseType, Rarity, AreaLevel, ItemLevel, Quality, Sockets, StackSize, GemLevel, WaystoneTier, UnidentifiedItemTier, Corrupted, TwiceCorrupted, Mirrored, Identified, HasExplicitMod, AnyEnchantment, HasVaalUniqueMod, IsVaalUnique, BaseArmour/BaseEvasion/BaseEnergyShield, Width, Height, AlwaysShow`
  - Actions: `SetFontSize, SetTextColor, SetBorderColor, SetBackgroundColor, PlayAlertSound, PlayEffect, MinimapIcon, DisableDropSound`
  - Block keyword: `Continue`
  - `Import "x.filter" [Optional]` was added to PoE2 in 0.3.0.
- **Item classes (exact strings):**
  - Weapons and off-hands: Bows, Bucklers, Crossbows, Foci, One Hand Maces, Quarterstaves, Quivers, Sceptres, Shields, Spears, Staves, Talismans, Two Hand Maces, Wands.
  - Armour and jewellery: Amulets, Belts, Body Armours, Boots, Gloves, Helmets, Rings.
  - Flasks, charms and jewels: Life Flasks, Mana Flasks, Charms, Jewels.
  - Currency and gems: Stackable Currency, Augment (runes / soul cores), Skill Gems, Support Gems, Omen.
  - Endgame and misc: Waystones, Map Fragments, Tablet, Pinnacle Keys, Vault Keys, Incubators, Expedition Logbook, Quest Items, Instance Local Items, Fishing Rods.
  - Uncut gems are matched by `BaseType "Uncut Skill Gem"` etc.
- **NeverSink licence:** MIT. He explicitly welcomes apps built on it, with a reference and the credits section kept (forum 3693141). Section `[[0100]] OVERRIDE AREA 1 - Override ALL rules here` is the insertion point.
- **Reload behaviour:** **UNVERIFIED** (`/itemfilter <name>` exists; hot-reload unknown).

### 3.5 Game state that drives the plan (0.5.5d)

**Leagues** (from `/api/trade2/data/leagues`): `Forbidden Rites`, `HC Forbidden Rites`, `Runes of Aldur`, `HC Runes of Aldur`, `Standard`, `Hardcore`. SSF strings are **UNVERIFIED**. A dead HC character moves to its softcore parent league.

**League setup:** first-run setup asks which league you play, from that live list. The default is **HC Forbidden Rites**, the owner's league. Hardcore features stay on unless a softcore league is picked. The league selects trade and price data, and is re-asked when a new league starts or when the log shows a character the app hasn't seen.

**Campaign** (poe2wiki Act pages; RePoE world_areas). Cruel was removed in 0.3.0.

| Stage | Town | Area levels | Final boss |
|---|---|---|---|
| Act 1 | Clearfell Encampment | 1–15 | Count Geonor |
| Act 2 | The Ardura Caravan | 16–31 | Jamanra, the Abomination |
| Act 3 | Ziggurat Encampment | 33–45 | Doryani, Royal Thaumaturge |
| Act 4 | Kingsmarch | 46–53 | Tavakai, the Chieftain |
| Interludes 1–3 (any order, scale +4 per completed) | The Refuge / Khari Bazaar / The Glade | 54–64 | Wulfric & Elswyth / Azmadi / Zelina & Zolin |
| Endgame | Ziggurat Refuge | 65+ (Waystone T1–T16 = 65–80) | Pinnacles: Arbiter of Ash, Arbiter of Divinity, Raven Trickster, Xesht, Bodach, Aberration, Vessel of Kulemak, Atziri |

**Resistance penalty** (poe2wiki Resistance; since 0.3.0 it follows area level after Act 4). The default resistance cap is 75%.

| Stage | Penalty |
|---|---|
| Act 2 | −10% |
| Act 3 | −20% |
| Act 4 | −30% |
| Area level 54–59 | −40% |
| Area level 60–64 | −50% |
| Area level 65+ | −60% |

**Passive points:** 99 from levels plus 24 from quests (+2 each, which can also be used as weapon-set points). **Ascendancy:** 4 trials × 2 points. Quest-point timeline from PoB2 `QuestRewards.lua` (pinned commit `2450f2f`):

| After | Quest area level | Cumulative quest points |
|---|---|---|
| Act 1 (Hunting Grounds, Ogham Farmlands) | 12 | 4 |
| Act 2 (Keth, Deshar) | 28 | 8 |
| Act 3 (Jungle Ruins, Aggorat) | 44 | 12 |
| Act 4 (Isle of Kin, Trial of the Ancestors) | 51 | 16 |
| Interludes (Wolvenhold, Khari Bazaar, Howling Caves) | 64 | 22 |
| Epilogue (Kingsmarch) | 62 | 24 |

The Act 4 sources disagree between poe2wiki pages; PoB data is used as the reference.

**Permanent campaign buffs** (poe2wiki Quest reward, Maxroll "permanent stats from campaign" updated 2026-09-18). This becomes the auto-ticking checklist (§10.2).

| Stage | Buffs |
|---|---|
| Act 1 | Beira +10% cold res; Candlemass +20 life; King in the Mists +30 Spirit |
| Act 2 | Sisters of Garukhan +10% lightning res; Valley of the Titans +1 charm slot plus a choice |
| Act 3 | Blackjaw +10% fire res; Ignagduk +30 Spirit; Venom Crypts choice (stun / ailment threshold / mana regen) |
| Act 4 | Halls of the Dead tattoos (3 × resistance or attribute); Kaimana defence choice; Navali; Goddess of Justice flask choice |
| Interludes | Molten One +5% life; Seven Pillars choice; Lythara +40 Spirit |

**Uncut gems by area level** (poe2wiki Uncut gem pages):

| Area level | Skill/spirit gem level | Support tier |
|---|---|---|
| 1 | 1 | — |
| 4 | 2 | 1 |
| 11 | 4 | 1 |
| 16 | 5 | 2 |
| 33 | 9 | 3 |
| 45 | 11 | 4 |
| 55 | 13 | 5 |
| 66 | 14–16 | 5 |
| 80 | 17–20 | 5 |

Other gem rules:
- Supports: up to 5 sockets. Copies of a support are allowed since 0.3.0, but not two from the same category on one skill. Lineage supports are one per character.

**Defences, for hardcore** (poe2wiki):
- Energy shield is hit first and recharges after 4 s.
- Armour works against physical hits only.
- Evasion fails against red-flash attacks; Deflection does not.
- Stun threshold equals maximum life; ailment threshold is half of maximum life.
- The dodge roll has no invulnerability frames.
- **No death log or recap exists in game**, which is why the app keeps its own (§10.4).

**Trade:**
- Asynchronous trade has existed since 0.3.0: Merchant's Tabs, "Instant Buyout" listings and a gold fee.
- On the website, **Travel to Hideout** needs you to be in game, in the same league, past Act 4, and not SSF.
- In game, the **Market panel** opens with `/`; "Secure Item" teleports you to the seller. On controller, **holding Triangle on an item fills the Market search** (0.5.0).

---

## 4. Game data: sources and pipeline

| Need | Primary source | Notes |
|---|---|---|
| Passive tree (nodes, ids, edges, positions, ascendancies) | **GGG `grindinggear/poe2-skilltree-export`** (`data.json`, 5153 nodes, commit "0.5.5") | GGG's own data, and the only in-game data GGG officially exports. String `id` matches log and `.build`; integer key matches PoB `nodes`. |
| Gems, bases, mods, uniques, item classes, world areas, ascendancies | **RePoE poe2** (`repoe-fork.github.io/poe2/`, `version.txt` = `4.5.5.2`) | Keys match `.build` ids (`Metadata/Items/Gems/…`). Licence says content is GGG's; use per their terms. Check `version.txt` every patch. |
| Text, acquisition, quest rewards, boss notes | **poe2wiki Cargo API** (`/api.php?action=cargoquery`, ≤500 rows per call) | CC BY-NC-SA 3.0; attribute it. The HTML site is behind Anubis/Cloudflare, but `api.php` works. |
| Human cross-check, links | **poe2db.tw** | No API or terms; link to it and use it as a source to cite. Don't depend on its internal JSON. |
| Older tree versions for imported builds | PoB2 `src/TreeData/0_x/tree.json` (MIT code; GGG data) | Needed for `0_1`–`0_4` builds. |
| Prices | Official currency-exchange history `GET https://web.poecdn.com/api/currency-exchange/poe2/{unix_hour}` (public, hourly); poe2scout API; poe.ninja **economy** endpoints | poe.ninja asks desktop apps to proxy through a backend. Personal use: low-frequency, cached, honest User-Agent. The poe.ninja builds API is off-limits. |

**Pipeline (`polr-data`):** download → validate → normalise → a **SQLite DB per patch version** (rusqlite `bundled`, FTS5 for fuzzy search) → the AI tools query it.
- It ships no data in the repo; it fetches on first run and on patch change.
- The patch version comes from the log line `[HTTP2] User agent: PoE poe2_production/tags/4.5.x` (EE2 fixture shows `4.4.0j` = 0.4.0j). Compare it with RePoE `version.txt`.

---

## 5. Build ingestion and stage digestion

### 5.1 Inputs

| Input | How | Status |
|---|---|---|
| PoB code (pasted) | base64url → zlib → `<PathOfBuilding2>` XML | **Implemented** (`polr-pob`), tested on two real codes (0.2 padded and 0.5 unpadded) |
| pobb.in, Maxroll PoB, poe2db PoB, pastebin, rentry, `pob2://`, YouTube redirects | URL mapping copied from PoB2's `BuildSiteTools.lua` | **Implemented** (`code::resolve`) |
| Maxroll planner `maxroll.gg/poe2/planner/<id>` | `planners.maxroll.gg/profiles/poe2/<id>` JSON. It has explicit "Act N" skill steps with `minLevel` and an ordered passive `history`. | Planned; undocumented endpoint, so experimental |
| `.build` files (Mobalytics, Maxroll, PoB2 export) | Read the BuildPlanner folder or a dropped file | **Implemented** (reader) |
| poe.ninja / Mobalytics links | Not fetched: poe.ninja says its builds/PoB endpoints are "not available for third-party use"; Mobalytics is Cloudflare-blocked | The app explains the alternative (their `.build`/PoB export) |
| Character import from your GGG account | OAuth `GET /character/poe2/{name}` | Blocked: GGG registration closed |

### 5.2 Canonical model (`polr-model`, planned)

`Build { meta, class, ascendancy, stages: Vec<StagePlan> }`. Each `StagePlan` has:
- the stage (`Act(n) | Interludes | Endgame` for now, data-driven for 1.0)
- a level range
- the passive set, with weapon sets and ascendancy
- skills and supports with level intervals
- gear targets per slot (base, mod priorities, notes)
- hardcore goals (resistance cap target for the stage's penalty, life/ES floor, ailment answers)
- author notes

### 5.3 Splitting into stages

Implemented in `polr-pob::stages`; tests use the real point counts.

1. **Title classification.** Patterns: `Act N`, `Interlude`, `Early/Starting Maps`, `Level N`, `Leveling N`, `Endgame/Maps/Main Tree`.
   - **Era-aware:** in `0_1`/`0_2` trees "Act 4–6" mean Cruel. In `0_3`–`0_5` guides, "Act 5" means the Interludes (Maxroll's convention).
   - Real titles seen: "Act 1"…"Act 6", "Early Maps", "Endgame w/ Diamonds" (0.2 Stormweaver), and "Leveling 1 – Bows"…"Leveling 7 – Spears", "Starting Maps/Random Gear", "Level 75 Endgame Setup", "Main Tree 93" (0.5 Deadeye).
2. **Level estimate.** A port of PoB2's `EstimatePlayerProgress` (verified against Build.lua). It returns level 14/28/40/48/61/64 for 17/33/47/59/76/87 main-tree points.
   - **Needs tree data** to count main-tree points (excluding class/ascendancy starts and ascendancy nodes). That arrives with `polr-data`.
3. **Resolve.** Explicit act or interlude labels win. Otherwise use "Level N"; otherwise the estimated level, mapped through the area-level bands (§3.5). Same-size alternatives ("Endgame w/ Diamonds") become **variants**, not stages.
4. **Pair skill and item sets.**
   - Apply PoB's loadout rules (identical titles, `{id}` suffixes, single set applies to all).
   - Then fuzzy matching: "Act N"/"Leveling N" → "Leveling"; "Endgame*" → "Endgame".
   - Split one skill set on empty "header" groups such as `--Act 1-2 Bows--`.
5. **Single endgame tree only.** Grow a connected allocation order from the class start toward high-value targets. Claude may propose the priority, but every prefix must remain a legal connected tree (validated). Then cut it at the per-stage point budgets (Act 1 ≈ 17–19, Act 2 ≈ 36, Act 3 ≈ 53–55, Act 4 ≈ 67, Interludes ≈ 80–88, endgame ≤ 123). Ascendancy points per stage: 0 / 2 / 4 / 4 / 4–6 / 8.
6. **Emit.** One `.build` per stage ("Act 1 – <build>" … "Endgame – <build>"). Set `level_interval` so skills and gear appear at the right level. Hardcore notes go in `additional_text` with markup, e.g. `<red>{Cap cold res before Act 3: −20% penalty}`.

---

## 6. The AI companion (Claude, via your subscription)

### 6.1 How it connects

```
Tauri app (Rust)
 ├─ MCP server (rmcp 3.5, streamable HTTP on 127.0.0.1:<random>, bearer token per launch)
 │    tools: game data · your build · live character state · prices · actions (confirm-gated)
 └─ spawns per turn:  claude -p "<message>" --output-format stream-json --verbose
                      --include-partial-messages --permission-mode dontAsk --restricted
                      --tools "WebSearch,WebFetch,Agent" --system-prompt <companion.md>
                      --agents <agents.json> --allowedTools "<our tools + web + Agent>"
                      --disallowedTools "Agent(Explore),Agent(Plan),…"   (Claude Code's built-in agents)
                      --mcp-config <file> --strict-mcp-config
                      [--model sonnet|opus] [--resume <session_id>]
```

**Why this shape:**
- No Rust Agent SDK exists, and the docs recommend driving `claude -p` from other languages.
- Claude Code uses the subscription login you created with `/login`; the app never sees it.
- `--bare` is **not** used: it disables OAuth/subscription auth.
- `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` are removed from the child process's environment, so an API key on your system can't silently take precedence over your plan.

**Verified live on 2026-10-01** (Claude Code 2.1.286, your Max login; `cargo run -p polr-ai --example smoke`):
- **Subscription auth works under `--restricted`.** That mode drops code-running tools, ignores user/project/local settings files, and confines file tools to the app's directory.
- **`--strict-mcp-config` must be passed even with no config.** Without it the session also loaded your claude.ai connectors (Gmail, Drive, Calendar, Claude Docs). With it, the session sees only the app's MCP server.
- **The agents file loads.** All 9 specialists appear in `init.agents`.
- **The built-in agents are blocked.** A live attempt to launch `Explore` returned DENIED. The `init` event reports the delegation tool as `Task`; `Agent` is accepted on the command line.
- **`--json-schema` works**, but only if the session has the `StructuredOutput` tool. The validated object arrives as `result.structured_output`.
- **`--max-turns` is not listed** by `claude --help` in this version, so it's off by default; each agent sets its own `maxTurns`.

**Streaming:**
- `stream_event` text deltas go to the chat UI through Tauri channels.
- `system/init` reports MCP status, tools and agents.
- `rate_limit_event` reports your subscription usage windows (§6.6).
- `system/api_retry` reports retries.
- `result` carries the session id, usage, cost and structured output.

**Prompt delivery:** the prompt is passed right after `-p`; list-valued flags like `--allowedTools` would otherwise swallow it.

**Implemented** (`crates/polr-ai`, 18 tests plus the live smoke example):
- command builder and stream-json parser with turn runner
- MCP config writer and the canonical tool-name registry (`tools.rs`)
- specialist roster (`agents.rs`) and background jobs (`jobs.rs`)
- prompts in `prompts/`

**Later optimisation:** keep a long-lived process with `--input-format stream-json`. The flag exists, but its input line shape is undocumented, so per-turn processes plus `--resume` come first.

### 6.2 Tools Claude gets (MCP, `mcp__polr__*`)

**Read-only:**
- `character_state`: name, class, level, zone, area level, act, deaths, quest points, buffs received, session time.
- `build_plan(stage?)`, `next_steps()` (plan vs reality: missing passives, unspent points, gems due, resistance gap vs the coming penalty), `campaign_rewards(stage)`.
- Game data: `lookup_gem`, `lookup_support_for(skill)` (tag-compatible supports available at this level), `lookup_base`, `lookup_unique`, `lookup_mod`, `lookup_passive`, `area_info`, `search_game_data`.
- `wiki(title|cargo query)`: cached, cited.
- `last_copied_item()`: parsed Ctrl+Alt+C text.
- `price(item|currency)`: cached sources with timestamps.

**Validation:**
- `validate_build`: ids exist in the current patch, the tree is connected, stage point budgets hold, supports are compatible, attribute requirements are met.

**Market and journal:**
- `trade_search`: runs in the logged-in trade session (§7).
- `death_journal`, `filter_preview`.

**Actions** (one tool, `propose_action`): Claude only *proposes*. The app shows a confirm card, and nothing happens until you press confirm. Kinds: write planner builds, write item filter, open trade search, **travel to hideout**, save plan change, apply config change (game closed).

The canonical names live in `polr-ai/src/tools.rs`, shared by the agents and the future MCP server; a test fails if an agent is granted a tool the server doesn't define.

### 6.3 Grounding contract

- The system prompt (`companion.md`) requires a tool lookup before any game fact, a source tag on facts, and "couldn't confirm" over guessing.
- Tools return `source` and `patch` fields so citations are mechanical.
- WebSearch/WebFetch are allowed as a fallback. Preferred domains are poe2db.tw, poe2wiki.net and pathofexile.com.

### 6.4 Conversations it should nail

| You say | It does |
|---|---|
| "Does this fit?" (after the item-check combo) | Parses the item, compares it with the stage's slot target and current gear (if known), and judges defences first ("+38 life, +22% cold, but you'd drop 18% fire below cap for Act 3's −20%"). |
| "Which supports for Act 3?" | Lists tag-compatible supports reachable at your level and uncut tier (area 33 → tier 3), ranked for your skill, with the hardcore trade-offs. |
| "Find me boots, 25% move speed, cold res, under 3 ex" | market-scout searches, ranks the top listings for your build, and you press **Travel** on the one you want (§7). |
| "I died to Jamanra, why?" | Uses the death journal entry (zone, level, time, your resistances vs the penalty) plus boss notes from the wiki, and asks what hit you. There is no combat log, and it says so. |
| "Make me a build" | Asks for class, playstyle and budget, drafts a build from validated data, explains each act's plan and hardcore safety plan, then exports it to the in-game planner on your approval. |

### 6.5 Voice

- **Push-to-talk:** hold the summon combo.
- **Speech-to-text:** local, `whisper-rs` 0.16 (base.en/small.en on CPU, large-v3-turbo on GPU). `sherpa-onnx` 1.13 is the alternative. Windows' own speech API needs MSIX packaging, so it's out.
- **Text-to-speech:** the `tts` crate (Windows OneCore voices); Piper/Kokoro are optional.
- **Default device:** GPU (the RTX 5090 has ample headroom at 4K DX12), with automatic CPU fallback and a setting to force either.

### 6.6 Models and limits

- **Model: Claude Opus 5.5 everywhere** — the companion, all nine specialists and every background job (owner's decision, 2026-10-01). It is pinned as `claude-opus-5-5` (`polr-ai::agents::MODEL`) rather than the `opus` alias, so a future release can't change behaviour silently. Verified live on your Max plan.
- **Subscription limits, verified live.** Every turn emits a `rate_limit_event` with your plan's windows, e.g. `five_hour` 9% and `seven_day` 3%, each with a reset time. Usage is shared with claude.ai.
  - The HUD shows both meters.
  - Background jobs pause above a configurable threshold (default 80% of the five-hour window), so chat always has headroom.
  - `api_retry` and 429 events are surfaced, not hidden.

### 6.7 API-key backend (only if distributed)

Same tool registry, driven directly over raw HTTP to the Messages API (no official Rust SDK). It streams, uses `claude-opus-5-5` by default, keeps keys in the OS keyring (`keyring` 4.2), and uses server-side fallbacks per current API guidance. Not built until needed.

### 6.8 Specialist agents and background jobs

The companion is the only agent you talk to. Behind it is a team of specialists defined in `polr-ai/src/agents.rs` and `prompts/agents/*.md`, handed to Claude Code as `--agents agents.json`. The format and fields (`description`, `prompt`, `tools`, `model`, `effort`, `maxTurns`, `omitClaudeMd`) come from Claude Code's subagent reference.

All specialists share a preamble with the grounding rules, the hardcore lens, and "never change anything yourself". Each gets **only the tools its job needs**.

| Agent | Model | Can use | Job |
|---|---|---|---|
| **build-architect** | Opus 5.5 (high effort) | game data, plan, `validate_build`, web, `propose_action`, can consult fact-checker and hc-safety-officer | Designs and reworks staged builds; must validate, get a safety review and fact-check before handing back |
| **build-auditor** | Opus 5.5 (high) | game data, plan, `validate_build`, rewards, can consult the two checkers | Stage-by-stage audit of imported or changed builds, most dangerous first |
| **gear-appraiser** | Opus 5.5 | copied item, state, plan, base/mod/unique lookups, price | "Does this fit?" → equip / keep / sell, hardcore order |
| **market-scout** | Opus 5.5 | plan, state, `trade_search`, price, lookups, `propose_action` | Precise searches, value ranking for this character, queues travel |
| **hc-safety-officer** | Opus 5.5 (high) | game data, state, plan, rewards, death journal, web | What kills you next; death debriefs |
| **route-coach** | Opus 5.5 | state, next steps, rewards, area info, plan | One to three next steps, overlay-sized |
| **loot-filter-smith** | Opus 5.5 | plan, state, base lookup, `filter_preview`, `propose_action` | Build- and stage-aware filter rules on NeverSink |
| **fact-checker** | Opus 5.5 (high) | game data, web — **no actions** | Adversarial CONFIRMED / CONTRADICTED / UNVERIFIED per claim |
| **patch-analyst** | Opus 5.5 | game data, plan, web | What a new patch changes for *your* build |

**Guardrails enforced in code (each has a test):**
- Only the companion, build-architect and build-auditor can delegate.
- The fact-checker can't act.
- Every granted MCP tool exists in the shared registry.
- Claude Code's own coding agents are denied.

**Background jobs** (`jobs.rs`) run a specialist as the main session (`--agent`), with no saved session history and a JSON report schema. The result is `summary` plus `findings[{severity danger|warning|info, stage, title, detail, source}]`, shown as cards. Triggers come from game events. None of them sends anything to the game.

| Job | Agent | Trigger |
|---|---|---|
| Patch watch | patch-analyst | The log's client version (`poe2_production/tags/4.x`) is newer than the game data |
| Build audit | build-auditor | A build is imported or a stage plan changes |
| Death debrief | hc-safety-officer | `has been slain` for the tracked character |
| Session recap | route-coach | Game closed, or a long AFK after play |

Jobs respect the usage-window threshold (§6.6). They are individually toggleable.

**Verified live:** route-coach ran as a job and returned a schema-valid report, parsed into `JobReport`.

---

## 7. Market and hideout travel

This is the flow you asked for: ask the AI for an item, it searches, you get teleported. It's a grey-area feature, accepted in §2.1.

**The trade session window**
- A Tauri webview window on `https://www.pathofexile.com/trade2`. You log in to your GGG account there **once**.
- The session cookie stays in that window's WebView2 profile. The app never reads, copies or stores it, the same as using the site in a browser.
- The window can stay hidden. It's shown only for login or when you want the full site.

**Flow**
1. You ask, by voice or text: "boots with 25% move speed and cold res under 3 ex".
2. **market-scout** turns that into a trade2 query. The query includes item class or base, stat filters (stat ids from the site's own `/api/trade2/data/stats`, cached daily), your league, a max price, and status **Instant Buyout** (`securable`) so travel works.
3. `trade_search` runs **inside the trade window**, as same-origin `fetch` calls to `/api/trade2/search/poe2/{league}` and `/api/trade2/fetch/…`. It uses the site's headers and obeys `X-Rate-Limit-*` (anonymous search limit observed: `5:10:60, 15:60:300, …`).
4. market-scout ranks the listings by value *for this character*: gaps closed per currency spent, and no new gaps opened. You get the top few on screen, read aloud if you're on voice.
5. You pick one, then press **Travel** (✕ on the pad, or click). The app makes **one** `POST /api/trade2/whisper` with the listing's `hideout_token`, inside the trade window, the same request the site's own button sends. The game takes you to the seller's hideout, where you buy from Ange and pay the gold fee.
   - One press, one action, so it's inside the macro rules.
   - Requirements: in game, same league, past Act 4, not SSF.
   - A 404 means the item already sold; the app says so and offers the next listing.
6. **"In-person" listings** (no instant buyout): the app copies the site's whisper text. One press sends it as one chat message (one action).

**Without the session** (not logged in, or the site is down): the app opens the official search page instead. The page URL carries the query, reportedly base64url(gzip(query)) — **spike S4**. You press Travel on the site.

**Controller in-game path:** the AI can also tell you exactly what to filter in the in-game **Market** (`/`), or to **hold Triangle** on your own item to prefill a search. "Secure Item" teleports you there too.

**Prices:** official currency-exchange hourly digests, poe.ninja economy endpoints (direct, cached with ETag, honest User-Agent) and poe2scout. All are attributed and timestamped.

**Spike S11:** confirm how a Tauri 2 window on a remote origin hands results back to Rust (`eval` plus a capability scoped to `https://www.pathofexile.com/*`, or a custom-protocol callback), and that the site accepts the in-page `fetch` with its normal headers.

---

## 8. Item filters and in-game customisation

### 8.1 Filter generator (`polr-filter`)

- **Base:** your chosen NeverSink strictness, from your installed copy or the GitHub release (MIT).
- **Injection point:** a generated block is inserted at `[[0100]] OVERRIDE AREA 1`. The output is `PathOfLeastResistance – <build>.filter` in the filters folder. NeverSink's files are never edited, and the MIT notice and credits are preserved.
- **One file covers every act** by using `AreaLevel` windows (Act 1 ≤15, Act 2 16–31, Act 3 33–45, Act 4 46–53, Interludes 54–64, endgame 65+). There is no mid-session switching.
- **Switching filters:** when a new filter is written, the app offers a one-press `/itemfilter <name>` (one chat command per press). It never sends it automatically after the file write, which would be "reacting to file changes".

**Build rules generated from data:**
- Your weapon class(es) and their bases, with a beam and minimap icon.
- Armour of your defence type via `BaseArmour/BaseEvasion/BaseEnergyShield` (the type follows the attribute requirement: Str→AR, Dex→EV, Int→ES, and hybrids).
- `Uncut Skill/Support/Spirit Gem`.
- Artificer's Orb and Lesser/Greater Jeweller's Orb.
- Flask tier upgrades by area level.
- **Hardcore charms** loud once droppable: Thawing (12), Stone (8), Antidote (24), Dousing/Grounding (32).
- High `UnidentifiedItemTier` rares while levelling.

**Guardrails:**
- Never hide rares during the campaign on Very Strict or above (NeverSink's own warning).
- Uniques fall through to NeverSink's tiering.

**Validation:** after writing, the app checks `item_filter_loaded_successfully` in the config the next time you select the filter.

**Later:** upload to your account via OAuth `account:item_filter` (realm `poe2`, `validate=true`), so it also works on console. That waits on GGG OAuth registration.

### 8.2 In-game Build Planner sync

- Writes the per-stage `.build` files (§5.3).
- When the log shows you entering a new act, it **suggests** the next stage file to select in game.
- With the game closed, it can also set `[UI] active_builds` for your character to the right stage directly, using the config policy in §8.3.

### 8.3 Settings coach — and the config-file policy

- **The coach:** a checklist of recommended options, shown with your current values read from the config.
  - Hardcore: enemy mini life bars on, low-life warning not disabled, `pulse_player_life_when_low`, resistance icons and flask buffs shown, pause enabled.
  - Loot: Key Pickup per NeverSink's FAQ.
  - Controller: dedicated no-sprint dodge roll (0.5.0).
- **Applying them (grey area, accepted §2.1).** Press "Apply" on the coach and the app writes `poe2_production_Config.ini`, under these conditions:
  - **only while PoE2 isn't running** (checked by process; the game rewrites the file itself)
  - **a timestamped backup first**
  - a byte-preserving editor (BOM, line endings and untouched lines kept; `polr-gamefiles::config_ini`, tested)
  - a diff shown before writing
  - Controller users can still apply each setting in game (Options) with step-by-step guidance instead.
- The same mechanism sets the selected filter (`item_filter`) and the active Build Planner stage (`active_builds`) between sessions.

### 8.4 Chat commands on a press

The useful ones: `/hideout`, `/itemfilter <name>`, `/deaths`, `/played`, `/remaining`, `/dnd`, `/kills`, `/whois` (poe2wiki Chat, 0.5).
- They can be bound to app combos. **One press sends one command** (macro rules).
- PoE2 must be in the foreground, and the controller-mode behaviour of keyboard chat input is part of spike S1.

---

## 9. Controller-first experience (PS5 DualSense)

### 9.1 Your setup (verified locally)

- Config: `user_input_mode=gamepad`, `auto_input_method_switching=true`, borderless windowed 3840×2160 DX12.
- DualSense haptic and adaptive-trigger settings exist in `[Gamepad_UI]`. Whether PoE2 natively drives them on PC is **UNVERIFIED**.

### 9.2 Reading the pad in the background

- **Library:** SDL3 (`sdl3` 0.20, SDL 3.4) on its own thread, gamepad subsystem only.
- **Hints:**
  - `SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1`, so the pad is read while PoE2 has focus.
  - `SDL_HINT_JOYSTICK_ENHANCED_REPORTS=auto`. The default switches a Bluetooth DualSense to enhanced reports, which **breaks DirectInput for other apps until the controller is power-cycled**.
  - `SDL_HINT_JOYSTICK_HIDAPI_PS5_PLAYER_LED=0`.
- **No writes to the pad** (rumble, LED, triggers) while PoE2 runs.
- **Why not the alternatives:** gilrs (WGI) needs a focused window and its XInput backend can't see a DualSense. WebView2's Gamepad API is focus-bound and buggy with the Steam overlay; it's used only as a fallback when the app is focused.

### 9.3 Combos and modes

- **Summon combo:** user-configurable via a "press your combo" screen.
  - Candidates are **Create** and **Mute** (Mute is readable over USB or in enhanced mode). PoE2's use of both is **UNVERIFIED** (spike S3).
  - Avoid the PS button (Steam chords) and the touchpad (PoE2 radial menu / map).
- **HUD mode:** a transparent, always-on-top, **non-focusable** overlay (`focusable(false)` → `WS_EX_NOACTIVATE`, click-through). Display only, so PoE2 keeps every input. It shows the stage card, next passives, resistance vs upcoming penalty and voice status.
- **Interactive mode:** the combo brings the app forward. D-pad/left stick move focus, ✕ confirms, ◯ goes back (◯ at the top level returns to PoE2), L1/R1 switch tabs, △ opens context actions, Options opens the app menu, the right stick scrolls.
  - **Spike S2:** check that PoE2 ignores pad input while unfocused. If it doesn't, interactive mode is limited to town/hideout with a warning.
- **Keyboard/mouse parity:** a global shortcut (`tauri-plugin-global-shortcut` 2.4) and the same action map (arrows/Tab/Enter/Esc).

### 9.4 Item check on controller

1. Highlight an item with the controller cursor.
2. Press the item combo. The app sends **one** Ctrl+Alt+C via `SendInput` and only does so while PoE2 is in the foreground. One manual action, allowed.
3. The app reads the clipboard (timeout about 500 ms), restores your clipboard, and Claude answers.
- Advanced copy works in controller mode since 0.4.0e.
- **Spike S1:** check that synthetic keyboard input doesn't flip `auto_input_method_switching` into keyboard/mouse UI.
- **Fallback (on, grey area accepted §2.1):** a screenshot crop of the tooltip under the cursor, read by Windows OCR. If OCR is unsure, Claude vision reads it instead; gear-appraiser can read an image file. It's used when the clipboard copy fails, for example in panels where copy doesn't work. It never triggers inputs.

### 9.5 UI rules

- Big focus rings and spatial navigation: `@noriginmedia/norigin-spatial-navigation-core` 4.1 (framework-agnostic) or a small Rust implementation.
- PlayStation glyphs: Kenney Input Prompts, CC0. The glyph set is auto-detected from SDL's gamepad type, overridable.
- Hold-to-confirm for anything that writes files or opens trade.

### 9.6 Couch mode (opt-in)

- Serves the UI to a phone or tablet browser on your LAN (axum + WebSocket).
- Security: QR-code pairing with a one-time token, bound to the chosen private adapter only, Origin/Host checks.
- The phone can't send anything to the game.

### 9.7 Setup checklist shown on first run

- Steam Input **off** for PoE2, with the DualSense on **USB**. If the Bluetooth L2/R2 bug appears, turn Steam Input on.
- Disable Steam's Desktop Layout so it doesn't double-input in our app.
- No exclusive-mode remappers (DS4Windows/HidHide).
- Don't launch our app through Steam (WebView2 + Steam overlay gamepad bug).

---

## 10. Hardcore suite

1. **Resistance runway.**
   - The log gives the area level, and the app knows the next penalty step (§3.5).
   - **Before** a transition (entering Act 2, area level 54/60/65) the HUD shows "−10% more next zone: fire 71 → 61". Current resistances come from your copied items, your plan, or what you tell it. The game exposes no live stats, and the app says so.
2. **Permanent-buff checklist** per act. It auto-ticks from `has received` log lines, and nags before you leave an act with buffs missing (e.g. Candlemass +20 life, +30 Spirit skulls).
3. **Boss prep cards.** Cards for each act boss and pinnacle: mechanics summary with a wiki citation, damage types, recommended resistance and flask/charm answers. Shown when the log puts you in the boss zone.
4. **Death journal.**
   - On `has been slain`, it records character, zone, area level, level, time and session context, then offers a short "what hit you?" voice/text debrief.
   - Claude turns that into one concrete defensive change.
   - It also tracks near-death context such as Trial of Chaos runs (a death there is a real hardcore death).
5. **Readiness score per stage.** A checklist, not a fake DPS number: resistances capped for the coming penalty, life/ES floor, a stun or ailment answer, flask/charm coverage, Spirit used, gear tier. Its inputs are the plan, your gear and your answers.
6. **Safety nudges:** remind that Esc pauses solo play and that "Respawn at checkpoint" forfeits a boss fight instead of dying.

---

## 11. Tech stack and architecture

**Versions** were checked on crates.io on 2026-10-01. Tauri 3.0 is in alpha, so stay on 2.x.

| Concern | Choice |
|---|---|
| Shell | **Tauri 2.12.x** (WebView2 present: 154.0) |
| UI | **Leptos 0.8** (CSR via Trunk) — all-Rust front-end |
| Passive tree view | Canvas/WebGL from GGG's tree export |
| Plugins | global-shortcut 2.4, clipboard-manager 2.4, opener 2.7, store 2.5, updater 2.13, window-state 2.5, single-instance 2.5, notification 2.5, log 2.10 |
| Async / HTTP | tokio 1.53 · reqwest 0.13 |
| MCP server | **rmcp 3.5** (`transport-streamable-http-server`) on axum 0.8 |
| Data | rusqlite 0.40 `bundled` (FTS5) · serde/serde_json · roxmltree 0.21 · flate2 1 · base64 0.22 |
| Windows | windows 0.62 (`SetWinEventHook` to follow the game window, `SendInput`, foreground checks) · arboard 3.6 |
| Controller | **sdl3 0.20** |
| Voice | whisper-rs 0.16 (or sherpa-onnx 1.13) · tts 0.26 · cpal 0.18 |
| Misc | regex 1 · thiserror 2 · tracing · dirs 6 · keyring 4.2 (API-key mode only) |

**Workspace**

```
crates/
  polr-gamefiles   ✅ .build files, Client.txt events + tailer, config ini editor        (16 tests)
  polr-pob         ✅ PoB codes, share links, XML model, stage hints, PoB level estimator (14 tests)
  polr-ai          ✅ Claude Code bridge, stream-json (incl. usage windows), tool registry,
                      9 specialist agents, background jobs, prompts                     (18 tests + live smoke)
  polr-data        ⬜ game-data ingest → per-patch SQLite (tree export, RePoE, wiki cache)
  polr-model       ⬜ canonical Build/StagePlan, converters (PoB, .build, Maxroll), validator
  polr-mcp         ⬜ rmcp tool server implementing polr-ai::tools over data/model/gamefiles + live state
  polr-filter      ⬜ NeverSink-based filter generator
  polr-trade       ⬜ price sources, trade session window bridge (search/fetch/travel), URL fallback
  polr-input       ⬜ SDL3 pad thread, action map, combos, one-shot Ctrl+Alt+C
  polr-voice       ⬜ push-to-talk STT, TTS
  polr-hc          ⬜ penalties, buff checklist, boss cards, death journal, readiness
app/               ⬜ Tauri + Leptos (main window, HUD overlay, couch mode)
```

**Runtime flow:** log tailer and pad thread → event bus (tokio broadcast) → state store → UI (Tauri events) and MCP tools. Claude turns run as child processes. File writes go through one "actions" service that requires a UI confirmation token.

---

## 12. Open questions and spikes (verify before building on them)

| # | Question | How to settle |
|---|---|---|
| S1 | Does a synthetic Ctrl+Alt+C copy the controller-selected item without flipping PoE2 to keyboard/mouse UI (`auto_input_method_switching=true`)? | Manual test in town with the DualSense |
| S2 | Does PoE2 ignore pad input while unfocused (needed for interactive mode)? | Manual test |
| S3 | Are Create/Mute unused by PoE2 on PC? | Press-test plus bind screen |
| S4 | Is the trade2 search URL id really base64url(gzip(query))? | Build one locally, open it |
| S5 | `.build`: single-uint `level_interval` semantics; `weapon_set` 0 vs 1/2; meta gems; `inventory_id` for charms (`Charm1` vs `Flask1` slot_x) | Write test files, check `[BuildPlanner] Successfully loaded` and the in-game view |
| ~~S6~~ | ~~Subscription auth with isolation flags~~ | **Settled 2026-10-01:** works with `--restricted` + `--strict-mcp-config` (live smoke test) |
| S7 | Can PoB2's Lua run headless (via `mlua`/LuaJIT) for real defence numbers? | Look for a headless wrapper in PoB2; prototype |
| S8 | Filter reload: is there hot-reload, a refresh button, or only `/itemfilter`? Does an `Import` wrapper around NeverSink work? | Manual test |
| S9 | Interlude area ids and 1.0 Acts 5–6 data | RePoE `world_areas`; re-check at 1.0 |
| S10 | Exact SSF / HC SSF league strings | Trade/ladder data when you choose a league |
| S11 | Trade session window: Rust ↔ remote-origin page bridge in Tauri 2 (`eval` + scoped capability or custom protocol); site accepts in-page `fetch` for search, fetch and whisper | Prototype in M5 with your login |
| S12 | Built-in agent deny: the live test returned DENIED, but `permission_denials` was empty. Confirm with a tool-using companion turn once the MCP server exists. | Re-run the smoke test in M3 |
| S13 | Does the gear-appraiser read screenshots well enough for the OCR fallback (image passed as a file with `--add-dir`)? | Test in M4 |

Known data conflicts to keep visible:
- Act 4 quest-point sources (poe2wiki pages disagree; PoB data used).
- Deflection's 40% value is community-sourced.
- poe2db tree JSON paths are internal; don't rely on them.

---

## 13. Roadmap

Each milestone ends with something you can use in a real playthrough.

| Milestone | Delivers | Done when |
|---|---|---|
| **M0 Foundations** ✅ *(2026-10-01)* | Plan; `polr-gamefiles`, `polr-pob`, `polr-ai` (incl. agents and jobs) with tests from real files; toolchain installed; git repo | **Done:** 48 tests pass, clippy clean, live smoke test against your Claude Max login passes |
| **M1 Game link** 🟡 *(app running 2026-10-01: main window, HUD overlay, global hotkeys for item check / what next / ask / overlay, live log card, PoB import → staged in-game planner files via GGG tree data, Opus 5.5 chat with usage meters; UI is plain HTML/JS for now — Leptos later)* | Tauri shell; log tailer → live character card (level, act, zone, area level, deaths, quest points, buffs); reads your BuildPlanner files; first-run setup (league picker, default HC Forbidden Rites; "what we read" screen) | Playing Act 1 updates the card live |
| **M2 Build → stages → in-game** | `polr-data` (tree export + RePoE); PoB, link and `.build` import; stage split; writes per-stage `.build` files; passive drift alerts | A pobb.in link becomes Act 1…Endgame files the game logs as loaded |
| **M3 Companion + agents** | `polr-mcp` implementing the tool registry; chat UI with streaming and usage meters; the 9 specialists live; background jobs (build audit, death debrief, session recap, patch watch) as finding cards; item check (keyboard/mouse first) | "Does this fit?" answered with cited data via gear-appraiser; importing a build produces an audit |
| **M4 Controller & voice** | SDL3 pad thread, HUD overlay, interactive mode, controller item check (plus OCR fallback), push-to-talk STT (GPU), TTS | Full loop without touching the keyboard |
| **M5 Market** 🟡 *(2026-10-01: via chat — trade2 search/fetch with rate limiting, stat lookup, poe.ninja prices, in-app MCP tool server verified live with Claude Code, market-scout verified live on HC Forbidden Rites, Travel cards + logged-in trade window; sell = price advice)* | Trade session window; market-scout search and ranking; one-press **Travel to Hideout**; prices; in-game Market guidance | "Find me boots…" → ranked listings → one press → you're in the seller's hideout |
| **M6 Filters & settings coach** | NeverSink-based per-build filter; settings checklist | Filter loads (`item_filter_loaded_successfully`) and highlights your bases per act |
| **M7 Hardcore suite** | Resistance runway, buff checklist, boss cards, death journal, readiness | Warns before each penalty step; journals deaths |
| **M8 Build creator** | Claude-designed builds, validated and exported | A new build passes validation and loads in game |
| **M9 1.0 (Dec 11, 2026)** | Acts 5–6, Interludes removed, new tree/gem data, Duelist | Re-run M2 tests on 1.0 data |

Order rationale: M1–M2 give value with no AI at all. M3 builds on solid data. Controller (M4) comes before market and filters because it's how you play.

---

## 14. What I need from you

**Done (2026-10-01):**
- **Installed:** Rust 1.99 (stable-msvc), VS 2022 Build Tools (C++), and Claude Code 2.1.286, which is already logged in to your Max plan and added to your user PATH.
- **Decisions recorded in §2.1:** grey areas on, league asked with HC Forbidden Rites as the default, GPU voice.
- **Git:** repo initialised with your personal identity (`rudycorradetti4@gmail.com`, repo-local).

**Still needed:**
1. **GitHub login for the private repo.** Run `gh auth login` once (browser), then I'll create `PathOfLeastResistance` as a private repo and push.
2. **Folder rename:** close VS Code, then `Rename-Item "$env:USERPROFILE\Downloads\PathofChangeMeOnceNameIsDecided" PathOfLeastResistance`.
3. **Trade or SSF?** I'm assuming trade, since market features need it. Say if it's SSF.
4. **Later:** if GGG reopens OAuth registration, apply for character import and filter upload.

---

## 15. Sources (all fetched or checked 2026-10-01)

**GGG**
- Developer docs: https://www.pathofexile.com/developer/docs (index, /game, /reference, /authorization, /data, /changelog)
- Item filter reference: https://www.pathofexile.com/item-filter/about
- Terms of Use: https://www.pathofexile.com/legal/terms-of-use-and-privacy-policy
- Patch notes: 0.5.0 https://www.pathofexile.com/forum/view-thread/3932540 · 0.5.5 …/3999858 · list …/view-forum/2212
- Async Trade FAQ: …/view-thread/3828185
- POESESSID warning: …/view-thread/3328601
- Build guides on the website: …/view-thread/3972505
- Official tree export: https://github.com/grindinggear/poe2-skilltree-export
- Trade data endpoints: https://www.pathofexile.com/api/trade2/data/leagues (and /stats, /static, /filters)
- Currency exchange history: https://web.poecdn.com/api/currency-exchange/poe2

**Community data**
- poe2wiki API: https://www.poe2wiki.net/api.php (Cargo tables, Act/Interlude/Resistance/Quest reward/Uncut gem pages)
- RePoE: https://repoe-fork.github.io/poe2/
- poe2db: https://poe2db.tw/us/
- poe.ninja API policy: https://poe.ninja/docs/api
- poe2scout: https://api.poe2scout.com/openapi/v1.json
- Maxroll permanent stats: https://maxroll.gg/poe2/getting-started/permanent-stats-from-campaign
- Maxroll in-game planner guide: https://maxroll.gg/poe2/getting-started/how-to-use-the-in-game-build-planner

**Path of Building PoE2** (pinned `2450f2fc6ff5d35ace9b35bcd9d31e648ae9d7f1`): `src/Classes/ImportTab.lua`, `src/Modules/BuildSiteTools.lua`, `src/Modules/Build.lua` (acts table, `EstimatePlayerProgress`), `src/Data/QuestRewards.lua`, `src/Classes/PassiveSpec.lua`, `src/Modules/BuildExportPoE2.lua`, `src/TreeData/0_5/tree.json`

**Prior-art tools**
- Exiled Exchange 2: https://github.com/Kvan7/Exiled-Exchange-2 (log regexes, clipboard flow, trade limiter)
- Sidekick: https://github.com/Sidekick-Poe/Sidekick
- Exile-UI: https://github.com/Lailloken/Exile-UI
- qutm (Tauri 2 PoE2 overlay): https://github.com/Vyary/qutm
- NeverSink PoE2 filter (MIT): https://github.com/NeverSinkDev/NeverSink-Filter-for-PoE2 and forum thread 3693141
- poe2-build-forge (`.build` fixtures): https://github.com/chesler410/poe2-build-forge

**Controller**
- SDL3 hints: https://wiki.libsdl.org/SDL3/SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS and …/SDL_HINT_JOYSTICK_ENHANCED_REPORTS
- gilrs focus note: https://docs.rs/gilrs/latest/gilrs/
- Controller advanced copy (0.4.0e): https://www.pathofexile.com/forum/view-thread/3913283
- Bluetooth trigger bug: …/view-thread/3594757
- WebView2 gamepad bugs: https://github.com/MicrosoftEdge/WebView2Feedback/issues/3025 and …/5507

**Claude**
- https://code.claude.com/docs/en/cli-reference · /headless · /model-config · /authentication · /setup
- https://code.claude.com/docs/en/agent-sdk/overview (third-party login policy)

**Local evidence (read-only, this PC)**
- `…\OneDrive\Documents\My Games\Path of Exile 2\` (`poe2_production_Config.ini`, `BuildPlanner\*.build`, NeverSink filters)
- `…\Steam\steamapps\common\Path of Exile 2\logs\Client.txt`

*This product isn't affiliated with or endorsed by Grinding Gear Games in any way.*
