// Main window.
const $ = id => document.getElementById(id);
let settings = null;

/** A notification above everything (dialogs included). kind: "ok", "err" or "info". */
function toast(text, kind = "info") {
  const t = $("toast");
  t.textContent = String(text);
  t.className = `toast show ${kind}`;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => t.classList.remove("show"), kind === "err" ? 8000 : 5500);
}

$("toast").addEventListener("click", () => $("toast").classList.remove("show"));
$("open-dash").addEventListener("click", () => invoke("open_dashboard").catch(e => toast(`Couldn't open the dashboard: ${e}`, "err")));

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
  // Log times are "YYYY/MM/DD HH:MM:SS"; show the date too when it isn't today.
  const d = new Date(), today = `${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, "0")}/${String(d.getDate()).padStart(2, "0")}`;
  const when = item.time.startsWith(today) ? item.time.slice(11, 16) : `${item.time.slice(5, 10).replace("/", "-")} ${item.time.slice(11, 16)}`;
  li.innerHTML = `<time>${escapeHtml(when)}</time><span class="k-${item.kind}">${escapeHtml(item.text)}</span>`;
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

// "You · What next" for the player's messages, "Lifeline" for answers.
function speaker(kind, label) {
  if (kind.startsWith("user")) return label && label !== "Chat" ? `You · ${label}` : "You";
  return "Lifeline";
}

