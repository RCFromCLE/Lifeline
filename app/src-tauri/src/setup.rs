//! First-run setup: is Claude Code installed and signed in (it's how Lifeline
//! uses the player's own Claude plan), is Git for Windows there (Claude Code
//! on Windows uses it), is the game found. Install and sign-in open a
//! visible console the player can follow; nothing runs without a click.

use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::state::AppState;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Runs a short command quietly and returns stdout if it succeeded.
fn quiet(program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    // Report the subscription login, not an API key from the environment.
    cmd.env_remove("ANTHROPIC_API_KEY").env_remove("ANTHROPIC_AUTH_TOKEN");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn claude() -> String {
    lifeline_ai::claude_program().to_string_lossy().into_owned()
}

pub fn status(state: &AppState) -> Value {
    let program = claude();
    let version = quiet(&program, &["--version"]);
    let auth: Value = version
        .as_ref()
        .and_then(|_| quiet(&program, &["auth", "status", "--json"]))
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let logged_in = auth["loggedIn"].as_bool().unwrap_or(false);
    let method = auth["authMethod"].as_str().unwrap_or_default();
    let settings = state.settings.lock().unwrap().clone();
    json!({
        "claude": {
            "installed": version.is_some(),
            "version": version,
            "logged_in": logged_in,
            // Lifeline runs on a Claude subscription (Pro/Max), not API billing.
            "subscription": auth["subscriptionType"].as_str(),
            "uses_subscription": method == "claude.ai",
            "email": auth["email"].as_str(),
        },
        "git": quiet("git", &["--version"]).is_some(),
        "game_log": lifeline_gamefiles::paths::find_client_log().is_some(),
        "league": settings.league,
        "input": settings.input,
        "onboarded": settings.onboarded,
    })
}

/// Opens a console window running `script` in PowerShell.
fn console(title: &str, script: &str) -> Result<(), String> {
    let full = format!(
        "$Host.UI.RawUI.WindowTitle = '{title}'; {script}; Write-Host ''; \
         Write-Host 'Done. You can close this window and go back to Lifeline.' -ForegroundColor Yellow"
    );
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NoExit", "-ExecutionPolicy", "Bypass", "-Command", &full]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NEW_CONSOLE);
    cmd.spawn().map(|_| ()).map_err(|e| format!("Couldn't open PowerShell: {e}"))
}

/// Claude Code's official Windows installer (claude.ai/install.ps1).
pub fn install_claude() -> Result<(), String> {
    console(
        "Lifeline: installing Claude Code",
        "Write-Host 'Installing Claude Code (official installer from claude.ai)...' -ForegroundColor Cyan; \
         irm https://claude.ai/install.ps1 | iex",
    )
}

/// Git for Windows through winget.
pub fn install_git() -> Result<(), String> {
    console(
        "Lifeline: installing Git for Windows",
        "Write-Host 'Installing Git for Windows with winget...' -ForegroundColor Cyan; \
         winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements",
    )
}

/// Signs in with the player's Claude subscription (opens the browser).
pub fn login_claude() -> Result<(), String> {
    let program = claude().replace('\'', "''");
    console(
        "Lifeline: sign in to Claude",
        &format!(
            "Write-Host 'Sign in with your Claude account (Pro or Max) in the browser that opens.' -ForegroundColor Cyan; \
             & '{program}' auth login --claudeai"
        ),
    )
}

/// Saves the welcome answers and marks setup done.
pub fn finish(state: &AppState, league: String, input: String) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    if !league.trim().is_empty() {
        s.league = league.trim().to_owned();
    }
    if matches!(input.as_str(), "playstation" | "xbox" | "keyboard") {
        s.input = input;
    }
    s.onboarded = true;
    s.save(&state.data_dir)
}

/// Opens an https link in the default browser (build sources, video guides).
pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") || url.chars().any(|c| c.is_whitespace() || c == '"') {
        return Err("Only https links can be opened.".into());
    }
    Command::new("explorer.exe").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// A newer release on GitHub, if any: (version, page). Quietly None when
/// offline or the repository isn't public.
pub fn update_available() -> Option<Value> {
    let mut resp = ureq::get("https://api.github.com/repos/RCFromCLE/Lifeline/releases/latest")
        .header("User-Agent", "Lifeline update check")
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let body: Value = serde_json::from_str(&resp.body_mut().read_to_string().ok()?).ok()?;
    let tag = body["tag_name"].as_str()?.trim_start_matches('v').to_owned();
    let parse = |v: &str| v.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    (parse(&tag) > parse(env!("CARGO_PKG_VERSION")))
        .then(|| json!({"version": tag, "url": body["html_url"].as_str().unwrap_or("https://github.com/RCFromCLE/Lifeline/releases/latest")}))
}