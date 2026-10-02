// Main window.
const $ = id => document.getElementById(id);
let settings = null;

function toast(text) {
  const t = $("toast");
  t.textContent = text;
  t.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => t.classList.remove("show"), 4500);
}

// ---- tabs (also D-pad/arrow friendly: they're plain buttons) ----
document.querySelectorAll(".tab").forEach(btn => btn.addEventListener("click", () => {
  document.querySelectorAll(".tab").forEach(b => b.classList.toggle("active", b === btn));
  document.querySelectorAll(".view").forEach(v => v.classList.toggle("active", v.id === `view-${btn.dataset.tab}`));
  if (btn.dataset.tab === "build") refreshPlannerFiles();
}));

// ---- character card + feed ----
function renderCharacter(c) {
  const d = describeCharacter(c);
  $("char-name").textContent = d.name;
  $("char-meta").textContent = d.where + (c.buffs && c.buffs.length ? ` · ${c.buffs.length} permanent buffs` : "");
  const pill = $("penalty-pill");
  pill.textContent = c.res_penalty === null || c.res_penalty === undefined ? "Res —" : `Res ${c.res_penalty}%`;
  pill.className = `stat ${penaltyClass(c.res_penalty)}`;
  $("deaths-pill").textContent = `☠ ${c.deaths}`;
}

function feedItem(item) {
  const li = document.createElement("li");
  li.innerHTML = `<time>${escapeHtml(item.time.slice(11, 16))}</time><span class="k-${item.kind}">${escapeHtml(item.text)}</span>`;
  return li;
}

function renderFeed(items) {
  const ul = $("feed");
  ul.innerHTML = "";
  items.slice().reverse().forEach(i => ul.appendChild(feedItem(i)));
}

// ---- usage meters ----
function renderUsage(u) {
  const names = { five_hour: "5-hour", seven_day: "7-day" };
  $("usage").innerHTML = (u.windows || []).map(w => {
    const cls = w.pct >= 80 ? "high" : w.pct >= 50 ? "mid" : "";
    return `<div class="meter"><span>${names[w.name] || w.name}</span><span class="bar"><i class="${cls}" style="width:${Math.min(100, w.pct)}%"></i></span><span>${w.pct}%</span></div>`;
  }).join("");
}

// ---- conversations + chat ----
// Each conversation is its own Claude session; several can answer at once.
let convs = [];          // [{id, title, busy, in_game, count}]
let active = null;       // selected conversation id
const live = {};         // conv id -> {raw, tools} while answering
const unread = new Set();

function renderConvList() {
  $("conv-list").innerHTML = convs.slice().sort((a, b) => (b.in_game - a.in_game) || (b.id - a.id)).map(c => `
    <li data-id="${c.id}" class="${c.id === active ? "active" : ""} ${c.in_game ? "in-game" : ""}" tabindex="0">
      <span class="t">${escapeHtml(c.title)}</span>
      ${c.busy ? '<span class="busy">●</span>' : unread.has(c.id) ? '<span class="unread"></span>' : ""}
    </li>`).join("");
  $("conv-list").querySelectorAll("li").forEach(li => {
    const open = () => selectConv(Number(li.dataset.id));
    li.addEventListener("click", open);
    li.addEventListener("keydown", ev => { if (ev.key === "Enter") open(); });
  });
}

function addMessage(kind, label, html) {
  const div = document.createElement("div");
  div.className = `msg ${kind}`;
  div.innerHTML = `<div class="label">${escapeHtml(label)}</div><div class="body">${html}</div>`;
  $("chat").appendChild(div);
  $("chat").scrollTop = $("chat").scrollHeight;
  return div;
}

function liveMessage() {
  return $("chat").querySelector(".msg.live");
}

