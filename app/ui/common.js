// Shared helpers for the main window and the HUD overlay.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

// Small Markdown subset: code fences, headings, bullets, bold, inline code.
function renderMarkdown(text) {
  const parts = String(text || "").split(/```(?:\w+)?\n?/);
  return parts.map((part, i) => {
    if (i % 2 === 1) return `<pre>${escapeHtml(part)}</pre>`;
    const lines = escapeHtml(part).split("\n");
    let html = "", inList = false;
    for (const raw of lines) {
      let line = raw
        .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
        .replace(/`([^`]+)`/g, "<code>$1</code>");
      const bullet = line.match(/^\s*(?:[-*•]|\d+\.)\s+(.*)$/);
      if (bullet) {
        if (!inList) { html += "<ul>"; inList = true; }
        html += `<li>${bullet[1]}</li>`;
        continue;
      }
      if (inList) { html += "</ul>"; inList = false; }
      const heading = line.match(/^#{1,4}\s+(.*)$/);
      if (heading) html += `<div><strong>${heading[1]}</strong></div>`;
      else if (line.trim()) html += `<div>${line}</div>`;
      else html += "<div style='height:6px'></div>";
    }
    if (inList) html += "</ul>";
    return html;
  }).join("");
}

function penaltyClass(p) {
  if (p === null || p === undefined) return "";
  if (p <= -40) return "bad";
  if (p < 0) return "warn";
  return "";
}

// A gem's colour from its attribute: "str", "dex", "int" or a mix like "str/dex".
const GEM_COLORS = { str: "#d8574a", dex: "#86c46e", int: "#6fa3d6" };
function gemStyle(attr) {
  const colors = String(attr || "").split("/").map(a => GEM_COLORS[a]).filter(Boolean);
  if (!colors.length) return "";
  const bg = colors.length === 1 ? colors[0] : `linear-gradient(135deg, ${colors.join(", ")})`;
  return `--gem:${colors[0]};background:${bg}`;
}
const gemDot = attr => `<span class="gd" style="${gemStyle(attr)}"></span>`;

function describeCharacter(c) {
  const name = c.name ? `${c.name} — ${c.class} level ${c.level}` : "No character seen in the log yet";
  const where = c.zone ? `${c.zone} · area ${c.area_level}${c.act ? ` · Act ${c.act}` : ""}` : "";
  return { name, where };
}