function addMessage(kind, label, html) {
  const div = document.createElement("div");
  div.className = `msg ${kind}`;
  div.innerHTML = `<div class="label">${escapeHtml(speaker(kind, label))}</div><div class="body">${html}</div>`;
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
    if (c.in_game) addMessage("ai", "Companion", renderMarkdown("Answers to your in-game hotkeys and HUD buttons land here."));
    else {
      const m = addMessage("ai", "Companion", `Ask anything about your character, or start with one of these:
        <div class="starters">${STARTERS.map(s => `<button class="ghost starter">${escapeHtml(s)}</button>`).join("")}</div>`);
      m.querySelectorAll(".starter").forEach(b => b.addEventListener("click", () => { $("ask-input").value = b.textContent; sendAsk(); }));
    }
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
    toast(e.text, "err");
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
    try { toast(await invoke("confirm_action", { id: a.id })); } catch (e) { toast(e, "err"); }
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
  const cards = [...m.cards].sort((a, b) => (b.delta_pct || 0) - (a.delta_pct || 0));
  const SHOWN = 8;
  wrap.innerHTML = `<div class="market-head">Market · ${cards.length} listing${cards.length === 1 ? "" : "s"} graded · ± vs ${escapeHtml(m.compared_to)}</div><div class="market-grid"></div>`;
  const grid = wrap.querySelector(".market-grid");
  cards.forEach((c, i) => {
    const pct = Math.round(c.delta_pct);
    const card = document.createElement("div");
    card.className = "mcard";
    card.innerHTML = `
      <div class="mtop2"><div class="mprice">${escapeHtml(c.price || "no price")}</div>
        <div class="badge ${badgeClass(pct)}">${pct > 0 ? "+" : ""}${pct}%</div></div>
      ${itemCard(listingItem(c))}
      <div class="mverdict">${escapeHtml(c.verdict || "")}</div>
      <div class="mfoot"><span class="mseller">${escapeHtml(c.seller)}</span>
        ${c.instant_buyout ? `<button class="primary go">Travel</button>` : `<span class="hint">in person only</span>`}</div>`;
    const go = card.querySelector(".go");
    if (go) go.addEventListener("click", async () => {
      go.disabled = true;
      try { toast(await invoke("travel", { listingId: c.listing_id, searchId: c.search_id || null }), "ok"); }
      catch (e) { toast(e, "err"); go.disabled = false; }
    });
    if (i >= SHOWN) card.classList.add("hidden");
    grid.appendChild(card);
  });
  if (cards.length > SHOWN) {
    const more = document.createElement("button");
    more.className = "ghost market-more";
    more.textContent = `Show all ${cards.length}`;
    more.addEventListener("click", () => {
      const open = more.dataset.open === "1";
      grid.querySelectorAll(".mcard").forEach((el, i) => el.classList.toggle("hidden", open && i >= SHOWN));
      more.dataset.open = open ? "" : "1";
      more.textContent = open ? `Show all ${cards.length}` : `Show top ${SHOWN}`;
    });
    wrap.appendChild(more);
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

// ---- my gear (recorded with the record-equipped hotkey or Ctrl+C mode) ----
function openGearChecklist() {
  document.querySelector('.tab[data-tab="play"]').click();
  setTimeout(() => $("gear-card").scrollIntoView({ behavior: "smooth", block: "center" }), 200);
}
// Slot names a player recognizes, from the game's item classes.
const SLOT_NAME = { "Rings": "Ring 1", "Rings (2)": "Ring 2", "Body Armours": "Body armour", "Helmets": "Helmet", "Amulets": "Amulet", "Belts": "Belt", "Shields": "Off-hand", "Bucklers": "Off-hand", "Quivers": "Off-hand", "Foci": "Off-hand" };
let GEAR = {};
function renderGear(g) {
  GEAR = g || {};
  const entries = Object.entries(GEAR);
  $("gear-count").textContent = `${entries.length} of 10 slots`;
  $("gear").classList.add("item-grid");
  $("gear").innerHTML = entries.map(([slot, text]) =>
    `<li><div class="slot-row"><span class="slot-l">${escapeHtml(SLOT_NAME[slot] || slot)}</span><button class="ghost x" data-slot="${escapeHtml(slot)}" title="Forget this item">×</button></div>${itemCardFromText(text, { equipped: true })}</li>`
  ).join("");
  renderGearHow();
  $("gear").querySelectorAll(".x").forEach(b => b.addEventListener("click", async () => {
    await invoke("forget_equipped", { slot: b.dataset.slot });
    renderGear(await invoke("equipped"));
  }));
}
listen("equipped", ({ payload }) => renderGear(payload));

// How to record: Ctrl+C mode (the game's own copy) or the Record gear hotkey.
function renderGearHow() {
  const key = settings?.hotkeys?.record_equipped || "Alt+Shift+E";
  const on = typeof GEAR_WATCH !== "undefined" && GEAR_WATCH > 0;
  const until = on ? new Date(Date.now() + GEAR_WATCH * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : "";
  $("gear-how").innerHTML = `
    <button class="${on ? "" : "primary"}" id="gear-watch-play">${on ? `Stop recording (on until ${until})` : "Record my gear"}</button>
    <div class="hint">${on
      ? "Now in game: open your inventory (<kbd>I</kbd>), point at each worn item and press <kbd>Ctrl+C</kbd>. Each one appears here."
      : `Turn on, then in game point at each worn item and press <kbd>Ctrl+C</kbd>. Or any time: point at an item and press <kbd>${escapeHtml(key)}</kbd>.`}</div>`;
  $("gear-watch-play").addEventListener("click", async () => {
    try {
      GEAR_WATCH = await invoke("gear_watch", { on: !on });
      toast(GEAR_WATCH ? "Recording for 10 minutes. In game, point at each worn item and press Ctrl+C." : "Stopped recording gear.", GEAR_WATCH ? "ok" : "info");
    } catch (e) { toast(e, "err"); }
    renderGearHow();
  });
}
listen("gear-watch", () => setTimeout(renderGearHow, 0));
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
  $("r-summary").textContent = r ? r.summary : "Not rated yet.";
  $("r-meta").textContent = r ? `${r.stage} · level ${r.level} · score ${Math.round(r.score)}/100 · rated ${new Date(r.rated_at * 1000).toLocaleString()}` : "";
  $("r-expl").textContent = r ? r.explanation : "";
  $("r-cats").innerHTML = r ? r.categories.map(c => `
    <div class="r-cat tip"><span class="grade small ${gradeClass(c.grade)}">${escapeHtml(c.grade)}</span>
      <div><div class="n">${escapeHtml(c.name)}</div><div class="t">${escapeHtml(c.note)}</div></div></div>`).join("") : "";
  renderPieces(r);
  const recs = r ? r.recommendations : [];
  $("r-recs").innerHTML = recs.length ? "" : `<p class='hint'>${r ? "No upgrades to buy right now." : "Press Rate: the upgrades that would raise your grade most show here."}</p>`;
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

// Every graded piece, grouped: gear slots, skills, defences, passives, flasks.
function renderPieces(r) {
  const pieces = r?.pieces || [];
  if (!pieces.length) {
    $("r-pieces").innerHTML = `<p class="hint">${r ? "Rate again to grade each slot, skill and defence." : "Rate to grade each slot, skill and defence."}</p>`;
    return;
  }
  const groups = [];
  for (const p of pieces) {
    let g = groups.find(x => x.name === p.group);
    if (!g) groups.push(g = { name: p.group, items: [] });
    g.items.push(p);
  }
  const unrecorded = pieces.filter(p => p.group === "Gear" && /not recorded/i.test(p.note)).length;
  const key = settings?.hotkeys?.record_equipped || "Alt+Shift+E";
  const nudge = unrecorded ? `<div class="r-nudge">${unrecorded} gear slot${unrecorded > 1 ? "s" : ""} not recorded, so ${unrecorded > 1 ? "they grade" : "it grades"} F. Record your gear on the Play tab, then Rate again.
    <button class="primary" id="r-gear-help">Record my gear</button></div>` : "";
  $("r-pieces").innerHTML = nudge + groups.map(g => `
    <div class="r-group"><h3>${escapeHtml(g.name)} <span class="hint">${g.items.length}</span></h3>
      <div class="r-pieces">${g.items.map(p => `
        <div class="r-piece${p.verified ? "" : " unverified"}">
          <span class="grade small ${gradeClass(p.grade)}">${escapeHtml(p.grade)}</span>
          <div class="body">
            <div class="n">${escapeHtml(p.name)}${p.verified ? "" : ` <span class="unv" title="The game doesn't show this; graded from the plan or missing data">unverified</span>`}</div>
            ${p.have ? `<div class="have">${escapeHtml(p.have)}</div>` : ""}
            <div class="t">${escapeHtml(p.note)}</div>
          </div>
        </div>`).join("")}
      </div></div>`).join("");
  $("r-gear-help")?.addEventListener("click", openGearChecklist);
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
  $("s-summary").textContent = p ? p.summary : "Press Suggest skills & rotations to get your buttons, supports and rotations for where you are.";
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
    : "<p class='hint'>Rotations for clearing and bosses show here after Suggest.</p>";
}
$("s-run").addEventListener("click", () => invoke("run_skills").catch(e => toast(e, "err")));
listen("skills", ({ payload }) => renderSkills(payload));
listen("skills-status", ({ payload }) => { $("s-status").textContent = payload.text || ""; $("s-run").disabled = !!payload.busy; });
const STARTERS = [
  "What should I upgrade first?",
  "Are my resistances OK for the next act?",
  "What can kill me in this act?",
  "Which support gems should I use?",
];

// The open conversation, starting one if none is open.
async function ensureConv() {
  if (active === null) await selectConv(await invoke("new_conversation"));
  return active;
}

async function sendAsk() {
  const text = $("ask-input").value.trim();
  if (!text) return;
  const conv = await ensureConv();
  if (convs.find(c => c.id === conv)?.busy) {
    toast("Still answering. Wait a moment, or press + New to ask in another chat.", "err");
    return;
  }
  $("ask-input").value = "";
  invoke("ask", { conv, text }).catch(e => { $("ask-input").value = text; toast(e, "err"); });
}
$("ask-form").addEventListener("submit", ev => { ev.preventDefault(); sendAsk(); });
$("ask-input").addEventListener("keydown", ev => {
  if (ev.key === "Enter" && !ev.shiftKey) { ev.preventDefault(); sendAsk(); }
});
$("btn-next").addEventListener("click", async () => invoke("what_next", { conv: await ensureConv() }).catch(e => toast(e, "err")));
$("btn-new-conv").addEventListener("click", async () => {
  const id = await invoke("new_conversation");
  await selectConv(id);
  $("ask-input").focus();
});
$("btn-delete-conv").addEventListener("click", () => {
  if (active === null) return;
  confirmClick($("btn-delete-conv"), "Sure? Delete", async () => {
    try {
      const gone = active;
      await invoke("delete_conversation", { conv: gone });
      const next = convs.find(c => c.id !== gone);
      if (next) selectConv(next.id); else selectConv(await invoke("new_conversation"));
    } catch (e) { toast(e, "err"); }
  });
});
$("btn-overlay").addEventListener("click", () => invoke("toggle_overlay")
  .then(() => toast("HUD switched. It only shows while Path of Exile 2 is open and in front.", "info"))
  .catch(e => toast(e, "err")));

$("btn-paste-item").addEventListener("click", async () => {
  const text = await invoke("clipboard_item").catch(() => null);
  if (!text) {
    toast("Copy an item first: in game, point at it and press Ctrl+C. Then click Check item.", "err");
    return;
  }
  invoke("check_item_text", { conv: await ensureConv(), text }).catch(e => toast(e, "err"));
});
// ---- build import ----
function renderImport(v) {
  const rows = v.stages.map(s => `<tr class="${s.chosen ? "chosen" : ""}">
      <td>${escapeHtml(s.title || "(untitled)")}</td><td><span class="stage-tag">${escapeHtml(s.stage)}</span></td>
      <td>${s.main_points}</td><td>${s.ascendancy_points}</td><td>${s.level_to >= 100 ? `${s.level_from}+` : `${s.level_from}–${s.level_to}`}</td></tr>`).join("");
  $("import-result").innerHTML = `
    <h2 style="margin-top:14px">${escapeHtml(v.ascendancy || v.class_name || "Build")} · level ${v.level ?? "?"}</h2>
    ${v.warning ? `<div class="status err">${escapeHtml(v.warning)}</div>` : ""}
    <table><thead><tr><th>Tree</th><th>Stage</th><th>Passives</th><th>Ascendancy pts</th><th>Levels</th></tr></thead><tbody>${rows}</tbody></table>
    <p class="hint" style="margin-top:8px">★ = written for that stage</p>
    <div class="import-actions">
      <button id="btn-write" class="primary">Send to game planner</button>
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

// ---- the game's Build Planner folder: grouped by build, delete with a confirm ----
function confirmClick(button, label, action) {
  if (button.dataset.armed) { action(); return; }
  button.dataset.armed = "1";
  const old = button.textContent;
  button.textContent = label;
  button.classList.add("danger");
  setTimeout(() => { delete button.dataset.armed; button.textContent = old; button.classList.remove("danger"); }, 3000);
}

async function refreshPlannerFiles() {
  const files = await invoke("planner_list");
  $("planner-hint").textContent = files.length ? `${files.length} files` : "";
  if (!files.length) { $("planner-files").innerHTML = "<p class='hint'>No builds in the game's planner yet. Follow a build (Showcase or My builds), then Send to game planner.</p>"; return; }
  // "Act 2 - Silverfist Companion…" → group by author and the build part of the name.
  const groups = new Map();
  for (const f of files) {
    const base = f.file.replace(/\.build$/, "");
    const cut = base.indexOf(" - ");
    const stage = cut > 0 ? base.slice(0, cut) : "";
    const build = cut > 0 ? base.slice(cut + 3) : base;
    const key = `${f.author || ""}|${build.slice(0, 16)}`;
    if (!groups.has(key)) groups.set(key, { build, author: f.author, files: [] });
    const g = groups.get(key);
    if (build.length > g.build.length) g.build = build;
    g.files.push({ ...f, stage });
  }
  $("planner-files").innerHTML = [...groups.values()].map((g, gi) => `
    <div class="pgroup">
      <div class="phead2"><b>${escapeHtml(g.build)}</b><span class="hint">${g.files.length} file${g.files.length > 1 ? "s" : ""}${g.author ? " · by " + escapeHtml(g.author) : ""}</span>
        <button class="primary follow-group" data-g="${gi}" title="Make this your current build in Lifeline">Follow</button><button class="ghost del-group" data-g="${gi}">Delete all</button></div>
      <ul>${g.files.map(f => `<li><span>${escapeHtml(f.stage || f.name)}</span><button class="ghost del-one" data-file="${escapeHtml(f.file)}" title="Delete ${escapeHtml(f.file)}">Delete</button></li>`).join("")}</ul>
    </div>`).join("");
  const list = [...groups.values()];
  const remove = async names => {
    try { await invoke("delete_planner_files", { files: names }); toast(`Deleted ${names.length} file${names.length > 1 ? "s" : ""}.`); }
    catch (e) { toast(e, "err"); }
    refreshPlannerFiles();
  };
  $("planner-files").querySelectorAll(".follow-group").forEach(b => b.addEventListener("click", async () => {
    try {
      const v = await invoke("follow_planner", { files: list[b.dataset.g].files.map(f => f.file) });
      toast(`✓ Now following ${v.name}.`, "ok");
      showSub("current");
    } catch (e) { toast(`Couldn't follow that build: ${e}`, "err"); }
  }));
  $("planner-files").querySelectorAll(".del-one").forEach(b => b.addEventListener("click", () => confirmClick(b, "Sure?", () => remove([b.dataset.file]))));
  $("planner-files").querySelectorAll(".del-group").forEach(b => b.addEventListener("click", () =>
    confirmClick(b, `Delete ${list[b.dataset.g].files.length}?`, () => remove(list[b.dataset.g].files.map(f => f.file)))));
}
$("planner-open").addEventListener("click", () => invoke("open_planner_folder").catch(e => toast(e, "err")));
$("planner-refresh").addEventListener("click", refreshPlannerFiles);

