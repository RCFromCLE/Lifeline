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
  pill.className = `pill ${penaltyClass(c.res_penalty)}`;
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
      ? "Hotkey answers land here: item checks and what-next."
      : "New conversation. Other open conversations are shared with me as context, so you can refer to them."));
  }
  for (const m of c.messages) {
    const div = addMessage(m.role === "user" ? "user" : "ai", m.label, renderMarkdown(m.text));
    if (m.error) div.classList.add("error");
  }
  renderLive(id);
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
    toast("Copy an item in game first (Ctrl+Alt+C), or use the item-check hotkey while hovering it.");
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
    <table><thead><tr><th>PoB tree spec</th><th>Stage</th><th>Passives</th><th>Ascendancy</th><th>≈ Level</th></tr></thead><tbody>${rows}</tbody></table>
    <p class="hint" style="margin-top:8px">★ = the spec written for that stage. Skill sets: ${v.skill_sets.map(escapeHtml).join(", ") || "none"} · Item sets: ${v.item_sets.map(escapeHtml).join(", ") || "none"}</p>
    <div class="import-actions">
      <button id="btn-write" class="primary">Write stages to the in-game Build Planner</button>
      <span class="hint" style="margin:0">One .build file per ★ stage, named "Act 1 - ${escapeHtml(v.name)}" …</span>
    </div>`;
  $("btn-write").addEventListener("click", writeStages);
}

$("import-form").addEventListener("submit", async ev => {
  ev.preventDefault();
  const input = $("import-input").value.trim();
  if (!input) return;
  $("import-status").className = "status";
  $("import-status").textContent = "Importing… (first import also downloads GGG's passive tree data)";
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
    $("import-status").textContent = `Wrote ${files.length} files: ${files.join(", ")}. In game: open the Build Planner and pick the stage for your character.`;
    refreshPlannerFiles();
  } catch (e) {
    $("import-status").className = "status err";
    $("import-status").textContent = e;
  }
}

async function refreshPlannerFiles() {
  const files = await invoke("planner_files");
  $("planner-files").innerHTML = files.map(f => `<li>${escapeHtml(f)}</li>`).join("") || "<li class='hint'>No .build files yet.</li>";
}

// ---- settings ----
function renderKeysSummary(s) {
  const k = s.hotkeys;
  $("keys-summary").innerHTML = [
    ["Item check", k.item_check], ["What next", k.what_next], ["Ask", k.ask], ["HUD overlay", k.toggle_overlay],
  ].map(([n, v]) => `<li><span>${n}</span><kbd>${escapeHtml(v)}</kbd></li>`).join("");
}

function fillSettings(s) {
  const f = $("settings-form");
  for (const key of ["item_check", "what_next", "ask", "toggle_overlay"]) f.elements[key].value = s.hotkeys[key];
  f.elements.league.value = s.league;
  f.elements.overlay_on_hotkey.checked = s.overlay_on_hotkey;
  renderKeysSummary(s);
}

$("settings-form").addEventListener("submit", async ev => {
  ev.preventDefault();
  const f = ev.target;
  const next = {
    league: f.elements.league.value.trim() || "HC Forbidden Rites",
    overlay_on_hotkey: f.elements.overlay_on_hotkey.checked,
    hotkeys: Object.fromEntries(["item_check", "what_next", "ask", "toggle_overlay"].map(k => [k, f.elements[k].value.trim()])),
  };
  const failed = await invoke("save_settings", { settings: next });
  settings = next;
  renderKeysSummary(next);
  $("settings-status").className = failed.length ? "status err" : "status ok";
  $("settings-status").textContent = failed.length ? `Couldn't register: ${failed.join("; ")}` : "Saved — hotkeys are live.";
});

// ---- events + startup ----
listen("character", ({ payload }) => renderCharacter(payload));
listen("feed", ({ payload }) => { $("feed").prepend(feedItem(payload)); if (payload.kind === "death") toast(payload.text); });
listen("usage", ({ payload }) => renderUsage(payload));
listen("notice", ({ payload }) => toast(payload));
listen("item", () => toast("Item copied from the game — asking the gear appraiser…"));
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
  convs = snap.conversations;
  const first = convs.filter(c => !c.in_game).sort((a, b) => b.id - a.id)[0];
  if (first) await selectConv(first.id);
  else await selectConv(await invoke("new_conversation"));
  toast(`Hover an item in game and press ${snap.settings.hotkeys.item_check} to check it; ${snap.settings.hotkeys.what_next} for next steps.`);
})();