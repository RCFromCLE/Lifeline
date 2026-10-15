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
pub fn finish(state: &AppState, input: String) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    if matches!(input.as_str(), "playstation" | "xbox" | "keyboard") {
        s.input = input;
    }
    s.onboarded = true;
    s.save(&state.data_dir)
}

/// Opens an https link in the default browser (build sources, video guides).
pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") || url.len() > 2000 || url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | ',' | '\\')) {
        return Err("Only https links can be opened.".into());
    }
    Command::new("explorer.exe").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// A newer release on GitHub, if any: (version, page). Quietly None when
/// offline or the repository isn't public.
pub fn update_available() -> Option<Value> {
    let latest = latest_release()?;
    (latest["newer"] == true).then(|| json!({"version": latest["version"], "url": latest["url"]}))
}

/// The newest release on GitHub: version, name, date, its notes (Markdown,
/// capped) and whether it's newer than this copy. Fetched at most every ten
/// minutes; None when offline.
pub fn latest_release() -> Option<Value> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, Value)>> = Mutex::new(None);
    if let Some((at, v)) = CACHE.lock().unwrap().as_ref() {
        if at.elapsed() < Duration::from_secs(600) {
            return Some(v.clone());
        }
    }
    let mut resp = crate::builds::web()
        .get("https://api.github.com/repos/RCFromCLE/Lifeline/releases/latest")
        .header("User-Agent", "Lifeline update check")
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let body: Value = serde_json::from_str(&resp.body_mut().read_to_string().ok()?).ok()?;
    let v = release_info(&body, env!("CARGO_PKG_VERSION"))?;
    *CACHE.lock().unwrap() = Some((Instant::now(), v.clone()));
    Some(v)
}

/// The fields the app shows from a GitHub release (`installed`: this copy's version).
fn release_info(body: &Value, installed: &str) -> Option<Value> {
    let tag = body["tag_name"].as_str()?.trim_start_matches('v').to_owned();
    let parse = |v: &str| v.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    // Always this repository's release page, whatever the response says.
    let url = format!("https://github.com/RCFromCLE/Lifeline/releases/tag/v{}", tag.replace(|c: char| !c.is_ascii_alphanumeric() && c != '.', ""));
    let notes: String = body["body"].as_str().unwrap_or_default().chars().take(20_000).collect();
    Some(json!({
        "version": tag,
        "newer": parse(&tag) > parse(installed),
        "name": body["name"].as_str().unwrap_or_default(),
        "published": body["published_at"].as_str().unwrap_or_default(),
        "notes": notes,
        "url": url,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_fields_and_newer() {
        let body = json!({"tag_name": "v0.6.2", "name": "v0.6.2", "published_at": "2026-10-15T12:00:00Z",
                          "body": "**Dashboard**
- The playthrough log is back.", "html_url": "https://evil.example/x"});
        let r = release_info(&body, "0.6.1").unwrap();
        assert_eq!(r["version"], "0.6.2");
        assert_eq!(r["newer"], true);
        assert_eq!(r["url"], "https://github.com/RCFromCLE/Lifeline/releases/tag/v0.6.2", "the link is always this repo's");
        assert!(r["notes"].as_str().unwrap().contains("playthrough log"));
        assert_eq!(release_info(&body, "0.6.2").unwrap()["newer"], false);
        assert_eq!(release_info(&body, "0.10.0").unwrap()["newer"], false, "versions compare as numbers");
        assert!(release_info(&json!({}), "0.6.1").is_none());
    }
}