// ---- settings ----
// Sound cues, each on or off; level up and death start off.
const CUES = ["level_up", "new_act", "boss_area", "penalty", "death", "ready"];
const CUE_DEFAULTS = { level_up: false, new_act: true, boss_area: true, penalty: true, death: false, ready: true };
const HOTKEY_KEYS = ["item_check", "what_next", "ask", "toggle_overlay", "record_equipped", "move_overlay"];

function fillSettings(s) {
  const f = $("settings-form");
  for (const key of HOTKEY_KEYS) f.elements[key].value = s.hotkeys[key];
  f.elements.league.value = s.league;
  f.elements.overlay_on_hotkey.checked = s.overlay_on_hotkey;
  f.elements.sound.checked = s.sound !== false;
  f.elements.volume.value = s.volume ?? 0.6;
  f.elements.input.value = s.input || "keyboard";
  f.elements.text_size.value = s.text_size || "auto";
  applyTextSize(s.text_size);
  for (const c of CUES) f.elements["cue_" + c].checked = (s.sound_cues || {})[c] ?? CUE_DEFAULTS[c];
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
    text_size: f.elements.text_size.value,
    sound_cues: Object.fromEntries(CUES.map(c => [c, f.elements["cue_" + c].checked])),
  };
  try {
    const failed = await invoke("save_settings", { settings: next });
    settings = next;
    $("settings-status").className = failed.length ? "status err" : "status ok";
    $("settings-status").textContent = failed.length ? `Couldn't use: ${failed.join("; ")}. Pick another combo.` : "✓ Saved.";
  } catch (e) {
    $("settings-status").className = "status err";
    $("settings-status").textContent = `Couldn't save: ${e}`;
  }
});

