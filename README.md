# PathOfLeastResistance

An AI-assisted, hardcore-first playthrough companion for **Path of Exile 2**,
written in Rust. It takes a build link, splits it into Act 1 → Act 4 →
Interludes → Endgame, writes those stages straight into the game's own
**Build Planner**, generates matching **item filters**, watches your game log
to know where you are, and lets you talk (or type) to Claude about your gear,
gems, market finds and next steps — controller-first (PS5 DualSense), with full
mouse/keyboard support.

> Status: planning + foundations. See [PLAN.md](PLAN.md) for the full,
> research-grounded plan and roadmap.

## Layout

```
PLAN.md                    the plan (start here)
Cargo.toml                 Rust workspace
crates/
  polr-gamefiles/          files the PoE2 client reads/writes:
                             BuildPlanner/*.build, logs/Client.txt events,
                             poe2_production_Config.ini
  polr-pob/                Path of Building codes + share links, build XML,
                             stage classification, PoB's level estimator
  polr-ai/                 drives your own Claude Code CLI (subscription login):
                             companion + 9 specialist agents (build architect,
                             auditor, gear appraiser, market scout, HC safety
                             officer, route coach, filter smith, fact checker,
                             patch analyst), background jobs, usage meters
  polr-data/               GGG's official passive-tree export
  polr-model/              PoB build → stages → in-game .build files
app/
  src-tauri/               the desktop app (Tauri 2): hotkeys, HUD overlay, log
                             watcher, build import/export, Opus 5.5 companion
  ui/                      main window + overlay (HTML/CSS/JS)
```

Next crates (MCP tools, filters, trade, controller input, voice, hardcore
suite) are scheduled in PLAN.md §11 and §13.

## Run the app

```powershell
cargo build --release -p polr-app
.\target\release\PathOfLeastResistance.exe        # or double-click it
```

| Hotkey (default, change in *Hotkeys & settings*) | What it does |
|---|---|
| `Alt+Shift+D` | **Item check** — with an item hovered in PoE2, sends one Ctrl+Alt+C, the Opus 5.5 gear appraiser judges it, answer shows on the HUD |
| `Alt+Shift+N` | **What next** — route coach's next 1–3 steps for where you are |
| `Alt+Shift+A` | **Ask** — brings the app up with the chat focused |
| `Alt+Shift+O` | **HUD overlay** — show/hide the click-through overlay over the game |

Tabs: **Play** (parallel conversations that know about each other, live game feed, usage meters; ask the market in plain words — the market scout searches, ranks listings for your build and gives you a **Travel to hideout** button; press **Trade site login** once first), **Build** (paste a PoB code or
pobb.in link → act-by-act stages → write them into the in-game Build Planner), **Hotkeys & settings**
(hotkeys, league — default HC Forbidden Rites). Play PoE2 in windowed fullscreen for the overlay.

## Prerequisites (Windows)

- Rust (stable, via rustup) and the Visual Studio 2022 Build Tools
  ("Desktop development with C++") — needed to compile.
- WebView2 runtime — already present on Windows 11.
- Claude Code CLI, logged in with your Claude Pro/Max subscription — the app
  talks to Claude through it (PLAN.md §6).

```powershell
cargo test --workspace                     # offline tests (48)
cargo run -p polr-ai --example smoke       # live check against your Claude Code login (uses a little usage)
```

*This product isn't affiliated with or endorsed by Grinding Gear Games in any way.*
