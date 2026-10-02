// Builds tab: guided showcase (class → ascendancy → priorities → builds),
// My builds (saved ideas and generated builds), generating a full build.
// Uses $, invoke, listen, escapeHtml, toast, renderImport from the other scripts.

let ART = {};
let SHOW = null;
let LIB = [];
const W = { class: null, ascendancy: null, focus: "balanced", styles: [], budget: "low", complexity: "any" };

const esc = s => escapeHtml(s ?? "");
const prefs = () => ({ ...W });

/** A class or ascendancy portrait cropped from GGG's art atlas. */
function portrait(cls, asc, size) {
  const a = ART[cls];
  const f = a && (a.frames[asc || ""] || a.frames[""]);
  if (!f) return `<div class="portrait" style="width:${size}px;height:${size}px"></div>`;
  const sx = size / f.w, sy = size / f.h;
  return `<div class="portrait" style="width:${size}px;height:${size}px;background-image:url('${a.url}');` +
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

function buildCard(a, { score, reasons = [], state } = {}) {
  const extra = reasons.map(why => `<span class="${/Risky|expensive/.test(why) ? "bad" : "good"}">${esc(why)}</span>`);
  return `<div class="top">${portrait(a.class, a.ascendancy, 64)}<div>
      ${state ? `<div class="state">${esc(state)}</div>` : ""}
      <div class="nm">${esc(a.name)}</div><div class="sub2">${esc(a.class)} · ${esc(a.ascendancy)} · ${esc(a.main_skill)}</div></div>
      ${score !== undefined ? `<span class="match" title="How well it fits your picks">${score}<small>% fit</small></span>` : ""}</div>
    <div class="sum">${esc(a.summary)}</div>${bars(a.ratings)}${tags(a, extra)}`;
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
  root.setProperty("--art-url", `url('${a.url}')`);
  root.setProperty("--art-size", `${a.w * sx}px ${a.h * sy}px`);
  root.setProperty("--art-pos", `-${f.x * sx}px -${f.y * sy}px`);
}

async function loadShowcase() {
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

function renderBuilds() {
  const list = SHOW.builds;
  $("w-count").textContent = SHOW.total ? `${list.length} of ${SHOW.total}` : "";
  if (!list.length) {
    $("w-builds").innerHTML = `<p class="hint">${SHOW.total ? "Nothing matches. Loosen the picks above." : "The researched builds are still being added."}</p>`;
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

document.querySelectorAll("#subtabs .subtab").forEach(t => t.addEventListener("click", () => showSub(t.dataset.sub)));
function showSub(name) {
  document.querySelectorAll("#subtabs .subtab").forEach(x => x.classList.toggle("active", x.dataset.sub === name));
  document.querySelectorAll("#view-build .sub").forEach(x => x.classList.toggle("active", x.id === `sub-${name}`));
}

// ---- details, generation, my builds ----
function openModal(html) {
  $("modal-body").innerHTML = html;
  $("build-modal").classList.remove("hidden");
}
function closeModal() { $("build-modal").classList.add("hidden"); }
$("modal-x").addEventListener("click", closeModal);
$("build-modal").addEventListener("click", ev => { if (ev.target === $("build-modal")) closeModal(); });
document.addEventListener("keydown", ev => { if (ev.key === "Escape") closeModal(); });

const skillLine = s => `<li><span class="gem">${esc(s.gem)}</span>${s.supports?.length ? ` <span class="sups">+ ${s.supports.map(esc).join(", ")}</span>` : ""}</li>`;
const list = (title, items, ordered = false) => items?.length
  ? `<div class="d-sec"><h3>${title}</h3><${ordered ? "ol" : "ul"}>${items.map(x => typeof x === "string" ? `<li>${esc(x)}</li>` : skillLine(x)).join("")}</${ordered ? "ol" : "ul"}></div>` : "";

function archetypeDetail(a) {
  return `<div class="d-head">${portrait(a.class, a.ascendancy, 128)}<div>
      <h1>${esc(a.name)}</h1><div class="sub2">${esc(a.class)} · ${esc(a.ascendancy)} · main skill ${esc(a.main_skill)}</div>
      ${tags(a)}</div></div>
    <p>${esc(a.summary)}</p>${bars(a.ratings)}
    <div class="d-grid">
      ${list("Endgame skills", a.skills)}
      ${list("Levelling", a.leveling)}
      ${list("Ascendancy order", a.ascendancy_passives, true)}
      ${list("Key passives", [...(a.keystones || []), ...(a.key_passives || [])])}
      ${list("Key uniques", a.key_uniques)}
      ${a.gear ? `<div class="d-sec"><h3>Gear</h3><p style="margin:0;font-size:13px">${esc(a.gear)}</p></div>` : ""}
      ${list("Strengths", a.strengths)}
      ${list("Weaknesses", a.weaknesses)}
      <div class="d-sec sources"><h3>Sources</h3>${(a.sources || []).map(s => `<div>${esc(s)}</div>`).join("")}</div>
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
  } catch (e) { toast(e); return; }
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
      ${full ? `<button class="primary" id="d-write">Use it + write to Build Planner</button><button id="d-use">Use it</button>`
             : `<button class="primary" id="d-gen">Create full build</button>`}
      <button class="ghost" id="d-del">Delete</button>
    </div>`);
  if (full) {
    const use = async write => {
      try {
        const r = await invoke("use_saved", { id: e.id, write });
        toast(write ? `Active build set; wrote ${r.files.length} stages. Pick one in the game's Build Planner.` : "Active build set.");
        if (write) refreshPlannerFiles();
      } catch (err) { toast(err); }
    };
    $("d-write").addEventListener("click", () => use(true));
    $("d-use").addEventListener("click", () => use(false));
  } else {
    $("d-gen").addEventListener("click", () => generate(a));
  }
  $("d-del").addEventListener("click", async () => {
    LIB = await invoke("remove_saved", { id: e.id });
    renderMine();
    closeModal();
  });
}

function renderMine() {
  $("mine-count").textContent = LIB.length ? `(${LIB.length})` : "";
  if (!LIB.length) {
    $("mine").innerHTML = `<p class="hint">Star builds in the Showcase to compare them here, or create a full build.</p>`;
    return;
  }
  $("mine").innerHTML = LIB.map((e, i) => {
    const a = e.archetype || { name: e.design?.name, class: e.design?.class, ascendancy: e.design?.ascendancy, main_skill: "", summary: e.summary, ratings: {}, style: [], damage: [] };
    return `<button class="bcard" data-i="${i}">${a.ratings?.damage ? buildCard(a, { state: e.design ? "Full build" : "Idea" }) : esc(a.name)}</button>`;
  }).join("");
  $("mine").querySelectorAll(".bcard").forEach(b => b.addEventListener("click", () => openSaved(LIB[b.dataset.i])));
}

// Play tab's "Create build" opens the guided showcase.
function openShowcase() {
  document.querySelector('.tab[data-tab="build"]').click();
  showSub("showcase");
}

(async () => {
  try { ART = await invoke("class_art"); } catch (_) { ART = {}; }
  LIB = await invoke("library");
  renderMine();
  await loadShowcase();
  const st = await invoke("build_status");
  if (st.busy) toast("A build is still being generated.");
})();