// Hotkey boxes record the combo you press (no typing, no typos).
for (const key of HOTKEY_KEYS) {
  const input = $("settings-form").elements[key];
  input.readOnly = true;
  input.placeholder = "Click, then press keys";
  input.addEventListener("focus", () => { input.dataset.was = input.value; input.value = ""; input.placeholder = "Press the combo now…"; });
  input.addEventListener("blur", () => { if (!input.value) input.value = input.dataset.was || ""; input.placeholder = "Click, then press keys"; });
  input.addEventListener("keydown", ev => {
    if (ev.key === "Tab") return;
    ev.preventDefault();
    if (ev.key === "Escape") { input.value = input.dataset.was || ""; input.blur(); return; }
    if (["Control", "Alt", "Shift", "Meta"].includes(ev.key)) return;
    const mods = [ev.ctrlKey && "Ctrl", ev.altKey && "Alt", ev.shiftKey && "Shift"].filter(Boolean);
    if (!mods.length) { $("settings-status").className = "status err"; $("settings-status").textContent = "Use Ctrl, Alt or Shift with the key so the game doesn't get it too."; return; }
    const name = ev.code.startsWith("Key") ? ev.code.slice(3) : ev.code.startsWith("Digit") ? ev.code.slice(5) : ev.key.length === 1 ? ev.key.toUpperCase() : ev.code;
    input.value = [...mods, name].join("+");
    input.dispatchEvent(new Event("change", { bubbles: true }));
    input.blur();
  });
}