function renderLive(conv) {
  const l = live[conv];
  if (!l || conv !== active) return;
  let m = liveMessage();
  if (!m) { m = addMessage("ai live", "Companion", ""); }
  m.querySelector(".body").innerHTML = l.raw ? renderMarkdown(l.raw) : "<em>Thinking…</em>";
  if (l.tool) {
    let t = m.querySelector(".tools");
    if (!t) { t = document.createElement("div"); t.className = "tools"; m.appendChild(t); }
    t.textContent = `↳ ${l.tool}`;
  }
  $("chat").scrollTop = $("chat").scrollHeight;
}

async function selectConv(id) {
  active = id;
  unread.delete(id);
  renderConvList();
  const c = await invoke("conversation", { conv: id });
  $("chat").innerHTML = "";
  if (!c) return;
  if (!c.messages.length) {
    addMessage("ai", "Companion", renderMarkdown(c.in_game
      ? "Hotkey answers land here."
      : "New chat. I can see your other chats."));
  }
  for (const m of c.messages) {
    if (m.role === "market") { try { $("chat").appendChild(renderMarket(JSON.parse(m.text))); } catch (_) { } continue; }
    const div = addMessage(m.role === "user" ? "user" : "ai", m.label, renderMarkdown(m.text));
    if (m.error) div.classList.add("error");
  }
  renderLive(id);
  renderActions(id);
}

listen("conversations", ({ payload }) => { convs = payload; renderConvList(); });

listen("ai", ({ payload: e }) => {
  const conv = e.conv;
  if (e.type === "start") {
    live[conv] = { raw: "", tool: "" };
    if (conv === active) { addMessage("user", e.label, renderMarkdown(e.question)); renderLive(conv); }
  } else if (e.type === "delta") {
    if (live[conv]) { live[conv].raw += e.text; renderLive(conv); }
  } else if (e.type === "tool") {
    if (live[conv]) { live[conv].tool = e.text; renderLive(conv); }
  } else if (e.type === "done") {
    delete live[conv];
    if (conv === active) {
      const m = liveMessage() || addMessage("ai", "Companion", "");
      m.classList.remove("live");
      m.querySelector(".body").innerHTML = renderMarkdown(e.text);
      if (e.error) m.classList.add("error");
      $("chat").scrollTop = $("chat").scrollHeight;
    } else {
      unread.add(conv);
      const c = convs.find(x => x.id === conv);
      toast(`Answer ready in "${c ? c.title : "another conversation"}"`);
      renderConvList();
    }
  } else if (e.type === "error") {
    toast(e.text);
  }
});

// ---- proposed actions (travel / open search) — only happen on a press ----
const actions = new Map(); // id -> action

function actionCard(a) {
  const div = document.createElement("div");
  div.className = "action-card";
  div.dataset.action = a.id;
  const label = a.kind === "travel" ? "Travel to hideout" : "Open search";
  div.innerHTML = `<div class="s"><div class="k">${a.kind === "travel" ? "Buy" : "Trade search"}</div>${escapeHtml(a.summary)}</div>
    <button class="primary go">${label}</button><button class="ghost no">Dismiss</button>`;
  div.querySelector(".go").addEventListener("click", async () => {
    try { toast(await invoke("confirm_action", { id: a.id })); } catch (e) { toast(e); }
    actions.delete(a.id); div.remove();
  });
  div.querySelector(".no").addEventListener("click", () => { invoke("dismiss_action", { id: a.id }); actions.delete(a.id); div.remove(); });
  return div;
}

function renderActions(conv) {
  for (const a of actions.values()) {
    if (a.conv === conv && !$("chat").querySelector(`[data-action="${a.id}"]`)) {
      const live = liveMessage();
      if (live) $("chat").insertBefore(actionCard(a), live); else $("chat").appendChild(actionCard(a));
    }
  }
  $("chat").scrollTop = $("chat").scrollHeight;
}

listen("action", ({ payload: a }) => {
  actions.set(a.id, a);
  if (a.conv === active) renderActions(a.conv);
  else { unread.add(a.conv); renderConvList(); }
});
$("btn-trade-login").addEventListener("click", () => invoke("open_trade_window").catch(toast));
// ---- market cards: item image, price, ±% for your build, Travel ----
function badgeClass(p) {
  if (p >= 10) return "up2";
  if (p > 0) return "up1";
  if (p > -10) return "flat";
  if (p > -30) return "down1";
  return "down2";
}

