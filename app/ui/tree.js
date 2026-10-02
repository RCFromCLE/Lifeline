// ---- Passive tree: the followed build's plan drawn on the real tree ----
// Gold = planned for this stage, green = taken, red = taken but off-plan.
// Attribute nodes show the stat to pick (S / D / I); numbers are the order.
const SVGNS = "http://www.w3.org/2000/svg";
let TREE = null;          // layout: { bounds, nodes: [[skill, x, y, kind, name]], edges }
let TREE_POS = null;      // skill → [x, y, kind, name]
let TPLAN = null;         // tree_plan result
let TSTAGE = null;        // stage label the player picked (null = current)
let VIEW = null;          // { x, y, w, h } viewBox
const RADIUS = { 0: 20, 1: 34, 2: 46, 3: 30, 4: 64, 5: 20 };
const ATTR = { str: ["S", "#e58a6e"], dex: ["D", "#8fd17a"], int: ["I", "#79b2e8"] };

function el(tag, attrs = {}, text) {
  const e = document.createElementNS(SVGNS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  if (text != null) e.textContent = text;
  return e;
}

function edgePath(edges, keep) {
  let d = "";
  for (const [a, b, cx, cy] of edges) {
    if (keep && !keep(a, b)) continue;
    const p = TREE_POS.get(a), q = TREE_POS.get(b);
    if (!p || !q) continue;
    if (cx == null) { d += `M${p[0]} ${p[1]}L${q[0]} ${q[1]}`; continue; }
    const r = Math.hypot(p[0] - cx, p[1] - cy);
    const cross = (p[0] - cx) * (q[1] - cy) - (p[1] - cy) * (q[0] - cx);
    d += `M${p[0]} ${p[1]}A${r.toFixed(0)} ${r.toFixed(0)} 0 0 ${cross > 0 ? 1 : 0} ${q[0]} ${q[1]}`;
  }
  return d;
}

function setView(v) {
  VIEW = v;
  const svg = $("tree-svg");
  svg.setAttribute("viewBox", `${v.x} ${v.y} ${v.w} ${v.h}`);
  // Tree units per screen pixel: keeps labels and plan markers readable at any zoom.
  const box = svg.getBoundingClientRect();
  const k = Math.max(v.w / (box.width || 800), v.h / (box.height || 560));
  svg.style.setProperty("--k", k.toFixed(3));
}

// Frame the planned and taken nodes (or the whole tree).
function fitTree() {
  const pts = [...(TPLAN?.planned || []), ...(TPLAN?.allocated || []), TPLAN?.start].map(s => TREE_POS.get(s)).filter(Boolean);
  const box = $("tree-view").getBoundingClientRect();
  const aspect = (box.width || 800) / (box.height || 560);
  let [x0, y0, x1, y1] = TREE.bounds;
  if (pts.length) {
    x0 = Math.min(...pts.map(p => p[0])) - 700; x1 = Math.max(...pts.map(p => p[0])) + 700;
    y0 = Math.min(...pts.map(p => p[1])) - 700; y1 = Math.max(...pts.map(p => p[1])) + 700;
  }
  let w = x1 - x0, h = y1 - y0;
  if (w / h < aspect) { const nw = h * aspect; x0 -= (nw - w) / 2; w = nw; } else { const nh = w / aspect; y0 -= (nh - h) / 2; h = nh; }
  setView({ x: x0, y: y0, w, h });
}

function drawTree() {
  const svg = $("tree-svg");
  svg.innerHTML = "";
  const planned = new Set(TPLAN.planned), earlier = new Set(TPLAN.earlier), taken = new Set(TPLAN.allocated);
  const off = new Set(TPLAN.off_plan);
  const onPath = s => planned.has(s) || s === TPLAN.start;
  const takenPath = s => taken.has(s) || s === TPLAN.start;
  svg.appendChild(el("path", { d: edgePath(TREE.edges), class: "t-edge" }));
  svg.appendChild(el("path", { d: edgePath(TREE.edges, (a, b) => onPath(a) && onPath(b)), class: "t-edge plan" }));
  svg.appendChild(el("path", { d: edgePath(TREE.edges, (a, b) => takenPath(a) && takenPath(b)), class: "t-edge taken" }));

  const bg = el("g", { class: "t-bg" }), fg = el("g", { class: "t-fg" }), labels = el("g", { class: "t-labels" });
  const order = new Map(TPLAN.next.map((n, i) => [n.skill, i + 1]));
  for (const [s, x, y, kind] of TREE.nodes) {
    const r = RADIUS[kind] || 20;
    const mine = planned.has(s) || off.has(s) || s === TPLAN.start;
    if (!mine) { bg.appendChild(el("circle", { cx: x, cy: y, r, class: `t-node k${kind}`, "data-s": s })); continue; }
    const d = TPLAN.details[s] || {};
    const state = s === TPLAN.start ? "start" : off.has(s) ? "off" : taken.has(s) ? "taken" : earlier.has(s) ? "earlier" : "plan";
    fg.appendChild(el("circle", { cx: x, cy: y, r: r + 4, class: `t-node mine ${state} k${kind}`, "data-s": s, style: `--r:${r + 4};--min:${kind === 1 || kind === 2 ? 9 : 6.5}` }));
    if (d.attr) {
      const [letter, color] = ATTR[d.attr];
      labels.appendChild(el("text", { x, y, class: "t-attr", fill: color }, letter));
    }
    const n = order.get(s);
    if (n) labels.appendChild(el("text", { x, y, class: "t-order" }, n));
    if ((kind === 1 || kind === 2) && d.name) labels.appendChild(el("text", { x, y, class: "t-name" }, d.name));
  }
  svg.append(bg, fg, labels);
}

function treeTip(ev) {
  const s = Number(ev.target.dataset?.s);
  const tip = $("tree-tip");
  if (!s) { tip.classList.add("hidden"); return; }
  const d = TPLAN?.details[s];
  const p = TREE_POS.get(s);
  const name = d?.name || p?.[3] || "Passive";
  const status = s === TPLAN?.start ? "Class start" : TPLAN?.off_plan.includes(s) ? "Taken · not in the plan"
    : TPLAN?.allocated.includes(s) ? "Taken" : d ? "Planned" : "";
  tip.innerHTML = `<b>${esc(name)}</b>${status ? `<div class="st">${status}</div>` : ""}${(d?.stats || []).map(x => `<div>${esc(x)}</div>`).join("")}`;
  const box = $("tree-view").getBoundingClientRect();
  tip.style.left = `${Math.min(ev.clientX - box.left + 14, box.width - 260)}px`;
  tip.style.top = `${ev.clientY - box.top + 14}px`;
  tip.classList.remove("hidden");
}

function renderTreeSide() {
  const p = TPLAN;
  const attrs = p.attributes.length
    ? p.attributes.map(a => `<li><span class="t-chip" style="color:${ATTR[a.short][1]}">${ATTR[a.short][0]}</span> <b>${a.nodes} × ${esc(a.attr)}</b>${a.why.length ? `<div class="hint">for ${a.why.map(esc).join(", ")}</div>` : ""}</li>`).join("")
    : "<li class='hint'>No attribute nodes in this stage.</li>";
  const next = p.next.map((n, i) => `<li data-s="${n.skill}" class="${n.notable ? "notable" : ""}"><span class="t-num">${i + 1}</span>${n.attr ? `<span class="t-chip" style="color:${ATTR[n.attr][1]}">${ATTR[n.attr][0]}</span>` : ""}${esc(n.name)}</li>`).join("");
  const asc = p.ascendancy.map(a => `<li class="${a.taken ? "done" : ""}">${a.taken ? "✓ " : ""}${esc(a.name)}</li>`).join("");
  $("tree-side").innerHTML = `
    <div class="d-sec"><h3>Attribute nodes</h3><ul class="t-attrs">${attrs}</ul>
      <div class="hint">Every "+5 to any Attribute" node: pick this stat. Lifeline works it out from your gems and gear.</div></div>
    <div class="d-sec"><h3>Take in this order</h3>${next ? `<ol class="t-next">${next}</ol>` : "<p class='hint'>All planned passives for this stage are taken.</p>"}
      ${p.allocated_seen ? "" : "<div class='hint'>Nothing seen in the game log yet; allocate a point in game and it shows here.</div>"}</div>
    ${asc ? `<div class="d-sec"><h3>Ascendancy</h3><ul class="t-asc">${asc}</ul></div>` : ""}`;
  $("tree-side").querySelectorAll(".t-next li").forEach(li => {
    const s = Number(li.dataset.s);
    li.addEventListener("mouseenter", () => highlight(s, true));
    li.addEventListener("mouseleave", () => highlight(s, false));
    li.addEventListener("click", () => centerOn(s));
  });
}

function highlight(s, on) {
  $("tree-svg").querySelectorAll(`.mine[data-s="${s}"]`).forEach(c => c.classList.toggle("hl", on));
}

function centerOn(s) {
  const p = TREE_POS.get(s);
  if (!p || !VIEW) return;
  const w = Math.min(VIEW.w, 3200), h = w * VIEW.h / VIEW.w;
  setView({ x: p[0] - w / 2, y: p[1] - h / 2, w, h });
  highlight(s, true);
  setTimeout(() => highlight(s, false), 1200);
}

function zoomBy(f, cx, cy) {
  const v = VIEW;
  const px = cx ?? v.x + v.w / 2, py = cy ?? v.y + v.h / 2;
  setView({ x: px - (px - v.x) * f, y: py - (py - v.y) * f, w: v.w * f, h: v.h * f });
}

function wireTreeView() {
  const view = $("tree-view"), svg = $("tree-svg");
  const toTree = ev => {
    const b = svg.getBoundingClientRect();
    return [VIEW.x + (ev.clientX - b.left) / b.width * VIEW.w, VIEW.y + (ev.clientY - b.top) / b.height * VIEW.h];
  };
  view.addEventListener("wheel", ev => {
    if (!VIEW) return;
    ev.preventDefault();
    const [x, y] = toTree(ev);
    zoomBy(ev.deltaY > 0 ? 1.18 : 1 / 1.18, x, y);
  }, { passive: false });
  let drag = null;
  view.addEventListener("pointerdown", ev => {
    if (!VIEW || ev.button !== 0 || ev.target.closest("button")) return;
    drag = { x: ev.clientX, y: ev.clientY, v: { ...VIEW } };
    view.setPointerCapture(ev.pointerId);
    view.classList.add("dragging");
  });
  view.addEventListener("pointermove", ev => {
    if (!drag) { treeTip(ev); return; }
    const b = svg.getBoundingClientRect();
    setView({ ...drag.v, x: drag.v.x - (ev.clientX - drag.x) / b.width * drag.v.w, y: drag.v.y - (ev.clientY - drag.y) / b.height * drag.v.h });
  });
  const end = () => { drag = null; view.classList.remove("dragging"); };
  view.addEventListener("pointerup", end);
  view.addEventListener("pointercancel", end);
  view.addEventListener("pointerleave", () => $("tree-tip").classList.add("hidden"));
  $("tz-in").addEventListener("click", () => zoomBy(1 / 1.4));
  $("tz-out").addEventListener("click", () => zoomBy(1.4));
  $("tz-fit").addEventListener("click", fitTree);
  // Keyboard: + / - zoom, 0 fits, arrows pan (when the tree has focus).
  view.addEventListener("keydown", ev => {
    if (!VIEW) return;
    const step = VIEW.w * 0.12;
    const moves = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (moves[ev.key]) { setView({ ...VIEW, x: VIEW.x + moves[ev.key][0], y: VIEW.y + moves[ev.key][1] }); ev.preventDefault(); }
    else if (ev.key === "+" || ev.key === "=") zoomBy(1 / 1.3);
    else if (ev.key === "-") zoomBy(1.3);
    else if (ev.key === "0") fitTree();
  });
}

let treeTimer = null;
function refreshTree(stage) {
  if (stage !== undefined) TSTAGE = stage;
  clearTimeout(treeTimer);
  treeTimer = setTimeout(async () => {
    if (!CURRENT) { $("tree-card").classList.add("hidden"); return; }
    try {
      if (!TREE) {
        TREE = await invoke("tree_layout");
        TREE_POS = new Map(TREE.nodes.map(n => [n[0], [n[1], n[2], n[3], n[4]]]));
      }
      TPLAN = await invoke("tree_plan", { stage: TSTAGE });
    } catch (e) {
      $("tree-card").classList.remove("hidden");
      $("tree-side").innerHTML = `<p class="hint">${esc(String(e))}</p>`;
      return;
    }
    $("tree-card").classList.remove("hidden");
    $("tree-stages").innerHTML = TPLAN.stages.map(s => `<button class="cstage${s === TPLAN.stage ? " on" : ""}" data-stage="${esc(s)}">${esc(s)}</button>`).join("");
    $("tree-stages").querySelectorAll("button").forEach(b => b.addEventListener("click", () => refreshTree(b.dataset.stage)));
    const refit = !VIEW || $("tree-svg").dataset.stage !== TPLAN.stage;
    drawTree();
    $("tree-svg").dataset.stage = TPLAN.stage;
    if (refit) fitTree();
    renderTreeSide();
  }, 300);
}

wireTreeView();
