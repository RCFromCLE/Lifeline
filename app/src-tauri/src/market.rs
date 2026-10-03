//! The trade window (pathofexile.com, where the player logs in once) and the
//! confirmed actions the AI proposes: travel to a seller, open a search.
//! Travel sends the same request the site's own "Travel to Hideout" button
//! does, from inside the logged-in page — the session cookie never leaves it.

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::AppState;

const TRADE_LABEL: &str = "trade";

fn trade_home(league: &str) -> String {
    format!(
        "https://www.pathofexile.com/trade2/search/poe2/{}",
        lifeline_trade::encode_segment(league)
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
        .on_page_load(|window, payload| {
            // A Travel waiting on the page (first load, or back from signing in).
            if payload.event() != PageLoadEvent::Finished || !on_trade_site(payload.url()) {
                return;
            }
            let pending = window.state::<AppState>().pending_travel.lock().unwrap().take();
            if let Some((listing, search)) = pending {
                let _ = window.eval(travel_script(&listing, &search));
            }
        })
        .build()
        .map_err(|e| e.to_string())
}

/// Travel's requests only work from the trade site's own pages.
fn on_trade_site(url: &tauri::Url) -> bool {
    matches!(url.host_str(), Some("www.pathofexile.com" | "pathofexile.com"))
}

fn travel_script(listing_id: &str, search_id: &str) -> String {
    let lid = serde_json::to_string(listing_id).unwrap_or_default();
    let qid = serde_json::to_string(search_id).unwrap_or_default();
    format!(
        r#"(async () => {{
  const banner = (msg, ok) => {{
    let b = document.getElementById('lifeline-banner');
    if (!b) {{ b = document.createElement('div'); b.id = 'lifeline-banner'; document.body.appendChild(b); }}
    b.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:2147483647;padding:12px;font:16px sans-serif;text-align:center;'
      + (ok ? 'background:#1f3d1a;color:#c8f0b0' : 'background:#4a1d17;color:#ffd0c8');
    b.textContent = 'Lifeline: ' + msg;
  }};
  try {{
    if (!/(^|\.)pathofexile\.com$/.test(location.hostname)) {{ banner('finish signing in to pathofexile.com, then press Travel again.', false); return; }}
    const hdr = {{ 'X-Requested-With': 'XMLHttpRequest' }};
    const r = await fetch('/api/trade2/fetch/' + {lid} + '?query=' + {qid} + '&realm=poe2', {{ credentials: 'same-origin', headers: hdr }});
    if (!(r.headers.get('content-type') || '').includes('json')) {{ banner('the trade site did not answer (HTTP ' + r.status + '). Log in on this page, then press Travel again.', false); return; }}
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

/// Trade ids are short alphanumeric strings.
fn plain_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Travel to the seller of a listing (one press). The card carries its
/// search id, so this works after a restart; older cards fall back to the
/// searches made this session.
pub fn travel(app: &AppHandle, listing_id: &str, search_id: Option<&str>) -> Result<String, String> {
    let state = app.state::<AppState>();
    let search_id = match search_id.filter(|s| plain_id(s)) {
        Some(s) => s.to_owned(),
        None => state
            .listings
            .lock()
            .unwrap()
            .get(listing_id)
            .cloned()
            .ok_or("This pick is from an older search. Press Rate (or ask again) to refresh the listings.")?,
    };
    if !plain_id(listing_id) {
        return Err("That listing id isn't valid.".into());
    }
    Ok(run_travel(app, listing_id, &search_id)?.into())
}

/// Travels now if the trade window is on pathofexile.com; otherwise opens
/// it (never navigating away from a sign-in in progress) and travels as
/// soon as it lands on the trade site.
fn run_travel(app: &AppHandle, listing_id: &str, search_id: &str) -> Result<&'static str, String> {
    let ready = app
        .get_webview_window(TRADE_LABEL)
        .and_then(|w| w.url().ok())
        .is_some_and(|u| on_trade_site(&u));
    let state = app.state::<AppState>();
    if ready {
        *state.pending_travel.lock().unwrap() = None;
        let window = open_trade_window(app, None)?;
        window
            .eval(travel_script(listing_id, search_id))
            .map_err(|_| "Couldn't reach the trade window.")?;
        Ok("Travel requested — watch the game.")
    } else {
        *state.pending_travel.lock().unwrap() = Some((listing_id.to_owned(), search_id.to_owned()));
        open_trade_window(app, None)?;
        Ok("Sign in to pathofexile.com in the trade window; Travel runs as soon as you're on the trade site.")
    }
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
    let result = run_confirmed(app, &action);
    // A failed action (planner folder locked, trade site down…) can be retried.
    if result.is_err() {
        state.actions.lock().unwrap().push(action);
    }
    result
}

fn run_confirmed(app: &AppHandle, action: &crate::state::PendingAction) -> Result<String, String> {
    let state = app.state::<AppState>();
    let league = state.settings.lock().unwrap().league.clone();
    match action.kind.as_str() {
        "travel" => {
            let lid = action.listing_id.as_deref().ok_or("no listing")?;
            let qid = action.search_id.as_deref().ok_or("no search")?;
            run_travel(app, lid, qid)?;
            Ok(format!(
                "Travel requested: {}. Watch the game (and the banner in the trade window).",
                action.summary
            ))
        }
        "open_search" => {
            let qid = action.search_id.as_deref().ok_or("no search")?;
            let url = format!(
                "https://www.pathofexile.com/trade2/search/poe2/{}/{qid}",
                lifeline_trade::encode_segment(&league)
            );
            open_trade_window(app, Some(&url))?;
            Ok("Opened the search in the trade window.".into())
        }
        "write_planner" => {
            let files = crate::builds::write_stages(&state)?;
            let _ = app.emit("planner-files", crate::builds::planner_files());
            Ok(format!(
                "Wrote {} stages to the Build Planner. In game: open the Build Planner and pick one.",
                files.len()
            ))
        }
        other => Err(format!("unknown action {other}")),
    }
}
