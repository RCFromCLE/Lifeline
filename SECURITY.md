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
- Sends one copy keystroke to the game only when you press a Lifeline hotkey (item check or gear recording); it never reads or changes game memory.
- Runs Claude Code with your own account for AI features. Its built-in tools are limited to web search and web fetch
  plus Lifeline's own game-data tools; everything else (running commands, editing files) is denied.
- Talks to pathofexile.com (trade site) without your session cookie, and to GitHub to check for updates.
