# Lifeline

An AI-assisted, hardcore-first playthrough companion for **Path of Exile 2**,
written in Rust. It takes a build link, splits it into Act 1 → Act 4 →
Interludes → Endgame, writes those stages straight into the game's own
**Build Planner**, generates matching **item filters**, watches your game log
to know where you are, and lets you talk (or type) to Claude about your gear,
gems, market finds and next steps — controller-first (PS5 DualSense), with full
mouse/keyboard support.

> Status: early but playable. See [PLAN.md](PLAN.md) for the full,
> research-grounded plan and roadmap.

## Install

You need:
- **Windows 10 or 11**
- **Path of Exile 2** (Steam on any drive, or the standalone client)
- A **Claude Pro or Max** subscription. Lifeline's AI runs on your own plan, with no API key and no extra bill.

You don't need any developer tools.

### 1. Download

Go to **https://github.com/RCFromCLE/Lifeline/releases/latest**. Under **Assets**, download **`Lifeline_<version>_x64-setup.exe`**.

### 2. Install

1. Double-click the downloaded file.
2. If Windows shows **"Windows protected your PC"**, click **More info → Run anyway**. The installer isn't code-signed yet, so Windows warns about it.
3. Click through the installer. It installs just for your user, so it doesn't need admin rights.
4. Start **Lifeline** from the Start menu.

### 3. The welcome takes it from there

