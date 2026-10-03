// Playthrough dashboard (second screen): skills and their supports, the
// next passives to take and those taken, and the live game log. Plain text:
// no tree drawing, no chips; hover a notable or keystone to see what it does.
const $ = id => document.getElementById(id);
const esc = s => escapeHtml(s ?? "");

let FOLLOWING = null;      // the followed build's name (null = none)
let TREE = null, TREE_POS = null;  // tree layout: node kinds and names
let TPLAN = null;                  // the followed build's plan for this stage
const NEXT_SHOWN = 8;              // the very next passives

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
        return `<li><span class="sn">${esc(x.name)}</span>${why ? `<span class="sw">${esc(why)}</span>` : ""}</li>`;
      }).join("")
      : `<li class="empty">No supports planned</li>`;
    return `<div class="sk">
      <div class="sk-top"><span class="sk-name">${esc(s.name)}</span>
        ${cs?.button ? `<span class="sk-key">${esc(cs.button)}</span>` : ""}
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
  $("d-next").innerHTML = next.length
    ? next.slice(0, NEXT_SHOWN).map(n => `<li class="${n.notable ? "notable" : ""}" ${n.notable || n.keystone ? `data-tip="${n.skill}"` : ""}>${esc(n.name)}</li>`).join("")
    : `<li style="list-style:none" class="hint">Every planned passive for this stage is taken.</li>`;
  $("d-next-more").textContent = next.length > NEXT_SHOWN ? `Then ${next.length - NEXT_SHOWN} more for ${TPLAN.stage}.` : "";

  // Notables and keystones one by one; small passives grouped by name.
  const big = [], small = new Map();
  for (const s of TPLAN.allocated) {
    const pos = TREE_POS?.get(s);
    const kind = pos?.[2];
    const name = kind === 5 ? "Attribute" : pos?.[3] || TPLAN.details[s]?.name || "Passive";
    if (kind === 1 || kind === 2) big.push({ name, keystone: kind === 2, off: off.has(s), skill: s });
    else {
      const key = `${name}|${off.has(s)}`;
      const e = small.get(key) || { name, off: off.has(s), n: 0 };
      e.n++;
      small.set(key, e);
    }
  }
  big.sort((x, y) => (y.keystone - x.keystone) || x.name.localeCompare(y.name));
  const bigLines = big.map(b => `<div class="tk big ${b.keystone ? "ks" : ""} ${b.off ? "off" : ""}" data-tip="${b.skill}">${esc(b.name)}${b.off ? ` <span class="tk-off">off-plan</span>` : ""}</div>`);
  const smallLine = [...small.values()].sort((x, y) => y.n - x.n)
    .map(e => `<span class="${e.off ? "tk-offtext" : ""}">${esc(e.name)}${e.n > 1 ? ` ×${e.n}` : ""}</span>`).join(" · ");
  $("d-taken").innerHTML = (bigLines.join("") + (smallLine ? `<div class="tk small">${smallLine}</div>` : ""))
    || `<p class="hint">${TPLAN.allocated_seen ? "None yet." : "Nothing seen in the game log yet. Allocate a point in game and it shows here."}</p>`;
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

// ---- hover a notable or keystone: what it does ----
function nodeStats(skill) {
  const d = TPLAN?.details?.[skill];
  const pos = TREE_POS?.get(Number(skill));
  return { name: d?.name || pos?.[3] || "Passive", stats: d?.stats || [] };
}
document.addEventListener("mouseover", ev => {
  const host = ev.target.closest("[data-tip]");
  const tip = $("d-tip");
  if (!host) { tip.classList.add("hidden"); return; }
  const { name, stats } = nodeStats(host.dataset.tip);
  tip.innerHTML = `<b>${esc(name)}</b>${stats.length ? stats.map(s => `<div>${esc(s)}</div>`).join("") : `<div class="hint">No details</div>`}`;
  const r = host.getBoundingClientRect();
  tip.style.left = `${Math.min(r.left, window.innerWidth - 340)}px`;
  tip.style.top = `${r.bottom + 6}px`;
  tip.classList.remove("hidden");
});

listen("character", ({ payload }) => { renderHeader(payload); scheduleRefresh(3000); });
listen("feed", ({ payload }) => {
  const ul = $("d-feed");
  ul.prepend(feedLine(payload));
  while (ul.children.length > 200) ul.lastChild.remove();
});
listen("feed-all", ({ payload }) => renderFeed(payload));
listen("imported", () => scheduleRefresh());
listen("skills", () => scheduleRefresh());
listen("equipped", () => scheduleRefresh());

// Start once the page has loaded.
document.addEventListener("DOMContentLoaded", async () => {
  const snap = await invoke("snapshot");
  FOLLOWING = snap.imported?.name || null;
  renderHeader(snap.character);
  renderFeed(snap.feed || []);
  refresh();
});
