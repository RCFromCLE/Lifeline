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
```

More crates (game data, canonical build model, MCP tools, filters, trade,
controller input, voice, hardcore suite) and the Tauri app are scheduled in
PLAN.md §11 and §13.

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