function renderMarket(m) {
  const wrap = document.createElement("div");
  wrap.className = "market";
  wrap.innerHTML = `<div class="market-head">Market · ± vs ${escapeHtml(m.compared_to)}</div><div class="market-grid"></div>`;
  const grid = wrap.querySelector(".market-grid");
  for (const c of m.cards) {
    const pct = Math.round(c.delta_pct);
    const card = document.createElement("div");
    card.className = "mcard";
    card.innerHTML = `
      <div class="mtop">
        ${c.icon ? `<img src="${escapeHtml(c.icon)}" alt="">` : "<div class='noimg'></div>"}
        <div class="mtitle"><div class="mname">${escapeHtml(c.name || c.base)}</div><div class="mbase">${escapeHtml(c.name ? c.base : "")}${c.item_level ? ` · ilvl ${c.item_level}` : ""}</div>
          <div class="mprice">${escapeHtml(c.price || "no price")}</div></div>
        <div class="badge ${badgeClass(pct)}">${pct > 0 ? "+" : ""}${pct}%</div>
      </div>
      <div class="mverdict">${escapeHtml(c.verdict || "")}</div>
      <ul class="mmods">${(c.mods || []).map(x => `<li>${escapeHtml(x)}</li>`).join("")}</ul>
      ${c.requires ? `<div class="mreq">Requires ${escapeHtml(c.requires)}</div>` : ""}
      <div class="mfoot"><span class="mseller">${escapeHtml(c.seller)}</span>
        ${c.instant_buyout ? `<button class="primary go">Travel</button>` : `<span class="hint">in person only</span>`}</div>`;
    const go = card.querySelector(".go");
    if (go) go.addEventListener("click", async () => {
      go.disabled = true;
      try { toast(await invoke("travel", { listingId: c.listing_id })); } catch (e) { toast(e); go.disabled = false; }
    });
    grid.appendChild(card);
  }
  return wrap;
}

function placeInChat(el) {
  const live = liveMessage();
  if (live) $("chat").insertBefore(el, live); else $("chat").appendChild(el);
  $("chat").scrollTop = $("chat").scrollHeight;
}

listen("market", ({ payload }) => {
  if (payload.conv >= 900000) return;
  if (payload.conv === active) placeInChat(renderMarket(payload.market));
  else { unread.add(payload.conv); renderConvList(); }
});

// ---- my gear (recorded with the record-equipped hotkey) ----
function renderGear(g) {
  const entries = Object.entries(g || {});
  $("gear").innerHTML = entries.length ? entries.map(([slot, text]) => {
    const lines = text.split("\n").filter(l => l && !l.startsWith("Item Class") && !l.startsWith("Rarity") && !l.startsWith("--"));
    return `<li><span class="slot">${escapeHtml(slot)}</span><span class="gname">${escapeHtml(lines.slice(0, 2).join(" · "))}</span><button class="ghost x" data-slot="${escapeHtml(slot)}" title="Forget">×</button></li>`;
  }).join("") : `<li class="hint">Hover worn gear, press ${escapeHtml(settings ? settings.hotkeys.record_equipped : "the record hotkey")}.</li>`;
  $("gear").querySelectorAll(".x").forEach(b => b.addEventListener("click", async () => {
    await invoke("forget_equipped", { slot: b.dataset.slot });
    renderGear(await invoke("equipped"));
  }));
}
listen("equipped", ({ payload }) => renderGear(payload));
// ---- build rating page ----
const RATING_MARKET_BASE = 900100;
let ratingState = null;

function gradeClass(g) {
  if (!g) return "g-none";
  return `g-${g[0].toUpperCase()}`;
}

