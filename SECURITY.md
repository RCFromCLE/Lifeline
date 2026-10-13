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
- Sends one copy keystroke to the game only when you press a Lifeline hotkey (item check or gear recording).
  Nothing else can trigger it: not a button, not the AI, not a web page. It never reads or changes game memory.
- Runs Claude Code with your own account for AI features. Its built-in tools are limited to web search and web fetch
  plus Lifeline's own game-data tools; everything else (running commands, editing files) is denied.
- Talks to pathofexile.com (trade site) without your session cookie, and to GitHub to check for updates.

## How it's protected

- **Strict Content Security Policy.** The app's pages can only run Lifeline's own scripts and load images from the
  trade site's image server and GitHub. Text from trade listings, AI answers, imported builds, the clipboard and the
  game log is always escaped before it is shown.
- **The AI can't act on its own.** Travelling to a seller, opening a trade search, writing build files and switching
  the build you follow each need you to press a button that says exactly what it does.
- **Imported data is treated as data.** Build codes and planner files have size limits, file names can't leave the
  Build Planner folder, and names from imported builds are stripped to plain text before the AI sees them.
- **The trade window** always shows which site it is on in its title, and only pathofexile.com can report a travel result.
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
