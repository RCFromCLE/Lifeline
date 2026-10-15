// Main window: tabs, the character line, Skills, build import, the game's
// Build Planner folder, and settings. Builds live in builds.js and tree.js.
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

// ---- character line ----
function renderCharacter(c) {
  const d = describeCharacter(c);
  $("char-name").textContent = d.name;
  $("char-meta").textContent = d.where;
}

// ---- Claude usage meters (shown after a build or skills run) ----
function renderUsage(u) {
  const names = { five_hour: "5-hour", seven_day: "7-day" };
  $("usage").innerHTML = (u.windows || []).map(w => {
    const cls = w.pct >= 80 ? "high" : w.pct >= 50 ? "mid" : "";
    return `<div class="meter"><span>${escapeHtml(names[w.name] || w.name)}</span><span class="bar"><i class="${cls}" style="width:${Math.min(100, Number(w.pct) || 0)}%"></i></span><span>${Number(w.pct) || 0}%</span></div>`;
  }).join("");
}

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

// ---- build import ----
function renderImport(v) {
  const rows = v.stages.map(s => `<tr class="${s.chosen ? "chosen" : ""}">
      <td>${escapeHtml(s.title || "(untitled)")}</td><td><span class="stage-tag">${escapeHtml(s.stage)}</span></td>
      <td>${s.main_points}</td><td>${s.ascendancy_points}</td><td>${s.level_to >= 100 ? `${s.level_from}+` : `${s.level_from}–${s.level_to}`}</td></tr>`).join("");
  $("import-result").innerHTML = `
    <h2 class="import-title">${escapeHtml(v.ascendancy || v.class_name || "Build")} · level ${v.level ?? "?"}</h2>
    ${v.warning ? `<div class="status err">${escapeHtml(v.warning)}</div>` : ""}
    <table><thead><tr><th>Tree</th><th>Stage</th><th>Passives</th><th>Ascendancy pts</th><th>Levels</th></tr></thead><tbody>${rows}</tbody></table>
    <p class="hint import-note">★ = written for that stage</p>
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

// ---- settings: controls, text size and the overlay; every change saves on its own ----
function fillSettings(s) {
  const f = $("settings-form");
  f.elements.input.value = s.input || "keyboard";
  f.elements.text_size.value = s.text_size || "auto";
  f.elements.overlay.checked = s.overlay !== false;
  f.elements.passive_sound.checked = s.passive_sound !== false;
  applyTextSize(s.text_size);
}

$("settings-form").addEventListener("submit", async ev => {
  ev.preventDefault();
  const f = ev.target;
  const next = {
    ...settings, input: f.elements.input.value, text_size: f.elements.text_size.value,
    overlay: f.elements.overlay.checked, passive_sound: f.elements.passive_sound.checked,
  };
  try {
    await invoke("save_settings", { settings: next });
    settings = next;
    $("settings-status").className = "status ok";
    $("settings-status").textContent = "✓ Saved.";
  } catch (e) {
    $("settings-status").className = "status err";
    $("settings-status").textContent = `Couldn't save: ${e}`;
  }
});
$("settings-form").addEventListener("change", () => { if (settings) $("settings-form").requestSubmit(); });
$("chime-test").addEventListener("click", () => invoke("preview_chime").catch(e => toast(e, "err")));

// Text size: the whole app scales; Auto grows with the window.
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

// ---- About: this version, and the newest release on GitHub ----
const LATEST_RELEASE = "https://github.com/RCFromCLE/Lifeline/releases/latest";
Promise.resolve(window.__TAURI__?.app?.getVersion?.()).then(v => { if (v) $("app-version").textContent = v; }).catch(() => {});
$("btn-releases").addEventListener("click", () =>
  invoke("open_url", { url: LATEST_RELEASE }).catch(e => toast(`Couldn't open the browser: ${e}`, "err")));

// ---- events + startup ----
listen("character", ({ payload }) => renderCharacter(payload));
listen("usage", ({ payload }) => renderUsage(payload));
listen("notice", ({ payload }) => toast(payload));
listen("imported", ({ payload }) => renderImport(payload));

(async () => {
  const snap = await invoke("snapshot");
  settings = snap.settings;
  renderCharacter(snap.character);
  fillSettings(snap.settings);
  if (snap.problems?.length) toast(snap.problems.join(" "), "err");
  refreshPlannerFiles();
  renderSkills(await invoke("skills_snapshot"));
})();
