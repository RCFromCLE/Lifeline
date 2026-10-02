//! The trade window (pathofexile.com, where the player logs in once) and the
//! confirmed actions the AI proposes: travel to a seller, open a search.
//! Travel sends the same request the site's own "Travel to Hideout" button
//! does, from inside the logged-in page — the session cookie never leaves it.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::AppState;

const TRADE_LABEL: &str = "trade";

fn trade_home(league: &str) -> String {
    format!(
        "https://www.pathofexile.com/trade2/search/poe2/{}",
        polr_trade::encode_segment(league)
    )
}

/// Shows (creating if needed) the trade window, optionally at `url`.
pub fn open_trade_window(app: &AppHandle, url: Option<&str>) -> Result<WebviewWindow, String> {
    let league = app.state::<AppState>().settings.lock().unwrap().league.clone();
    let target = url.map(str::to_owned).unwrap_or_else(|| trade_home(&league));
    if let Some(w) = app.get_webview_window(TRADE_LABEL) {
        if url.is_some() {
            let parsed = target.parse().map_err(|e| format!("bad url: {e}"))?;
            w.navigate(parsed).map_err(|e| e.to_string())?;
        }
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(w);
    }
    let parsed = target.parse().map_err(|e| format!("bad url: {e}"))?;
    WebviewWindowBuilder::new(app, TRADE_LABEL, WebviewUrl::External(parsed))
        .title("pathofexile.com trade — log in here once")
        .inner_size(1180.0, 820.0)
        .build()
        .map_err(|e| e.to_string())
}

fn travel_script(listing_id: &str, search_id: &str) -> String {
    let lid = serde_json::to_string(listing_id).unwrap_or_default();
    let qid = serde_json::to_string(search_id).unwrap_or_default();
    format!(
        r#"(async () => {{
  const banner = (msg, ok) => {{
    let b = document.getElementById('polr-banner');
    if (!b) {{ b = document.createElement('div'); b.id = 'polr-banner'; document.body.appendChild(b); }}
    b.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:2147483647;padding:12px;font:16px sans-serif;text-align:center;'
      + (ok ? 'background:#1f3d1a;color:#c8f0b0' : 'background:#4a1d17;color:#ffd0c8');
    b.textContent = 'PathOfLeastResistance: ' + msg;
  }};
  try {{
    const hdr = {{ 'X-Requested-With': 'XMLHttpRequest' }};
    const r = await fetch('/api/trade2/fetch/' + {lid} + '?query=' + {qid} + '&realm=poe2', {{ credentials: 'same-origin', headers: hdr }});
    const j = await r.json();
    const listing = j && j.result && j.result[0] && j.result[0].listing;
    const token = listing && listing.hideout_token;
    if (!token) {{ banner(listing ? 'no travel token — log in on this page, or this listing is not Instant Buyout.' : 'listing not found (sold or expired).', false); return; }}
    const w = await fetch('/api/trade2/whisper', {{ method: 'POST', credentials: 'same-origin',
      headers: Object.assign({{ 'Content-Type': 'application/json' }}, hdr), body: JSON.stringify({{ token }}) }});
    if (w.ok) banner('travelling to the seller\'s hideout — check the game.', true);
    else if (w.status === 404) banner('that item already sold.', false);
    else banner('travel failed (HTTP ' + w.status + '). You must be in game, in the same league, past Act 4.', false);
  }} catch (e) {{ banner('travel failed: ' + e, false); }}
}})();"#
    )
}

/// Travel to the seller of a listing from a recent search (one press).
pub fn travel(app: &AppHandle, listing_id: &str) -> Result<String, String> {
    let state = app.state::<AppState>();
    let search_id = state
        .listings
        .lock()
        .unwrap()
        .get(listing_id)
        .cloned()
        .ok_or("That listing is no longer in a recent search.")?;
    run_travel(app, listing_id, &search_id)?;
    Ok("Travel requested — watch the game (and the banner in the trade window).".into())
}

fn run_travel(app: &AppHandle, listing_id: &str, search_id: &str) -> Result<(), String> {
    let existed = app.get_webview_window(TRADE_LABEL).is_some();
    let window = open_trade_window(app, None)?;
    let script = travel_script(listing_id, search_id);
    let app2 = app.clone();
    std::thread::spawn(move || {
        // A freshly opened window needs to load the site first.
        if !existed {
            std::thread::sleep(Duration::from_secs(6));
        }
        if window.eval(&script).is_err() {
            let _ = app2.emit("notice", "Couldn't reach the trade window.");
        }
    });
    Ok(())
}

/// Runs a confirmed action.
pub fn confirm(app: &AppHandle, action_id: u64) -> Result<String, String> {
    let state = app.state::<AppState>();
    let action = {
        let mut actions = state.actions.lock().unwrap();
        let pos = actions
            .iter()
            .position(|a| a.id == action_id)
            .ok_or("That action is gone.")?;
        actions.remove(pos)
    };
    let league = state.settings.lock().unwrap().league.clone();
    match action.kind.as_str() {
        "travel" => {
            let lid = action.listing_id.as_deref().ok_or("no listing")?;
            let qid = action.search_id.as_deref().ok_or("no search")?;
            let existed = app.get_webview_window(TRADE_LABEL).is_some();
            let window = open_trade_window(app, None)?;
            let script = travel_script(lid, qid);
            let app2 = app.clone();
            std::thread::spawn(move || {
                // A freshly opened window needs to load the site first.
                if !existed {
                    std::thread::sleep(Duration::from_secs(6));
                }
                if window.eval(&script).is_err() {
                    let _ = app2.emit("notice", "Couldn't reach the trade window.");
                }
            });
            Ok(format!(
                "Travel requested: {}. Watch the game (and the banner in the trade window).",
                action.summary
            ))
        }
        "open_search" => {
            let qid = action.search_id.as_deref().ok_or("no search")?;
            let url = format!(
                "https://www.pathofexile.com/trade2/search/poe2/{}/{qid}",
                polr_trade::encode_segment(&league)
            );
            open_trade_window(app, Some(&url))?;
            Ok("Opened the search in the trade window.".into())
        }
        other => Err(format!("unknown action {other}")),
    }
}
