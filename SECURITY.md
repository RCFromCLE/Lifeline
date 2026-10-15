# Security

Lifeline is a free hobby project provided as is, without warranty (see [LICENSE](LICENSE)).
Security reports are still welcome and taken seriously.

## Reporting a problem

Please report vulnerabilities privately through GitHub:
**[Report a vulnerability](https://github.com/RCFromCLE/Lifeline/security/advisories/new)**.
Don't open a public issue for them.

Include what you found, how to reproduce it, and the Lifeline version (Settings → About).
Fixes ship as a new release; the app tells users when one is out.

## What Lifeline does on your PC

- Reads Path of Exile 2's log file and Build Planner folder, and writes build files into that folder when you ask.
- Never sends input to the game, and never reads or changes game memory.
- Checks which window is in front (its window class and title, nothing else), so the overlay shows only over the game.
- Runs Claude Code with your own account for two buttons only: Create full build and Suggest skills & rotations.
  Its built-in tools are limited to web search and web fetch plus Lifeline's own game-data lookups; everything else
  (running commands, editing files) is denied.
- Talks to GitHub (updates, class art, game data), pobb.in-style sites when you import a build link, and the PoE2 wiki.
  It never talks to pathofexile.com and never uses a game or trade-site login.

## How it's protected

- **Strict Content Security Policy.** The app's pages can only run Lifeline's own scripts and load images from the
  app itself and GitHub. Text from imported builds, AI output and the game log is always escaped before it is shown.
- **The AI can't act on its own.** A build it designs goes to My builds; following it or writing it into the game's
  Build Planner is a button you press.
- **Imported data is treated as data.** Build codes and planner files have size limits, file names can't leave the
  Build Planner folder, and names from imported builds are stripped to plain text before the AI sees them.
- **The local tool server** listens on 127.0.0.1 only, with a random token for each launch.

## Automated scanning

- **CodeQL** scans the Rust backend, the JavaScript UI and the build workflows on every push and weekly.
- **RustSec audit** checks every dependency on each change and daily.
- **Dependabot** raises alerts and opens pull requests for vulnerable dependencies.
- **Secret scanning with push protection** blocks commits that contain credentials.

## Verifying a download

Releases are built by GitHub Actions from this repository, with every action pinned to a commit.
Each release includes `SHA256SUMS.txt` and signed build provenance:

```
gh attestation verify Lifeline_<version>_x64-setup.exe -R RCFromCLE/Lifeline
```

Only download Lifeline from [this repository's releases](https://github.com/RCFromCLE/Lifeline/releases).
The installer isn't code-signed yet, so Windows shows a SmartScreen warning.