function renderRating(snap) {
  ratingState = snap;
  const r = snap.rating;
  $("r-budget").value = snap.budget || "";
  $("r-auto").checked = !!snap.auto;
  $("r-run").disabled = !!snap.busy;
  $("r-grade").textContent = r ? r.grade : "?";
  $("r-grade").className = `grade big ${gradeClass(r && r.grade)}`;
  $("r-tip").textContent = r ? r.explanation : "Rate your build to see why.";
  $("r-summary").textContent = r ? r.summary : "Not rated yet.";
  $("r-meta").textContent = r ? `${r.stage} · level ${r.level} · score ${Math.round(r.score)}/100 · rated ${new Date(r.rated_at * 1000).toLocaleString()}` : "";
  $("r-expl").textContent = r ? r.explanation : "";
  $("r-cats").innerHTML = r ? r.categories.map(c => `
    <div class="r-cat tip"><span class="grade small ${gradeClass(c.grade)}">${escapeHtml(c.grade)}</span>
      <div><div class="n">${escapeHtml(c.name)}</div><div class="t">${escapeHtml(c.note)}</div></div></div>`).join("") : "";
  const recs = r ? r.recommendations : [];
  $("r-recs").innerHTML = recs.length ? "" : "<p class='hint'>None yet.</p>";
  recs.forEach((rec, i) => {
    const div = document.createElement("div");
    div.className = "r-rec";
    div.innerHTML = `<h3>${escapeHtml(rec.slot)} — ${escapeHtml(rec.title)}</h3><div class="why">${escapeHtml(rec.why)}</div>
      <div class="look">Look for: ${escapeHtml(rec.look_for)}</div><div class="r-cards" id="r-cards-${i}"></div>`;
    $("r-recs").appendChild(div);
    const cards = (snap.cards || {})[i];
    const holder = div.querySelector(".r-cards");
    if (cards) holder.appendChild(renderMarket(cards));
    else if (i < 3) holder.innerHTML = `<p class="hint">${snap.busy ? "Searching the market…" : "No market picks yet."}</p>`;
  });
}

$("r-run").addEventListener("click", () => {
  invoke("rate_build", { budget: $("r-budget").value.trim() || "10 exalted", auto: $("r-auto").checked });
});
listen("rating", ({ payload }) => renderRating(payload));
listen("rating-status", ({ payload }) => {
  $("r-status").textContent = payload.text || "";
  $("r-run").disabled = !!payload.busy;
});
listen("market", ({ payload }) => {
  if (payload.conv < RATING_MARKET_BASE) return;
  const i = payload.conv - RATING_MARKET_BASE;
  const holder = document.getElementById(`r-cards-${i}`);
  if (holder) { holder.innerHTML = ""; holder.appendChild(renderMarket(payload.market)); }
});
// ---- skills, supports, buttons & rotations ----
function renderSkills(snap) {
  const p = snap.plan;
  $("s-run").disabled = !!snap.busy;
  $("s-summary").textContent = p ? p.summary : "No setup yet.";
  $("s-meta").textContent = p ? `${p.stage} · level ${p.level}` : "";
  $("s-skills").innerHTML = p ? p.skills.map(s => `
    <div class="s-card">
      <div class="top"><span class="nm">${escapeHtml(s.name)}</span><span class="btnchip">${escapeHtml(s.button)}</span></div>
      <div class="role">${escapeHtml(s.role)}</div>
      <ol class="s-sups">${(s.supports || []).map((x, i) => `<li title="${escapeHtml(x.why)}"><span class="ord">${i + 1}.</span><span class="sn">${escapeHtml(x.name)}</span><span class="sw">${escapeHtml(x.why)}</span></li>`).join("")}</ol>
      ${s.notes ? `<div class="notes">${escapeHtml(s.notes)}</div>` : ""}
    </div>`).join("") : "";
  $("s-rotations").innerHTML = p ? p.rotations.map(r => `
    <div class="rot"><h3>${escapeHtml(r.situation)}</h3><ol>${r.steps.map(s => `<li>${escapeHtml(s)}</li>`).join("")}</ol></div>`).join("")
    : "<p class='hint'>None yet.</p>";
}
$("s-run").addEventListener("click", () => invoke("run_skills"));
listen("skills", ({ payload }) => renderSkills(payload));
listen("skills-status", ({ payload }) => { $("s-status").textContent = payload.text || ""; $("s-run").disabled = !!payload.busy; });
function sendAsk() {
  const text = $("ask-input").value.trim();
  if (!text || active === null) return;
  $("ask-input").value = "";
  invoke("ask", { conv: active, text });
}
$("ask-form").addEventListener("submit", ev => { ev.preventDefault(); sendAsk(); });
$("ask-input").addEventListener("keydown", ev => {
  if (ev.key === "Enter" && !ev.shiftKey) { ev.preventDefault(); sendAsk(); }
});
$("btn-next").addEventListener("click", () => invoke("what_next", { conv: active }));
$("btn-new-conv").addEventListener("click", async () => {
  const id = await invoke("new_conversation");
  await selectConv(id);
  $("ask-input").focus();
});
$("btn-delete-conv").addEventListener("click", async () => {
  if (active === null) return;
  try {
    await invoke("delete_conversation", { conv: active });
    const next = convs.find(c => c.id !== active);
    if (next) selectConv(next.id); else { active = null; $("chat").innerHTML = ""; }
  } catch (e) { toast(e); }
});
$("btn-overlay").addEventListener("click", () => invoke("toggle_overlay"));

