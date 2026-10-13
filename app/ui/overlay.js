// The in-game HUD (overlay.html). Uses escapeHtml and the Tauri globals from common.js.
const el = id => document.getElementById(id);
const win = () => window.__TAURI__.window.getCurrentWindow();
const ROT_LABEL = { Clearing: "Clear", Bossing: "Boss", Emergency: "Panic" };

// Panel: "answer" (hotkey replies), "rotation", or none; hovering the grade shows its reason on top.
let mode = null, hoverGrade = false, compact = false;
let answer = { label: "", status: "", html: "", error: false, conv: null, raw: "" };
let rating = null, rotations = null, rotIndex = 1, nextPenalty = null;

function renderPanel() {
  const show = !compact && (hoverGrade || mode);
  el("panel").classList.toggle("hidden", !show);
  el("rot-btn").classList.toggle("on", mode === "rotation");
  if (!show) return fit();
  el("p-open").classList.toggle("hidden", hoverGrade || mode !== "answer");
  el("p-close").classList.toggle("hidden", hoverGrade);
  if (hoverGrade) {
    el("ptitle").textContent = rating ? `${rating.grade} · ${rating.stage} · click to re-rate` : "Click to rate";
    el("pbody").innerHTML = rating
      ? `<p>${escapeHtml(rating.summary || "")}</p><p style="color:#a8977f">${escapeHtml(rating.explanation || "")}</p>`
      : `<p style="color:#a8977f">Grades your build F–S+ for where you are.</p>`;
  } else if (mode === "answer") {
    el("ptitle").textContent = answer.label;
    el("pbody").innerHTML = (answer.status ? `<div class="status">${escapeHtml(answer.status)}</div>` : "")
      + `<div class="${answer.error ? "err" : ""}">${answer.html}</div>`;
  } else if (mode === "rotation") {
    if (!rotations) {
      el("ptitle").textContent = "Rotation";
      el("pbody").innerHTML = `<div style="color:#a8977f">None yet.</div><button class="mini" id="make-rot">Make one</button>`;
      el("make-rot").onclick = () => { invoke("overlay_action", { action: "skills" }); el("rot-btn").classList.add("busy"); };
    } else {
      rotIndex = Math.min(rotIndex, rotations.length - 1);
      el("ptitle").innerHTML = `<span class="tabs">${rotations.map((r, i) =>
        `<button class="tab ${i === rotIndex ? "on" : ""}" data-i="${i}">${escapeHtml(ROT_LABEL[r.situation] || r.situation)}</button>`).join("")}</span>`;
      el("ptitle").querySelectorAll(".tab").forEach(b => b.onclick = () => { rotIndex = Number(b.dataset.i); renderPanel(); });
      el("pbody").innerHTML = `<ol>${rotations[rotIndex].steps.map(s => `<li>${escapeHtml(s)}</li>`).join("")}</ol>`;
    }
  }
  fit();
}

// The window hugs the content; only the bar and panel take clicks.
let fitFrame = 0;
function fit() {
  cancelAnimationFrame(fitFrame);
  fitFrame = requestAnimationFrame(() => {
    const r = el("root").getBoundingClientRect();
    invoke("fit_overlay", { width: r.width + 6, height: r.height + 6 });
    const rects = [...document.querySelectorAll(".hit")].filter(e => e.offsetParent).map(e => {
      const b = e.getBoundingClientRect();
      return [b.left, b.top, b.right, b.bottom];
    });
    invoke("overlay_regions", { rects });
  });
}
new ResizeObserver(fit).observe(el("root"));

function setCompact(on) {
  compact = !!on;
  document.body.classList.toggle("compact", compact);
  renderPanel();
}

function render(h) {
  const c = h.character;
  // The resistances that matter in this zone (researched), most important first.
  nextPenalty = h.next_penalty;
  const r = h.resists || {};
  const f = r.focus || {};
  const NAMES = { fire: "Fire", cold: "Cold", lightning: "Light", chaos: "Chaos" };
  const focus = (f.resists || []).filter(k => NAMES[k]);
  el("pen").className = "res full";
  const zone = r.zone ? `<span class="zn">${escapeHtml(r.zone)}</span>` : "";
  el("pen").innerHTML = zone + (focus.length
    ? focus.map(k => `<span class="el ${k}">${NAMES[k]}</span>`).join("")
    : f.physical ? `<span class="el">Physical</span>` : (r.need != null ? `Cap ${r.need}%` : "Res –"));
  el("pen").title = `${r.zone || "This zone"}${f.boss ? ` (boss: ${f.boss})` : ""}: ${f.why || "no data"}`
    + (r.need != null ? `
Capped at 75% here needs ${r.need}% (${r.penalty}% penalty).` : "")
    + (nextPenalty ? `
Next: ${nextPenalty}` : "");
  el("deaths").textContent = `☠${c.deaths}`;
  el("deaths").classList.toggle("hidden", !c.deaths);
  document.body.classList.toggle("unlocked", !!h.unlocked);
  fit();
}
let refreshTimer = null;
function refresh() { clearTimeout(refreshTimer); refreshTimer = setTimeout(() => invoke("hud").then(render), 250); }