On first start, Lifeline walks you through everything:
1. **Connect Claude:** it checks for Claude Code (Anthropic's free app that Lifeline runs your AI through) and Git for Windows. Missing pieces get **Install** buttons; then **Sign in** opens your browser to log in with your Claude account. Each step ticks off by itself.
2. **Your game:** it finds PoE2 and asks how you play (PlayStation controller, Xbox controller, or mouse and keyboard) and your league.
3. **How it works:** a one-page tour of the HUD, hotkeys, builds, rating, market and sounds.

You can reopen the tour any time from **Settings → Welcome tour**.

<details><summary>Prefer to set up Claude Code by hand?</summary>

```powershell
winget install --id Git.Git -e           # Git for Windows (Claude Code uses it)
irm https://claude.ai/install.ps1 | iex  # Claude Code
claude auth login --claudeai             # sign in with your Claude account (opens the browser)
```
</details>
### Good to know

- **PoE2 display mode:** Options → Graphics → Display Mode → **Windowed Fullscreen**.
  The HUD can't draw over exclusive fullscreen.
- **Game log:** found automatically in any Steam library or the standalone folder.
  If the HUD says it wasn't found, start the game once; the app keeps checking.
- **League:** *Hotkeys & settings* tab (default **HC Forbidden Rites**).
- **Trade:** on the **Play** tab, click **Trade site login** once and log in to
  pathofexile.com. The *Travel to hideout* buttons need this.
- **Your gear:** in game, hover each item you're wearing and press **Alt+Shift+E**.
  This powers the ±% on market cards and the build rating.
- **HUD:** a small bar of buttons in the top-left corner. It only shows while PoE2 is running and in front;
  when the app is in front it shrinks to just the grade. Drag the Lifeline logo on its left to move it (remembered).
  Buttons: grade (click to rate; hover for why), resistance penalty (▲ = the next act lowers it),
  🔍 check item, → what next, ⚡ rotation, 🛡 record gear, 💬 open app, ✕ hide.
- **Sounds:** short cues play for a level up, a new act, entering a boss area, a resistance penalty drop,
  death and answers being ready. Turn them off or change the volume in *Settings → Sounds*.

### Upgrading from 0.1.x (when it was called "PathOfLeastResistance")

The app is now **Lifeline**, so Windows sees it as a new program:
1. Install Lifeline as above.
2. Uninstall the old one: Windows Settings → Apps → Installed apps → **PathOfLeastResistance** → Uninstall.

On its first start, Lifeline copies your settings, chats, ratings, saved builds and recorded gear from the old app.

### Updating

Download the newest `…_x64-setup.exe` from the same Releases page and run it.
Your settings, conversations, ratings and recorded gear are kept. They live in
`%APPDATA%\com.rudyc.lifeline` and are separate on each PC.

### Uninstalling

Windows Settings → Apps → Installed apps → **Lifeline** → Uninstall.

### Troubleshooting

- **AI answers fail or say Claude Code isn't found:** open a new PowerShell and run
  `claude --version`. If that fails, redo step 1. If it works, run `claude` once to make
  sure you're still logged in.
- **A hotkey does nothing:** another program may own it. Change it in *Hotkeys & settings*.
- **Anything else:** the app writes a diagnostics log to
  `%APPDATA%\com.rudyc.lifeline\debug.log`.
## Layout

```
PLAN.md                    the plan (start here)
Cargo.toml                 Rust workspace
crates/
  lifeline-gamefiles/          files the PoE2 client reads/writes:
                             BuildPlanner/*.build, logs/Client.txt events,
                             poe2_production_Config.ini
  lifeline-pob/                Path of Building codes + share links, build XML,
                             stage classification, PoB's level estimator
  lifeline-ai/                 drives your own Claude Code CLI (subscription login):
                             companion + 9 specialist agents (build architect,
                             auditor, gear appraiser, market scout, HC safety
                             officer, route coach, filter smith, fact checker,
                             patch analyst), background jobs, usage meters
  lifeline-data/               GGG's official passive-tree export
  lifeline-model/              PoB build → stages → in-game .build files
app/
  src-tauri/               the desktop app (Tauri 2): hotkeys, HUD overlay, log
                             watcher, build import/export, Opus 5.5 companion
  ui/                      main window + overlay (HTML/CSS/JS)
```

Next crates (MCP tools, filters, trade, controller input, voice, hardcore
suite) are scheduled in PLAN.md §11 and §13.

## Using it

| Hotkey (default, change in *Hotkeys & settings*) | What it does |
|---|---|
| `Alt+Shift+D` | **Item check** — with an item hovered in PoE2, sends one Ctrl+Alt+C, the Opus 5.5 gear appraiser judges it, answer shows on the HUD |
| `Alt+Shift+N` | **What next** — route coach's next 1–3 steps for where you are |
| `Alt+Shift+A` | **Ask** — brings the app up with the chat focused |
| `Alt+Shift+O` | **HUD on/off**: the small button bar. It only shows while the game is running and in front. |
| `Alt+Shift+E` | **Record equipped** — hover an item you're wearing; used for ±% on market cards and the build rating |
| `Alt+Shift+M` | **Move HUD**: unlock to drag, press again to lock. The ⠿ handle always drags it too. |

Click the **grade** on the overlay to rate your build (F → S+ for where you are in the campaign; hover for the why). The **Rating** tab shows the breakdown and market picks for the top upgrades within your budget (market searches run on Sonnet 5.5).

Tabs: **Play** (parallel conversations that know about each other, live game feed, usage meters; ask the market in plain words — the market scout searches, ranks listings for your build and gives you a **Travel to hideout** button; press **Trade site login** once first), **Builds**:
- **Showcase:** pick a class, then an ascendancy (with the game's own portraits), then what matters (damage, balanced or tanky; style; budget; complexity). You get ranked, researched hardcore builds with rating bars. **Create full build** designs Act 1 to Endgame for the pick: passive paths fit within each stage's points, and gems and supports are checked against game data.
- **My builds:** saved ideas and generated builds. **Use it** makes one your active build, and one click writes it to the in-game Build Planner.
- **Import:** paste a Path of Building code or link.

**Hotkeys & settings**
(hotkeys, league — default HC Forbidden Rites). Play PoE2 in windowed fullscreen for the overlay.

## Building from source (developers)

- Rust (stable, via rustup) and the Visual Studio 2022 Build Tools
  ("Desktop development with C++") — needed to compile.
- WebView2 runtime — already present on Windows 11.
- Claude Code CLI, logged in with your Claude Pro/Max subscription — the app
  talks to Claude through it (PLAN.md §6).

```powershell
cargo build --release -p lifeline-app          # dev build: .\target\release\Lifeline.exe
cargo install tauri-cli --version "^2" --locked
cd app\src-tauri; cargo tauri build       # installer: target\release\bundle\nsis\*-setup.exe
cargo test --workspace                     # offline tests
cargo run -p lifeline-ai --example smoke       # live check against your Claude Code login (uses a little usage)
```

### Publishing a new version

1. Bump `version` in `app/src-tauri/tauri.conf.json` and in `[workspace.package]` of the root `Cargo.toml`, then commit.
2. `git tag v0.1.1; git push origin main --tags`.

GitHub Actions (`.github/workflows/release.yml`) builds the Windows installer and attaches it to
that tag's release. The laptop then downloads it as described above.

*This product isn't affiliated with or endorsed by Grinding Gear Games in any way.*
