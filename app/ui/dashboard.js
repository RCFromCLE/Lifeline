// Playthrough dashboard (second screen): skills and their supports, the
// passives taken and still to take, and the live game log. tree.js draws the
// tree; this page feeds it (TREE, TPLAN) and renders everything around it.
const $ = id => document.getElementById(id);
const esc = s => escapeHtml(s ?? "");

let FOLLOWING = null;      // the followed build's name (null = none)
let FITTED_STAGE = null;   // stage the tree was last framed for

// ---- header ----
function renderHeader(c) {
  if (!c) return;
  $("d-name").textContent = c.name || "Waiting for the game log…";
  const where = c.zone ? `${c.zone} · area ${c.area_level}${c.act ? ` · Act ${c.act}` : ""}` : "";
  $("d-meta").textContent = [c.class, where, FOLLOWING ? `Following ${FOLLOWING}` : ""].filter(Boolean).join(" · ");
  $("d-level").textContent = c.name ? c.level : "—";
  const res = $("d-res");
  res.textContent = c.res_penalty === null || c.res_penalty === undefined ? "—" : `${c.res_penalty}%`;
  res.className = `v ${penaltyClass(c.res_penalty)}`;
  const deaths = $("d-deaths");
  deaths.textContent = c.deaths ?? 0;
  deaths.className = `v ${c.deaths ? "bad" : ""}`;
}

// ---- skills and supports ----
function renderSkills(a, coach) {
  const plan = coach?.plan;
  const coachFor = name => plan?.skills?.find(s => s.name?.toLowerCase() === name.toLowerCase());
  const whyFor = (cs, sup) => cs?.supports?.find(x => x.name?.toLowerCase() === sup.toLowerCase())?.why;
  let skills = (a?.skills || []).map(s => ({
    name: s.skill, attr: s.attr,
    supports: s.supports.map((n, i) => ({ name: n, attr: s.support_attrs?.[i] })),
  }));
  // No followed build: show the skill coach's suggestions instead.
  if (!skills.length && plan?.skills?.length) {
    skills = plan.skills.map(s => ({ name: s.name, attr: "", supports: (s.supports || []).map(x => ({ name: x.name, attr: "" })) }));
  }
  $("d-stage").textContent = a?.stage ? `${a.stage} plan` : plan?.stage ? `${plan.stage} · skill coach` : "";
  if (!skills.length) {
    $("d-skills").innerHTML = `<p class="hint">${esc(a?.note || "Follow a build in Lifeline, or press Suggest skills on the Skills tab.")}</p>`;
    $("d-skills-note").textContent = "";
    return;
  }
  $("d-skills").innerHTML = skills.map(s => {
    const cs = coachFor(s.name);
    const sups = s.supports.length
      ? s.supports.map(x => {
        const why = whyFor(cs, x.name);
        return `<li>${gemDot(x.attr)}<span class="sn">${esc(x.name)}</span>${why ? `<span class="sw">${esc(why)}</span>` : ""}</li>`;
      }).join("")
      : `<li class="empty">No supports planned</li>`;
    const border = (gemStyle(s.attr).match(/--gem:[^;]+/) || [""])[0];
    return `<div class="sk" style="${border}">
      <div class="sk-top">${gemDot(s.attr)}<span class="sk-name">${esc(s.name)}</span>
        ${cs?.button ? `<span class="btnchip">${esc(cs.button)}</span>` : ""}
        <span class="sk-count">${s.supports.length} support${s.supports.length === 1 ? "" : "s"}</span></div>
      ${cs?.role ? `<div class="sk-role">${esc(cs.role)}</div>` : ""}
      <ol class="sk-sups">${sups}</ol>
    </div>`;
  }).join("");
  $("d-skills-note").textContent = "The game doesn't log gems, so this is the plan to socket, not what's socketed.";
}

