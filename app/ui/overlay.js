// The overlay: the next passive to take for the followed build, and a Skills
// button for the skills and supports planned for where you are. Same plan as
// Builds → Current. The window hugs whatever is showing.
const el = id => document.getElementById(id);
const ATTR = { Strength: "str", Dexterity: "dex", Intelligence: "int" };
let flashUntil = 0;

// The window takes exactly the size of what's showing.
function fit() {
  const r = el("root").getBoundingClientRect();
  invoke("fit_overlay", { width: r.right + 3, height: r.bottom + 3 }).catch(() => {});
}
new ResizeObserver(fit).observe(el("root"));

// The passive's kind as a symbol: minor, major (notable) or keystone.
const SYMBOL = {
  minor: `<svg viewBox="0 0 16 16"><circle class="minor" cx="8" cy="8" r="3.6"/></svg>`,
  notable: `<svg viewBox="0 0 16 16"><circle class="major" cx="8" cy="8" r="6.2"/><circle class="major-core" cx="8" cy="8" r="2.4"/></svg>`,
  keystone: `<svg viewBox="0 0 16 16"><polygon class="keystone" points="8,1.2 14,4.6 14,11.4 8,14.8 2,11.4 2,4.6"/><circle class="keystone-core" cx="8" cy="8" r="2.4"/></svg>`,
};
const SYMBOL_NAME = { minor: "Minor passive", notable: "Major passive (notable)", keystone: "Keystone" };

function showPassive(kind, html, cls = "", symbol = null, attr = "") {
  el("k").textContent = locked ? kind : "Drag to move";
  el("name").innerHTML = html;
  el("name").className = `name ${cls}`;
  el("sym").innerHTML = symbol ? SYMBOL[symbol] || "" : "";
  el("sym").className = `sym ${attr}`;
  el("sym").title = symbol ? SYMBOL_NAME[symbol] || "" : "";
  el("pill").className = "pill";
}

function renderSkills(a) {
  el("stage").textContent = `${a.stage} skills`;
  el("skills").innerHTML = (a.skills || []).map(s => `<li>
      <div>${gemDot(s.attr)}<span class="gem">${escapeHtml(s.skill)}</span></div>
      <div class="sups">${s.supports.length
        ? s.supports.map((x, i) => `<span>${gemDot(s.support_attrs?.[i])}${escapeHtml(x)}</span>`).join("")
        : `<span class="none">No supports planned</span>`}</div>
    </li>`).join("") || `<li class="none">No skills planned for this stage.</li>`;
}

async function refresh() {
  const a = await invoke("build_alignment").catch(() => null);
  const p = a?.passives;
  // Nothing to show without a followed build.
  el("root").classList.toggle("hidden", !p);
  if (!p) return;
  renderSkills(a);
  if (Date.now() < flashUntil) return;
  const next = p.missing[0];
  if (!next) {
    showPassive(a.stage, "✓ All passives taken");
    el("pill").classList.add("complete");
    return;
  }
  // "Attribute → Strength": the stat to pick on a +5 attribute node.
  const attr = next.name.match(/^Attribute → (\w+)$/);
  const kind = next.kind || (next.notable ? "notable" : "minor");
  if (attr) showPassive("Next passive", `Attribute: <span class="${ATTR[attr[1]] || ""}">${escapeHtml(attr[1])}</span>`, "", "minor", ATTR[attr[1]] || "");
  else showPassive("Next passive", escapeHtml(next.name), kind === "minor" ? "" : "notable", kind);
}

// ---- move: unlock, drag the passive pill anywhere, lock ----
let locked = true;
el("lock-btn").addEventListener("click", async () => {
  locked = !locked;
  el("lock-btn").setAttribute("aria-pressed", String(locked));
  el("root").classList.toggle("unlocked", !locked);
  try { await invoke("overlay_lock", { locked }); } catch (_) { /* keeps working where it is */ }
  refresh();
});
el("pill").addEventListener("mousedown", ev => {
  if (locked || ev.button !== 0) return;
  ev.preventDefault();
  window.__TAURI__.window.getCurrentWindow().startDragging().catch(() => {});
});

el("skills-btn").addEventListener("click", () => {
  const open = el("panel").classList.toggle("hidden") === false;
  el("skills-btn").setAttribute("aria-expanded", String(open));
});

// Game events come in bursts; refresh once they settle.
let timer = null;
function schedule(ms = 400) { clearTimeout(timer); timer = setTimeout(refresh, ms); }

// A planned passive was taken: confirm it for a moment, then the next one.
listen("passive-taken", ({ payload }) => {
  flashUntil = Date.now() + 2200;
  showPassive("Taken", `✓ ${escapeHtml(payload)}`);
  el("pill").classList.add("done");
  schedule(2300);
});
listen("character", () => schedule());
listen("imported", () => schedule(100));
setInterval(refresh, 30000);
refresh();
