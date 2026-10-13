// Shared helpers for the main window and the dashboard.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
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