$("btn-paste-item").addEventListener("click", async () => {
  let text = "";
  try { text = await navigator.clipboard.readText(); } catch (_) { }
  if (!text.startsWith("Item Class:")) {
    toast("Copy an item in game first (Ctrl+Alt+C).");
    return;
  }
  invoke("check_item_text", { conv: active, text });
});
// ---- build import ----
function renderImport(v) {
  const rows = v.stages.map(s => `<tr class="${s.chosen ? "chosen" : ""}">
      <td>${escapeHtml(s.title || "(untitled)")}</td><td><span class="stage-tag">${escapeHtml(s.stage)}</span></td>
      <td>${s.main_points}</td><td>${s.ascendancy_points}</td><td>${s.estimated_level}</td></tr>`).join("");
  $("import-result").innerHTML = `
    <h2 style="margin-top:14px">${escapeHtml(v.ascendancy || v.class_name || "Build")} · level ${v.level ?? "?"}</h2>
    ${v.warning ? `<div class="status err">${escapeHtml(v.warning)}</div>` : ""}
    <table><thead><tr><th>Spec</th><th>Stage</th><th>Passives</th><th>Asc.</th><th>≈ Lvl</th></tr></thead><tbody>${rows}</tbody></table>
    <p class="hint" style="margin-top:8px">★ = written for that stage</p>
    <div class="import-actions">
      <button id="btn-write" class="primary">Write to Build Planner</button>
    </div>`;
  $("btn-write").addEventListener("click", writeStages);
}

$("import-form").addEventListener("submit", async ev => {
  ev.preventDefault();
  const input = $("import-input").value.trim();
  if (!input) return;
  $("import-status").className = "status";
  $("import-status").textContent = "Importing…";
  try {
    const v = await invoke("import_build", { input });
    $("import-status").textContent = "";
    renderImport(v);
  } catch (e) {
    $("import-status").className = "status err";
    $("import-status").textContent = e;
  }
});

async function writeStages() {
  $("import-status").className = "status";
  $("import-status").textContent = "Writing…";
  try {
    const files = await invoke("write_stages");
    $("import-status").className = "status ok";
    $("import-status").textContent = `Wrote ${files.length} stages. Pick one in the game's Build Planner.`;
    refreshPlannerFiles();
  } catch (e) {
    $("import-status").className = "status err";
    $("import-status").textContent = e;
  }
}

async function refreshPlannerFiles() {
  const files = await invoke("planner_files");
  $("planner-files").innerHTML = files.map(f => `<li>${escapeHtml(f)}</li>`).join("") || "<li class='hint'>None yet.</li>";
}