// ---- text size: the whole app scales; Auto grows with the window ----
let TEXT_SIZE = "auto";
function applyTextSize(size) {
  TEXT_SIZE = size || "auto";
  const w = window.innerWidth;
  const zoom = { normal: 1, large: 1.2, xlarge: 1.4 }[TEXT_SIZE] ?? (w >= 2300 ? 1.3 : w >= 1700 ? 1.15 : 1);
  document.documentElement.style.zoom = zoom;
}
applyTextSize("auto");
window.addEventListener("resize", () => { if (TEXT_SIZE === "auto") applyTextSize("auto"); });
$("settings-form").elements.text_size.addEventListener("change", ev => applyTextSize(ev.target.value));
// Every change saves on its own (hotkey boxes save when you leave them).
$("settings-form").addEventListener("change", ev => {
  if (!settings) return;
  $("settings-form").requestSubmit();
});

document.querySelectorAll(".sound-tests button").forEach(b => b.addEventListener("click", () =>
  invoke("play_sound", { name: b.dataset.cue, volume: Number($("settings-form").elements.volume.value) })));

// ---- events + startup ----
listen("character", ({ payload }) => renderCharacter(payload));
listen("feed", ({ payload }) => { $("feed").prepend(feedItem(payload)); if (payload.kind === "death") toast(payload.text); });
listen("usage", ({ payload }) => renderUsage(payload));
listen("feed-all", ({ payload }) => renderFeed(payload));
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
  if (snap.hotkey_errors?.length) toast(snap.hotkey_errors.join(" "), "err");
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