function renderGrade(r) {
  rating = r;
  el("grade").textContent = r ? r.grade : "?";
  el("grade").className = `grade ${r ? "g-" + r.grade[0] : "g-none"}`;
  if (hoverGrade) renderPanel();
}

// ---- interactions ----
document.querySelectorAll(".drag").forEach(d => d.addEventListener("mousedown", ev => {
  if (ev.button === 0) { ev.stopPropagation(); win().startDragging(); }
}));
el("bar").addEventListener("mousedown", ev => {
  if (ev.button === 0 && document.body.classList.contains("unlocked") && ev.target === el("bar")) win().startDragging();
});
document.querySelectorAll("[data-action]").forEach(b => b.addEventListener("click", () => {
  invoke("overlay_action", { action: b.dataset.action }).catch(e => {
    answer = { ...answer, label: "Error", status: "", html: escapeHtml(String(e)), error: true };
    mode = "answer"; renderPanel();
  });
}));
el("rot-btn").addEventListener("click", () => { mode = mode === "rotation" ? null : "rotation"; renderPanel(); });
el("p-close").addEventListener("click", () => { mode = null; renderPanel(); });
el("p-open").addEventListener("click", () => invoke("open_in_game_chat"));
el("grade").addEventListener("mouseenter", () => { hoverGrade = true; renderPanel(); });
el("grade").addEventListener("mouseleave", () => { hoverGrade = false; renderPanel(); });
el("grade").addEventListener("click", () => { el("grade").classList.add("busy"); invoke("rate_now"); });

// ---- events ----
listen("hud-compact", ({ payload }) => setCompact(payload));
listen("overlay-mode", ({ payload }) => document.body.classList.toggle("unlocked", !!payload));
listen("character", refresh);
listen("feed", refresh);
listen("ai", ({ payload: e }) => {
  if (e.type === "start") {
    if (!e.hotkey) return;
    answer = { label: e.label, status: "…", html: "", error: false, conv: e.conv, raw: "" };
    mode = "answer";
  } else if (e.conv !== answer.conv) return;
  else if (e.type === "tool") answer.status = "…";
  else if (e.type === "delta") { answer.raw += e.text; answer.status = ""; answer.html = renderMarkdown(answer.raw); }
  else if (e.type === "done") { answer.status = ""; answer.html = renderMarkdown(e.text || answer.raw); answer.error = !!e.error; }
  else return;
  if (mode === "answer") renderPanel();
});
listen("rating", ({ payload }) => { renderGrade(payload.rating); el("grade").classList.toggle("busy", !!payload.busy); });
listen("rating-status", ({ payload }) => el("grade").classList.toggle("busy", !!payload.busy));
listen("skills", ({ payload }) => {
  const r = payload.plan && payload.plan.rotations;
  rotations = Array.isArray(r) && r.length ? r : null;
  el("rot-btn").classList.remove("busy");
  if (mode === "rotation") renderPanel();
});
listen("skills-status", ({ payload }) => el("rot-btn").classList.toggle("busy", !!payload.busy));
listen("gear-watch", ({ payload }) => el("gear-btn").classList.toggle("on", payload.seconds > 0));
// Every sound shows what it meant for 3 seconds.
let cueTimer = null;
listen("sound-cue", ({ payload: c }) => {
  const pop = el("cue-pop");
  pop.className = `cue-pop ${c.cue}`;
  el("cue-title").textContent = c.title;
  el("cue-detail").textContent = c.detail || "";
  clearTimeout(cueTimer);
  cueTimer = setTimeout(() => pop.classList.add("hidden"), 3000);
});
// Short messages (gear recorded, copy failed…) show on the HUD for a few seconds.
let noticeTimer = null;
// Settings → Sounds & notifications can turn HUD messages off.
let hudNotices = true;
invoke("notification_settings").then(s => { hudNotices = s.hud_notices !== false; }).catch(() => {});
listen("notification-settings", ({ payload }) => { hudNotices = payload.hud_notices !== false; });
listen("notice", ({ payload }) => {
  if (!hudNotices) return;
  const text = String(payload);
  const bad = /couldn|failed|isn't|no item|blocks|copy an item/i.test(text);
  answer = { ...answer, label: text.startsWith("✓") ? "Done" : "Lifeline", status: "", html: escapeHtml(text), error: bad };
  mode = "answer"; renderPanel();
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => { if (mode === "answer" && answer.html === escapeHtml(text)) { mode = null; renderPanel(); } }, bad ? 9000 : 3500);
});
invoke("gear_watch_left").then(s => el("gear-btn").classList.toggle("on", s > 0)).catch(() => {});

invoke("hud").then(h => {
  render(h);
  renderGrade(h.rating);
  rotations = Array.isArray(h.rotations) && h.rotations.length ? h.rotations : null;
  setCompact(h.compact);
});
setInterval(refresh, 15000);