// ---- settings ----
const HOTKEY_KEYS = ["item_check", "what_next", "ask", "toggle_overlay", "record_equipped", "move_overlay"];
function renderKeysSummary(s) {
  if (!$("keys-summary")) return;
  const k = s.hotkeys;
  $("keys-summary").innerHTML = [
    ["Item check", k.item_check], ["What next", k.what_next], ["Ask", k.ask], ["HUD overlay", k.toggle_overlay], ["Record equipped", k.record_equipped], ["Move HUD", k.move_overlay],
  ].map(([n, v]) => `<li><span>${n}</span><kbd>${escapeHtml(v)}</kbd></li>`).join("");
}

function fillSettings(s) {
  const f = $("settings-form");
  for (const key of HOTKEY_KEYS) f.elements[key].value = s.hotkeys[key];
  f.elements.league.value = s.league;
  f.elements.overlay_on_hotkey.checked = s.overlay_on_hotkey;
  f.elements.sound.checked = s.sound !== false;
  f.elements.volume.value = s.volume ?? 0.6;
  f.elements.input.value = s.input || "keyboard";
  renderKeysSummary(s);
}

$("settings-form").addEventListener("submit", async ev => {
  ev.preventDefault();
  const f = ev.target;
  // Keep every other setting (rating budget, HUD place…) as it is.
  const next = {
    ...settings,
    league: f.elements.league.value.trim() || "HC Forbidden Rites",
    overlay_on_hotkey: f.elements.overlay_on_hotkey.checked,
    hotkeys: Object.fromEntries(HOTKEY_KEYS.map(k => [k, f.elements[k].value.trim()])),
    sound: f.elements.sound.checked,
    volume: Number(f.elements.volume.value),
    input: f.elements.input.value,
  };
  const failed = await invoke("save_settings", { settings: next });
  settings = next;
  renderKeysSummary(next);
  $("settings-status").className = failed.length ? "status err" : "status ok";
  $("settings-status").textContent = failed.length ? `Couldn't register: ${failed.join("; ")}` : "Saved — hotkeys are live.";
});

document.querySelectorAll(".sound-tests button").forEach(b => b.addEventListener("click", () =>
  invoke("play_sound", { name: b.dataset.cue, volume: Number($("settings-form").elements.volume.value) })));

// ---- events + startup ----
listen("character", ({ payload }) => renderCharacter(payload));
listen("feed", ({ payload }) => { $("feed").prepend(feedItem(payload)); if (payload.kind === "death") toast(payload.text); });
listen("usage", ({ payload }) => renderUsage(payload));
listen("notice", ({ payload }) => toast(payload));
listen("item", () => toast("Checking item…"));
listen("open-conversation", ({ payload: id }) => {
  document.querySelector('.tab[data-tab="play"]').click();
  selectConv(id);
});
listen("imported", ({ payload }) => renderImport(payload));
listen("planner-files", () => refreshPlannerFiles());
$("btn-create-build-play").addEventListener("click", () => openShowcase());
listen("focus-chat", () => {
  document.querySelector('.tab[data-tab="play"]').click();
  $("ask-input").focus();
});

(async () => {
  const snap = await invoke("snapshot");
  settings = snap.settings;
  renderCharacter(snap.character);
  renderFeed(snap.feed);
  fillSettings(snap.settings);
  if (snap.imported) renderImport(snap.imported);
  refreshPlannerFiles();
  renderGear(await invoke("equipped"));
  renderRating(await invoke("rating_snapshot"));
  renderSkills(await invoke("skills_snapshot"));
  convs = snap.conversations;
  for (const a of await invoke("pending_actions")) actions.set(a.id, a);
  const first = convs.filter(c => !c.in_game).sort((a, b) => b.id - a.id)[0];
  if (first) await selectConv(first.id);
  else await selectConv(await invoke("new_conversation"));
  
})();