// ---- passives ----
function renderPassives(a) {
  const p = a?.passives;
  $("d-pct").textContent = p ? `${p.percent}%` : "—";
  $("d-pct").className = `v ${p ? (p.percent >= 90 ? "good" : p.percent < 50 ? "warn" : "") : ""}`;
  if (!TPLAN) {
    $("d-pcount").textContent = "";
    $("d-next").innerHTML = "";
    $("d-taken").innerHTML = `<p class="hint">${esc(a?.note || "Follow a build to see its passives.")}</p>`;
    $("d-asc").innerHTML = "";
    return;
  }
  const off = new Set(TPLAN.off_plan);
  $("d-pcount").textContent = `${TPLAN.allocated.length} taken · ${TPLAN.planned.length} planned for ${TPLAN.stage}${off.size ? ` · ${off.size} off-plan` : ""}`;

  const next = TPLAN.next;
  const ATTR_COLOR = { str: "#e58a6e", dex: "#8fd17a", int: "#79b2e8" };
  $("d-next").innerHTML = next.length
    ? next.slice(0, 18).map(n => `<li class="${n.notable ? "notable" : ""}">${n.attr ? `<span class="ac" style="color:${ATTR_COLOR[n.attr]}">${n.attr[0].toUpperCase()}</span>` : ""}${esc(n.name)}</li>`).join("")
      + (next.length > 18 ? `<li class="hint" style="list-style:none">+${next.length - 18} more</li>` : "")
    : `<li style="list-style:none" class="hint">Every planned passive for this stage is taken.</li>`;

  // Notables and keystones one by one; small passives grouped by name.
  const big = [], small = new Map();
  for (const s of TPLAN.allocated) {
    const pos = TREE_POS?.get(s);
    const kind = pos?.[2];
    const name = kind === 5 ? "Attribute" : pos?.[3] || TPLAN.details[s]?.name || "Passive";
    if (kind === 1 || kind === 2) big.push({ name, keystone: kind === 2, off: off.has(s) });
    else {
      const key = `${name}|${off.has(s)}`;
      const e = small.get(key) || { name, off: off.has(s), n: 0 };
      e.n++;
      small.set(key, e);
    }
  }
  big.sort((x, y) => (y.keystone - x.keystone) || x.name.localeCompare(y.name));
  const chips = [
    ...big.map(b => `<span class="pt big ${b.keystone ? "ks" : ""} ${b.off ? "off" : ""}">${esc(b.name)}</span>`),
    ...[...small.values()].sort((x, y) => y.n - x.n).map(e => `<span class="pt ${e.off ? "off" : ""}">${esc(e.name)}${e.n > 1 ? `<span class="x">×${e.n}</span>` : ""}</span>`),
  ];
  $("d-taken").innerHTML = chips.join("") || `<p class="hint">${TPLAN.allocated_seen ? "None yet." : "Nothing seen in the game log yet. Allocate a point in game and it shows here."}</p>`;
  $("d-asc").innerHTML = TPLAN.ascendancy.map(x => `<li class="${x.taken ? "done" : ""}">${x.taken ? "✓ " : ""}${esc(x.name)}</li>`).join("") || `<li>—</li>`;
}

// ---- refresh (debounced; game events arrive in bursts) ----
let refreshTimer = null, lastRefresh = 0;
function scheduleRefresh(minGap = 0) {
  clearTimeout(refreshTimer);
  const wait = Math.max(250, minGap - (Date.now() - lastRefresh));
  refreshTimer = setTimeout(refresh, wait);
}

async function refresh() {
  lastRefresh = Date.now();
  const [a, coach] = await Promise.all([
    invoke("build_alignment").catch(() => null),
    invoke("skills_snapshot").catch(() => null),
  ]);
  FOLLOWING = a?.build || null;
  TPLAN = null;
  if (a?.passives) {
    try {
      if (!TREE) {
        TREE = await invoke("tree_layout");
        TREE_POS = new Map(TREE.nodes.map(n => [n[0], [n[1], n[2], n[3], n[4]]]));
      }
      TPLAN = await invoke("tree_plan", { stage: null });
    } catch (e) {
      a.note = String(e);
    }
  }
  $("tree-view").classList.toggle("hidden", !TPLAN);
  if (TPLAN) {
    drawTree();
    if (FITTED_STAGE !== TPLAN.stage) { fitTree(); FITTED_STAGE = TPLAN.stage; }
  }
  renderSkills(a, coach);
  renderPassives(a);
  renderHeader(await invoke("snapshot").then(s => s.character).catch(() => null));
}

// ---- log ----
function feedLine(item) {
  const li = document.createElement("li");
  const d = new Date(), today = `${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, "0")}/${String(d.getDate()).padStart(2, "0")}`;
  const when = item.time.startsWith(today) ? item.time.slice(11, 16) : `${item.time.slice(5, 10).replace("/", "-")} ${item.time.slice(11, 16)}`;
  li.innerHTML = `<time>${esc(when)}</time><span class="k-${esc(item.kind)}">${esc(item.text)}</span>`;
  return li;
}
function renderFeed(items) {
  const ul = $("d-feed");
  ul.innerHTML = "";
  items.slice(-200).reverse().forEach(i => ul.appendChild(feedLine(i)));
}

// ---- full screen ----
async function toggleFullscreen() {
  const w = window.__TAURI__.window.getCurrentWindow();
  await w.setFullscreen(!(await w.isFullscreen()));
}
$("d-full").addEventListener("click", () => toggleFullscreen().catch(() => {}));
document.addEventListener("keydown", ev => {
  if (ev.key === "F11") { ev.preventDefault(); toggleFullscreen().catch(() => {}); }
});

let resizeTimer = null;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => { if (TPLAN) fitTree(); }, 200);
});

listen("character", ({ payload }) => { renderHeader(payload); scheduleRefresh(3000); });
listen("feed", ({ payload }) => {
  const ul = $("d-feed");
  ul.prepend(feedLine(payload));
  while (ul.children.length > 200) ul.lastChild.remove();
});
listen("feed-all", ({ payload }) => renderFeed(payload));
listen("imported", () => { FITTED_STAGE = null; scheduleRefresh(); });
listen("skills", () => scheduleRefresh());
listen("equipped", () => scheduleRefresh());

// tree.js loads after this file; start once every script has run.
document.addEventListener("DOMContentLoaded", async () => {
  const snap = await invoke("snapshot");
  FOLLOWING = snap.imported?.name || null;
  renderHeader(snap.character);
  renderFeed(snap.feed || []);
  refresh();
});
