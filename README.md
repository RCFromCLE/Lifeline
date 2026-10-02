# PathOfLeastResistance

An AI-assisted, hardcore-first playthrough companion for **Path of Exile 2**,
written in Rust. It takes a build link, splits it into Act 1 → Act 4 →
Interludes → Endgame, writes those stages straight into the game's own
**Build Planner**, generates matching **item filters**, watches your game log
to know where you are, and lets you talk (or type) to Claude about your gear,
gems, market finds and next steps — controller-first (PS5 DualSense), with full
mouse/keyboard support.

> Status: early but playable. See [PLAN.md](PLAN.md) for the full,
> research-grounded plan and roadmap.

## Install on your gaming PC (laptop)

You don't need Rust or any developer tools for this. You need **Windows 10 or 11**,
**Path of Exile 2** (Steam on any drive, or the standalone client), and your
**Claude Pro/Max** subscription.

### 1. Install Claude Code and log in (one time)

The app talks to Claude through Claude Code, using your subscription, not an API key.
Open **PowerShell** (Start → type `powershell` → Enter) and run these one at a time:

```powershell
winget install --id Git.Git -e          # Claude Code on Windows uses Git for Windows
irm https://claude.ai/install.ps1 | iex # installs Claude Code
```

Close PowerShell, open a **new** one, then log in:

```powershell
claude --version   # should print a version number
claude             # opens your browser: choose "Claude account with subscription" and approve
```

When it says you're logged in, type `/exit` and close PowerShell.

### 2. Download the installer

1. In your browser, sign in to GitHub as **RCFromCLE** (the repo is private).
2. Go to **https://github.com/RCFromCLE/PathOfLeastResistance/releases/latest**.
3. Under **Assets**, click **`PathOfLeastResistance_<version>_x64-setup.exe`** to download it.

(Or from PowerShell, if you use the GitHub CLI: `gh release download --repo RCFromCLE/PathOfLeastResistance --pattern "*setup.exe"`.)

### 3. Install and run it

1. Double-click the downloaded `…_x64-setup.exe`.
2. If Windows shows **"Windows protected your PC"**, click **More info → Run anyway**.
   The installer isn't code-signed, so Windows warns about it.
3. Click through the installer. It installs just for your user, so it doesn't need admin rights.
4. Start **PathOfLeastResistance** from the Start menu.

### 4. First-run checklist

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
  when the app is in front it shrinks to just the grade. Drag the **⠿** handle to move it (remembered).
  Buttons: grade (click to rate; hover for why), resistance penalty (▲ = the next act lowers it),
  🔍 check item, → what next, ⚡ rotation, 🛡 record gear, 💬 open app, ✕ hide.
### Updating

Download the newest `…_x64-setup.exe` from the same Releases page and run it.
Your settings, conversations, ratings and recorded gear are kept. They live in
`%APPDATA%\com.rudyc.pathofleastresistance` and are separate on each PC.

### Uninstalling

Windows Settings → Apps → Installed apps → **PathOfLeastResistance** → Uninstall.

### Troubleshooting

- **AI answers fail or say Claude Code isn't found:** open a new PowerShell and run
  `claude --version`. If that fails, redo step 1. If it works, run `claude` once to make
  sure you're still logged in.
- **A hotkey does nothing:** another program may own it. Change it in *Hotkeys & settings*.
- **Anything else:** the app writes a diagnostics log to
  `%APPDATA%\com.rudyc.pathofleastresistance\debug.log`.
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

Tabs: **Play** (parallel conversations that know about each other, live game feed, usage meters; ask the market in plain words — the market scout searches, ranks listings for your build and gives you a **Travel to hideout** button; press **Trade site login** once first), **Build** (**Create with AI**: answer up to 4 quick questions and the build architect designs Act 1 → Endgame for your class: passive paths computed within each stage's points, gems and supports checked against game data, gear goals. One click writes it to the in-game Build Planner. Or paste a PoB code or
pobb.in link → act-by-act stages → write them into the in-game Build Planner), **Hotkeys & settings**
(hotkeys, league — default HC Forbidden Rites). Play PoE2 in windowed fullscreen for the overlay.

## Building from source (developers)

- Rust (stable, via rustup) and the Visual Studio 2022 Build Tools
  ("Desktop development with C++") — needed to compile.
- WebView2 runtime — already present on Windows 11.
- Claude Code CLI, logged in with your Claude Pro/Max subscription — the app
  talks to Claude through it (PLAN.md §6).

```powershell
cargo build --release -p polr-app          # dev build: .\target\release\PathOfLeastResistance.exe
cargo install tauri-cli --version "^2" --locked
cd app\src-tauri; cargo tauri build       # installer: target\release\bundle\nsis\*-setup.exe
cargo test --workspace                     # offline tests
cargo run -p polr-ai --example smoke       # live check against your Claude Code login (uses a little usage)
```

### Publishing a new version

1. Bump `version` in `app/src-tauri/tauri.conf.json` and in `[workspace.package]` of the root `Cargo.toml`, then commit.
2. `git tag v0.1.1; git push origin main --tags`.

GitHub Actions (`.github/workflows/release.yml`) builds the Windows installer and attaches it to
that tag's release. The laptop then downloads it as described above.

*This product isn't affiliated with or endorsed by Grinding Gear Games in any way.*
