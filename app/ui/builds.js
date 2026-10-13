// Builds tab: guided showcase (class → ascendancy → priorities → builds),
// My builds (saved ideas and created builds), creating a full build, and the
// build being followed (passives against the plan, skills for the stage).
// Uses $, invoke, listen, escapeHtml, toast, renderImport from the other scripts.

let ART = {};
let SHOW = null;
let LIB = [];
const W = { class: null, ascendancy: null, focus: "balanced", styles: [], budget: "low", complexity: "any" };

const esc = s => escapeHtml(s ?? "");
const prefs = () => ({ ...W });

/** An image address safe inside CSS url('…'). */
const cssUrl = u => encodeURI(u || "").replace(/'/g, "%27");

/** A class or ascendancy portrait cropped from GGG's art atlas. */
function portrait(cls, asc, size) {
  const a = ART[cls];
  const f = a && (a.frames[asc || ""] || a.frames[""]);
  if (!f) return `<div class="portrait" style="width:${size}px;height:${size}px"></div>`;
  const sx = size / f.w, sy = size / f.h;
  return `<div class="portrait" style="width:${size}px;height:${size}px;background-image:url('${esc(cssUrl(a.url))}');` +
    `background-size:${a.w * sx}px ${a.h * sy}px;background-position:-${f.x * sx}px -${f.y * sy}px"></div>`;
}

const COMPLEXITY = ["Very simple", "Simple", "Moderate", "Demanding", "Hard"];
const BUDGET = ["Shoestring", "Cheap", "Moderate", "Pricey", "Wealthy"];
/** Plain-text facts about a build: playstyle, damage, cost, difficulty. */
function tags(a, extra = []) {
  const r = a.ratings;
  const facts = [
    ...a.style, ...a.damage.slice(0, 3),
    BUDGET[r.budget - 1], COMPLEXITY[r.complexity - 1],
    a.league_start ? "League start" : "",
  ].filter(Boolean).map(esc);
  return `${extra.length ? `<div class="good-at">${extra.join("")}</div>` : ""}<div class="facts">${facts.join(" · ")}</div>`;
}
function bars(r) {
  const row = (label, v, cls = "") => `<span>${label}</span><div class="b"><i class="${cls}" style="width:${v * 20}%"></i></div>`;
  return `<div class="bars">${row("Damage", r.damage)}${row("Tanky", r.tankiness)}${row("Clear", r.clear_speed)}` +
    `${row("Bossing", r.bossing)}${row("HC safety", r.hardcore, "hc")}</div>`;
}

// ---- versions: when the guide was published and for which patch ----
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
/** "2026-09-14" → "Sep 14, 2026"; "2026-09" → "Sep 2026". */
function fmtDate(s) {
  const [y, m, d] = (s || "").split("-").map(Number);
  if (!y || !m) return s || "";
  return d ? `${MONTHS[m - 1]} ${d}, ${y}` : `${MONTHS[m - 1]} ${y}`;
}
const patchKey = v => (v || "").split(".").slice(0, 2).map(Number);
/** True when the guide's patch is older than the newest patch in the catalog. */
function olderPatch(v) {
  const latest = SHOW?.latest_patch;
  if (!v || !latest) return false;
  const [a, b] = patchKey(v), [c, d] = patchKey(latest);
  return a < c || (a === c && b < d);
}
function versionLine(a) {
  const v = a.versions?.[0];
  if (!v) return a.added ? `<div class="ver">Patch unknown · added ${esc(fmtDate(a.added))}</div>` : "";
  const old = olderPatch(v.game_version);
  return `<div class="ver${old ? " old" : ""}" title="${v.inferred ? "Patch worked out from the date" : "Patch stated by the guide"}">` +
    `For ${esc(v.game_version)}${v.inferred ? "*" : ""} · ${esc(fmtDate(v.date))} · v${a.versions.length}${old ? " · older patch" : ""}</div>`;
}
function versionsSection(a) {
  const vs = a.versions || [];
  if (!vs.length && !a.added) return "";
  const rows = vs.map((v, i) => `<tr><td><b>v${vs.length - i}</b></td><td>${esc(v.game_version)}${v.inferred ? ` <small title="Worked out from the date">(from date)</small>` : ""}</td>
      <td>${esc(fmtDate(v.date))}</td><td>${esc(v.note)}</td>
      <td>${v.url ? `<button class="link" data-url="${esc(v.url)}" title="${esc(v.url)}">${esc(hostOf(v.url))}</button>` : ""}</td></tr>`).join("");
  return `<div class="d-sec versions"><h3>Versions</h3>
    ${vs.length ? `<table><thead><tr><th>Version</th><th>Game</th><th>Published</th><th>What</th><th></th></tr></thead><tbody>${rows}</tbody></table>` : `<p class="hint">No dated guide version found.</p>`}
    ${a.added ? `<p class="hint">Added to Lifeline ${esc(fmtDate(a.added))}.</p>` : ""}</div>`;
}

function buildCard(a, { score, reasons = [], state } = {}) {
  const extra = reasons.map(why => `<span class="${/Risky|expensive/.test(why) ? "bad" : "good"}">${esc(why)}</span>`);
  return `<div class="top">${portrait(a.class, a.ascendancy, 64)}<div>
      ${state ? `<div class="state">${esc(state)}</div>` : ""}
      <div class="nm">${esc(a.name)}</div><div class="sub2">${esc(a.class)} · ${esc(a.ascendancy)} · ${esc(a.main_skill)}</div></div>
      ${score !== undefined ? `<span class="match" title="How well it fits your picks">${score}<small>% fit</small></span>` : ""}</div>
    ${versionLine(a)}<div class="sum">${esc(a.summary)}</div>${bars(a.ratings)}${tags(a, extra)}`;
}

// ---- showcase ----
/** The log names an ascended character by its ascendancy; map it to its class. */
function classOf(name) {
  if (!SHOW || !name) return { cls: name, asc: "" };
  if (SHOW.classes.some(k => k.name === name)) return { cls: name, asc: "" };
  const k = SHOW.classes.find(k => k.ascendancies.some(a => a.name === name));
  return k ? { cls: k.name, asc: name } : { cls: name, asc: "" };
}

/** The player's class (or ascendancy) art as a soft backdrop behind the whole app. */
function setBackdrop(name) {
  const { cls, asc } = classOf(name);
  const a = ART[cls];
  const f = a && (a.frames[asc] || a.frames[""]);
  if (!f) return;
  const size = 720, sx = size / f.w, sy = size / f.h;
  const root = document.documentElement.style;
  root.setProperty("--art-url", `url('${cssUrl(a.url)}')`);
  root.setProperty("--art-size", `${a.w * sx}px ${a.h * sy}px`);
  root.setProperty("--art-pos", `-${f.x * sx}px -${f.y * sy}px`);
}

// ---- search: typing filters every build, whatever the picks above ----
let ALL = null; // every build, ranked by the current priorities
const searchText = () => $("w-search").value.trim().toLowerCase();

/** "no X", "without X" and "-X" hide builds that use X; every other word must match. */
function parseQuery(q, list) {
  const want = [], avoid = [];
  const words = q.split(/\s+/).filter(Boolean);
  for (let i = 0; i < words.length; i++) {
    const w = words[i];
    if (["no", "without", "not"].includes(w) && words[i + 1]) avoid.push(words[++i]);
    else if (w.startsWith("-") && w.length > 1) avoid.push(w.slice(1));
    else want.push(w);
  }
  const lower = xs => [...new Set(xs.map(x => (x || "").toLowerCase()).filter(Boolean))];
  return { want, avoid, classes: lower(list.map(a => a.class)), ascs: lower(list.map(a => a.ascendancy)) };
}

/** True when `text` holds the word (a trailing plural "s" is optional). */
const has = (text, w) => text.includes(w) || (w.length > 3 && w.endsWith("s") && text.includes(w.slice(0, -1)));
// Element words match what the build deals, not every mention in its text.
const ELEMENTS = ["fire", "cold", "lightning", "chaos", "physical", "ignite", "poison", "bleed"];

function searchable(a) {
  const r = a.ratings || {};
  const gems = [...(a.skills || []), ...(a.leveling || []), ...(a.act_plan || []).flatMap(s => s.skills || [])].map(s => s.gem);
  const minion = (a.style || []).includes("minion");
  const traits = [
    r.tankiness >= 4 ? "tanky tank" : "", r.damage >= 4 ? "damage dps" : "", r.bossing >= 4 ? "bossing boss killer" : "",
    r.hardcore >= 4 ? "hardcore safe hc" : "", r.budget <= 2 ? "cheap budget" : "", r.complexity <= 2 ? "easy simple" : "",
    a.league_start ? "league start starter" : "", minion ? "summoner summons" : "",
    ...(a.versions || []).slice(0, 1).map(v => `${v.game_version} patch ${v.game_version}`),
  ];
  const low = xs => xs.filter(Boolean).join(" | ").toLowerCase();
  return {
    all: low([a.name, a.main_skill, a.summary, ...(a.style || []), ...(a.damage || []), ...(a.defences || []),
      ...gems, ...(a.keystones || []), ...(a.key_uniques || []), ...(a.ascendancy_passives || []), ...traits]),
    elements: low([a.name, a.main_skill, ...(a.damage || [])]),
    uses: low([...gems, ...(a.style || []), ...(a.damage || []), minion ? "summons" : ""]),
    cls: (a.class || "").toLowerCase(),
    asc: (a.ascendancy || "").toLowerCase(),
  };
}

function wordMatches(s, w, p) {
  const el = w.replace(/s$/, "");
  if (el.length >= 3 && ELEMENTS.some(e => e.startsWith(el))) return s.elements.includes(el);
  if (p.classes.includes(w)) return s.cls === w;
  if (w.length >= 3 && p.classes.some(c => c.startsWith(w))) return s.cls.startsWith(w);
  if (w.length >= 3 && p.ascs.some(c => c.startsWith(w))) return s.asc.startsWith(w);
  return has(s.all, w);
}

function matches(a, p) {
  const s = searchable(a);
  return p.want.every(w => wordMatches(s, w, p)) && !p.avoid.some(w => has(s.uses, w));
}

$("w-search").addEventListener("input", () => renderBuilds());

async function loadShowcase() {
  ALL = null;
  SHOW = await invoke("showcase", { prefs: prefs() });
  if (SHOW.character.class) setBackdrop(SHOW.character.class);
  renderClasses();
  renderAscs();
  renderBuilds();
}

function renderClasses() {
  const c = SHOW.character;
  $("w-char").textContent = c.name ? `${c.name}: ${c.class} level ${c.level}` : "";
  const mine = (classOf(c.class).cls || "").toLowerCase();
  $("w-classes").innerHTML = SHOW.classes.map(k => `<button class="art-card ${W.class === k.name ? "on" : ""} ${k.name.toLowerCase() === mine ? "mine" : ""}" data-c="${esc(k.name)}">
      ${portrait(k.name, "", 92)}<span class="nm">${esc(k.name)}</span><span class="ct">${k.builds} builds</span></button>`).join("");
  $("w-classes").querySelectorAll("button").forEach(b => b.addEventListener("click", () => {
    W.class = W.class === b.dataset.c ? null : b.dataset.c;
    W.ascendancy = null;
    loadShowcase();
  }));
}

function renderAscs() {
  const k = SHOW.classes.find(x => x.name === W.class);
  if (!k) {
    $("w-ascs").innerHTML = `<p class="hint">Pick a class, or browse every build below.</p>`;
    return;
  }
  const list = [{ name: "", builds: k.builds }, ...k.ascendancies];
  $("w-ascs").innerHTML = list.map(a => `<button class="art-card ${(W.ascendancy || "") === a.name ? "on" : ""}" data-a="${esc(a.name)}">
      ${portrait(k.name, a.name, 80)}<span class="nm">${esc(a.name || "Any")}</span><span class="ct">${a.builds} builds</span></button>`).join("");
  $("w-ascs").querySelectorAll("button").forEach(b => b.addEventListener("click", () => {
    W.ascendancy = b.dataset.a || null;
    loadShowcase();
  }));
}

async function renderBuilds() {
  const q = searchText();
  $("sub-showcase").classList.toggle("searching", !!q);
  $("w-pick").textContent = q ? "Matching builds" : "Pick a build";
  let list = SHOW.builds;
  if (q) {
    if (!ALL) ALL = (await invoke("showcase", { prefs: { ...W, class: null, ascendancy: null, styles: [] } })).builds;
    if (q !== searchText()) return; // a newer keystroke is already rendering
    const parsed = parseQuery(q, ALL.map(r => r.archetype));
    list = ALL.filter(r => matches(r.archetype, parsed));
  }
  $("w-count").textContent = SHOW.total ? `${list.length} of ${SHOW.total}` : "";
  if (!list.length) {
    $("w-builds").innerHTML = `<p class="hint">${q ? `No build matches “${esc(q)}”. Try fewer words.`
      : SHOW.total ? "Nothing matches. Loosen the picks above." : "The researched builds are still being added."}</p>`;
    return;
  }
  $("w-builds").innerHTML = list.map((r, i) => `<button class="bcard" data-i="${i}">${buildCard(r.archetype, r)}</button>`).join("");
  $("w-builds").querySelectorAll(".bcard").forEach(b => b.addEventListener("click", () => openBuild(list[b.dataset.i].archetype)));
}

document.querySelectorAll("#sub-showcase .chips").forEach(group => group.addEventListener("click", ev => {
  const b = ev.target.closest("button");
  if (!b) return;
  const key = group.dataset.key;
  if (group.classList.contains("multi")) {
    b.classList.toggle("on");
    W[key] = [...group.querySelectorAll(".on")].map(x => x.dataset.v);
  } else {
    group.querySelectorAll("button").forEach(x => x.classList.toggle("on", x === b));
    W[key] = b.dataset.v;
  }
  loadShowcase();
}));

let userPickedSub = false;
document.querySelectorAll("#subtabs .subtab").forEach(t => t.addEventListener("click", () => { userPickedSub = true; showSub(t.dataset.sub); }));
function showSub(name) {
  document.querySelectorAll("#subtabs .subtab").forEach(x => x.classList.toggle("active", x.dataset.sub === name));
  document.querySelectorAll("#view-build .sub").forEach(x => x.classList.toggle("active", x.id === `sub-${name}`));
  $("subtabs").parentElement.scrollTop = 0;
  if (name === "planner" && typeof refreshPlannerFiles === "function") refreshPlannerFiles();
}

/** Builds → Game planner, from anywhere. */
function openGamePlanner() {
  document.querySelector('.tab[data-tab="build"]').click();
  userPickedSub = true;
  showSub("planner");
}

// ---- details, generation, my builds ----
const hostOf = url => { try { return new URL(url).hostname.replace(/^www\./, ""); } catch (_) { return url; } };

function openModal(html) {
  $("modal-body").innerHTML = html;
  $("build-modal").classList.remove("hidden");
  $("modal-body").querySelectorAll(".link[data-url]").forEach(l =>
    l.addEventListener("click", () => invoke("open_url", { url: l.dataset.url }).catch(e => toast(e, "err"))));
  // Start at the top (name and portrait), not at the buttons at the bottom.
  MODAL_RETURN = document.activeElement;
  $("build-modal").querySelector(".modal-box").scrollTop = 0;
  const first = $("modal-body").querySelector("button.primary") || $("modal-x");
  first.focus({ preventScroll: true });
}
let MODAL_RETURN = null;
function closeModal() {
  if ($("build-modal").classList.contains("hidden")) return;
  $("build-modal").classList.add("hidden");
  MODAL_RETURN?.focus?.({ preventScroll: true });
}
$("modal-x").addEventListener("click", closeModal);
$("build-modal").addEventListener("click", ev => { if (ev.target === $("build-modal")) closeModal(); });
document.addEventListener("keydown", ev => { if (ev.key === "Escape") closeModal(); });

const skillLine = s => `<li><span class="gem">${esc(s.gem)}</span>${s.supports?.length ? ` <span class="sups">+ ${s.supports.map(esc).join(", ")}</span>` : ""}</li>`;
const list = (title, items, ordered = false) => items?.length
  ? `<div class="d-sec"><h3>${title}</h3><${ordered ? "ol" : "ul"}>${items.map(x => typeof x === "string" ? `<li>${esc(x)}</li>` : skillLine(x)).join("")}</${ordered ? "ol" : "ul"}></div>` : "";

// The campaign act by act: skills (from which level), companions,
// passives, ascendancy, gear and notes for each stage.
function actPlan(steps) {
  if (!steps?.length) return "";
  const skill = s => `<li><b>${esc(s.gem)}</b>${s.from_level ? ` <span class="ap-lvl">from level ${s.from_level}</span>` : ""}${s.supports?.length ? `<div class="ap-sup">${s.supports.map(esc).join(" · ")}</div>` : ""}${s.note ? `<div class="ap-note">${esc(s.note)}</div>` : ""}</li>`;
  return `<div class="d-sec act-plan"><h3>Act by act</h3>${steps.map((st, i) => `
    <details class="ap-step" ${i === 0 ? "open" : ""}>
      <summary><b>${esc(st.stage)}</b>${st.levels ? ` <span class="ap-lvl">levels ${esc(st.levels)}</span>` : ""}</summary>
      ${st.skills?.length ? `<div class="ap-row"><span class="ap-k">Skills</span><ul>${st.skills.map(skill).join("")}</ul></div>` : ""}
      ${st.companions ? `<div class="ap-row"><span class="ap-k">Companions</span><p>${esc(st.companions)}</p></div>` : ""}
      ${st.passives?.length ? `<div class="ap-row"><span class="ap-k">Passives</span><p>${st.passives.map(esc).join(" → ")}</p></div>` : ""}
      ${st.ascendancy?.length ? `<div class="ap-row"><span class="ap-k">Ascendancy</span><p>${st.ascendancy.map(esc).join(", ")}</p></div>` : ""}
      ${st.gear ? `<div class="ap-row"><span class="ap-k">Gear</span><p>${esc(st.gear)}</p></div>` : ""}
      ${st.notes ? `<div class="ap-row"><span class="ap-k">Notes</span><p>${esc(st.notes)}</p></div>` : ""}
    </details>`).join("")}</div>`;
}

function archetypeDetail(a) {
  return `<div class="d-head">${portrait(a.class, a.ascendancy, 128)}<div>
      <h1>${esc(a.name)}</h1><div class="sub2">${esc(a.class)} · ${esc(a.ascendancy)} · main skill ${esc(a.main_skill)}</div>
      ${tags(a)}</div></div>
    <p>${esc(a.summary)}</p>${bars(a.ratings)}
    ${actPlan(a.act_plan)}
    ${versionsSection(a)}
    ${a.boss_rotation?.length ? `<div class="d-sec boss-rot"><h3>Boss rotation</h3><ol>${a.boss_rotation.map(s => `<li>${esc(s)}</li>`).join("")}</ol></div>` : ""}
    <div class="d-grid">
      ${list("Endgame skills", a.skills)}
      ${list("Levelling", a.leveling)}
      ${list("Ascendancy order", a.ascendancy_passives, true)}
      ${list("Key passives", [...(a.keystones || []), ...(a.key_passives || [])])}
      ${list("Key uniques", a.key_uniques)}
      ${a.gear ? `<div class="d-sec"><h3>Gear</h3><p style="margin:0;font-size:13px">${esc(a.gear)}</p></div>` : ""}
      ${list("Strengths", a.strengths)}
      ${list("Weaknesses", a.weaknesses)}
      ${a.videos?.length ? `<div class="d-sec"><h3>Video guides</h3>${a.videos.map((v, i) => `<button class="link video" data-url="${esc(v)}">▶ Watch guide${a.videos.length > 1 ? " " + (i + 1) : ""}</button>`).join("")}</div>` : ""}
      <div class="d-sec sources"><h3>Sources</h3>${(a.sources || []).map(s => `<button class="link" data-url="${esc(s)}" title="${esc(s)}">${esc(hostOf(s))}</button>`).join("")}</div>
    </div>`;
}

function reportTable(report) {
  if (!report) return "";
  const rows = report.stages.map(s => `<tr><td>${esc(s.stage)}</td><td>${s.level}</td><td>${s.points_used}/${s.points_budget}</td>
      <td>${s.ascendancy_used}/${s.ascendancy_budget}</td><td>${esc(s.reached.join(", "))}</td><td>${esc(s.skills.join("; "))}</td></tr>`).join("");
  return `<div class="d-sec" style="margin-top:14px"><h3>Stages</h3><table><thead><tr><th>Stage</th><th>Lvl</th><th>Points</th><th>Asc.</th><th>New passives</th><th>Skills</th></tr></thead><tbody>${rows}</tbody></table></div>`;
}

function openBuild(a) {
  const saved = LIB.some(e => !e.design && e.archetype?.id === a.id);
  openModal(`${archetypeDetail(a)}
    <div class="d-actions">
      <button class="primary" id="d-gen">Create full build</button>
      <button id="d-save" ${saved ? "disabled" : ""}>${saved ? "★ Saved" : "☆ Save to My builds"}</button>
    </div>`);
  $("d-save").addEventListener("click", async () => {
    LIB = await invoke("save_idea", { id: a.id });
    renderMine();
    $("d-save").textContent = "★ Saved";
    $("d-save").disabled = true;
  });
  $("d-gen").addEventListener("click", () => generate(a));
}

async function generate(a) {
  try {
    await invoke("generate_build", { id: a.id, prefs: prefs() });
  } catch (e) { toast(e, "err"); return; }
  openModal(`<div class="d-head">${portrait(a.class, a.ascendancy, 96)}<div><h1>${esc(a.name)}</h1>
      <div class="sub2">Designing Act 1 → Endgame. This takes a few minutes; you can close this and keep playing.</div></div></div>
      <ul class="steps" id="gen-steps"></ul>`);
}

listen("build-progress", ({ payload }) => {
  const ul = $("gen-steps");
  if (!ul) return;
  const li = document.createElement("li");
  li.textContent = payload.step;
  ul.appendChild(li);
  ul.scrollTop = ul.scrollHeight;
});
listen("build-done", ({ payload }) => {
  if (payload.ok) {
    toast(`${payload.entry.archetype?.name || "Build"} is ready in My builds.`);
    if (!$("build-modal").classList.contains("hidden")) openSaved(payload.entry);
  } else {
    toast(`Build failed: ${payload.error}`);
    const ul = $("gen-steps");
    if (ul) ul.insertAdjacentHTML("beforeend", `<li class="err">${esc(payload.error)}</li>`);
  }
});
listen("library", ({ payload }) => { LIB = payload; renderMine(); });
// Keep "your character" and the backdrop in step with the game log.
listen("character", ({ payload: c }) => {
  if (!SHOW) return;
  const changed = SHOW.character.class !== c.class || SHOW.character.name !== c.name;
  SHOW.character = { class: c.class, level: c.level, name: c.name };
  if (changed) { renderClasses(); if (c.class) setBackdrop(c.class); }
  else $("w-char").textContent = c.name ? `${c.name}: ${c.class} level ${c.level}` : "";
});

function openSaved(e) {
  const a = e.archetype;
  const full = !!e.design;
  openModal(`${a ? archetypeDetail(a) : `<h1>${esc(e.design?.name)}</h1>`}
    ${full ? `<div class="d-sec" style="margin-top:14px"><h3>Your build</h3><p style="margin:0">${esc(e.summary || e.design.summary)}</p></div>
      ${e.risks?.length ? list("Watch out for", e.risks) : ""}${reportTable(e.report)}` : ""}
    <div class="d-actions">
      ${full ? `<button class="primary" id="d-write">Follow + send to game planner</button><button id="d-use">Follow</button>`
             : `<button class="primary" id="d-gen">Create full build</button>`}
      <button class="ghost" id="d-del">Delete</button>
    </div>`);
  if (full) {
    const use = async write => {
      try {
        const r = await invoke("use_saved", { id: e.id, write });
        toast(write ? `✓ Saved to the game's Build Planner: ${r.files.length} stages. They appear in game right away; pick the stage for your level.` : "✓ Now following this build.", "ok");
        closeModal();
        showSub("current");
        if (write) refreshPlannerFiles();
      } catch (err) { toast(`Couldn't save to the Build Planner: ${err}`, "err"); }
    };
    $("d-write").addEventListener("click", () => use(true));
    $("d-use").addEventListener("click", () => use(false));
  } else {
    $("d-gen").addEventListener("click", () => generate(a));
  }
  $("d-del").addEventListener("click", () => confirmClick($("d-del"), "Sure? Delete", async () => {
    await deleteSaved([e.id]);
    closeModal();
  }));
}

// ---- My builds: open, delete one, or clear older copies of the same build ----
let ACTIVE_SAVED = null;
async function refreshActiveSaved() {
  try { ACTIVE_SAVED = await invoke("active_saved"); } catch (_) { ACTIVE_SAVED = null; }
  renderMine();
}
listen("imported", refreshActiveSaved);

async function deleteSaved(ids) {
  try {
    for (const id of ids) LIB = await invoke("remove_saved", { id });
    renderMine();
    toast(ids.length > 1 ? `✓ Deleted ${ids.length} builds.` : "✓ Build deleted.", "ok");
  } catch (err) { toast(`Couldn't delete: ${err}`, "err"); }
}

// Older entries of the same build and kind (newest first in LIB; the followed one is kept).
function olderCopies() {
  const seen = new Set(), extra = [];
  const key = e => `${e.archetype?.id || e.design?.name}|${e.design ? "full" : "idea"}`;
  const ordered = [...LIB].sort((x, y) => (y.id === ACTIVE_SAVED) - (x.id === ACTIVE_SAVED));
  for (const e of ordered) (seen.has(key(e)) ? extra.push(e.id) : seen.add(key(e)));
  return extra;
}

const when = s => s ? new Date(s * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" }) + " " +
  new Date(s * 1000).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" }) : "";

function renderMine() {
  $("mine-count").textContent = LIB.length ? `(${LIB.length})` : "";
  const extra = olderCopies();
  $("mine-tools").innerHTML = extra.length
    ? `<button class="ghost" id="mine-dedupe" title="Keeps the newest (or followed) copy of each build">Delete ${extra.length} older ${extra.length > 1 ? "copies" : "copy"}</button>` : "";
  if (extra.length) $("mine-dedupe").addEventListener("click", () =>
    confirmClick($("mine-dedupe"), `Sure? Delete ${extra.length}`, () => deleteSaved(extra)));
  if (!LIB.length) {
    $("mine").innerHTML = `<p class="hint">Star builds in the Showcase to compare them here, or create a full build.</p>`;
    return;
  }
  $("mine").innerHTML = LIB.map((e, i) => {
    const a = e.archetype || { name: e.design?.name, class: e.design?.class, ascendancy: e.design?.ascendancy, main_skill: "", summary: e.summary, ratings: {}, style: [], damage: [] };
    const following = e.id === ACTIVE_SAVED;
    const state = [following ? "▶ Following" : e.design ? "Full build" : "Idea", when(e.saved_at)].filter(Boolean).join(" · ");
    return `<div class="mine-item${following ? " is-followed" : ""}">
      <button class="bcard" data-i="${i}">${a.ratings?.damage ? buildCard(a, { state }) : `<div class="nm">${esc(a.name)}</div><div class="state">${esc(state)}</div>`}</button>
      <button class="card-del" data-i="${i}" title="Delete" aria-label="Delete ${esc(a.name)}">✕</button>
    </div>`;
  }).join("");
  $("mine").querySelectorAll(".bcard").forEach(b => b.addEventListener("click", () => openSaved(LIB[b.dataset.i])));
  $("mine").querySelectorAll(".card-del").forEach(b => b.addEventListener("click", ev => {
    ev.stopPropagation();
    confirmClick(b, "Delete?", () => deleteSaved([LIB[b.dataset.i].id]));
  }));
}

// The welcome's last step opens the guided showcase.
function openShowcase() {
  document.querySelector('.tab[data-tab="build"]').click();
  showSub("showcase");
}

(async () => {
  try { ART = await invoke("class_art"); } catch (_) { ART = {}; }
  // The Current card may have drawn before the art arrived.
  if (CURRENT) renderCurrent(CURRENT);
  LIB = await invoke("library");
  await refreshActiveSaved();
  await loadShowcase();
  const st = await invoke("build_status");
  if (st.busy) toast("A build is still being generated.");
})();

// ---- Current build: what Lifeline follows, and how the character lines up ----
let CURRENT = null;
function renderCurrent(v) {
  CURRENT = v;
  const f = $("following");
  if (!v) { f.classList.add("hidden"); return; }
  f.textContent = `Following: ${v.name}`;
  f.classList.remove("hidden");
  const asc = v.ascendancy || "", cls = v.class_name || classOf(asc).cls;
  const chosen = v.stages.filter(s => s.chosen).sort((a, b) => a.level_from - b.level_from);
  $("cur-card").innerHTML = `<div class="d-head">${portrait(cls, asc, 104)}<div class="grow">
      <div class="state">${esc(v.source)}</div><h1>${esc(v.name)}</h1>
      <div class="sub2">${esc(cls)}${asc ? " · " + esc(asc) : ""} · ${chosen.length} stages</div>
      <div class="cur-stages">${chosen.map(s => `<button class="cstage" data-stage="${esc(s.stage)}" title="Show this stage on the tree">${esc(s.stage)} <small>${s.level_to >= 100 ? `levels ${s.level_from}+` : `levels ${s.level_from}–${s.level_to}`}</small></button>`).join("")}</div></div></div>
    <div class="d-actions">
      <button class="primary" id="cur-write" title="Write this build's stages into the game's Build Planner">Send to game planner</button>
      <button id="cur-planner" title="See, follow or delete the builds in the game's Build Planner">Manage game planner</button>
      <button class="ghost" id="cur-change">Change build</button>
    </div>`;
  $("cur-planner").addEventListener("click", openGamePlanner);
  $("cur-write").addEventListener("click", async () => {
    try {
      const files = await invoke("write_stages");
      toast(`✓ Saved to the game's Build Planner: ${files.length} stages.`, "ok");
      refreshPlannerFiles();
    } catch (e) { toast(`Couldn't save to the Build Planner: ${e}`, "err"); }
  });
  $("cur-change").addEventListener("click", () => showSub("showcase"));
  // Stage chips show that stage on the tree.
  $("cur-card").querySelectorAll(".cstage").forEach(c => c.addEventListener("click", () => {
    refreshTree(c.dataset.stage);
    $("tree-card").scrollIntoView({ behavior: "smooth", block: "start" });
  }));
  refreshAlignment();
}

let alignTimer = null;
function refreshAlignment() {
  clearTimeout(alignTimer);
  alignTimer = setTimeout(async () => {
    if (!CURRENT) { $("align-card").classList.add("hidden"); $("tree-card").classList.add("hidden"); return; }
    const a = await invoke("build_alignment");
    $("align-card").classList.remove("hidden");
    if (!a.passives) { $("align-body").innerHTML = `<p class="hint">${esc(a.note || "")}</p>`; return; }
    $("align-stage").textContent = `${a.stage} plan · ${a.character.name || "character"} level ${a.character.level}`;
    document.querySelectorAll(".cstage").forEach(s => s.classList.toggle("on", s.dataset.stage === a.stage));
    const p = a.passives;
    $("align-body").innerHTML = `
      <div class="align-top"><div class="align-pct">${p.percent}<small>%</small></div>
        <div class="grow"><div class="bars" style="grid-template-columns:110px 1fr"><span>Passives</span><div class="b"><i class="hc" style="width:${p.percent}%"></i></div></div>
        <div class="hint" style="margin:6px 0 0">${p.allocated_of_plan} of ${p.planned} planned passives for ${esc(a.stage)} allocated · ${p.off_plan.length} off-plan</div></div></div>
      <div class="d-grid">
        <div class="d-sec"><h3>Take next</h3><ul>${p.missing.slice(0, 10).map(m => `<li class="${m.notable ? "notable" : ""}">${esc(m.name)}</li>`).join("") || "<li>Nothing — on plan</li>"}</ul>${p.missing.length > 10 ? `<div class="hint">+${p.missing.length - 10} more</div>` : ""}</div>
        <div class="d-sec"><h3>Off-plan passives</h3><ul>${p.off_plan.slice(0, 10).map(n => `<li>${esc(n)}</li>`).join("") || "<li>None</li>"}</ul></div>
        <div class="d-sec"><h3>Skills for this stage</h3><ul class="gem-list">${a.skills.map(s => `<li>${gemDot(s.attr)}<span class="gem">${esc(s.skill)}</span>${s.supports.length ? `<div class="sups">${s.supports.map((x, i) => `${gemDot(s.support_attrs?.[i])}${esc(x)}`).join(" ")}</div>` : ""}</li>`).join("") || "<li>—</li>"}</ul>
          <div class="hint">The game doesn't log gems, so this is the plan to socket.</div></div>
      </div>`;
    if (typeof refreshTree === "function") refreshTree();
  }, 250);
}

$("following").addEventListener("click", () => { document.querySelector('.tab[data-tab="build"]').click(); showSub("current"); });
$("align-refresh").addEventListener("click", refreshAlignment);
listen("imported", ({ payload }) => { const first = !CURRENT; renderCurrent(payload); if (first && !userPickedSub) showSub("current"); });
let alignThrottle = 0;
listen("character", () => { if (CURRENT && Date.now() - alignThrottle > 5000) { alignThrottle = Date.now(); refreshAlignment(); } });
(async () => {
  const snap = await invoke("snapshot");
  if (snap.imported) { renderCurrent(snap.imported); showSub("current"); }
})();