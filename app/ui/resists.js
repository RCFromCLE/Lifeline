// ---- Header: the resistances that matter in this zone ----
// Which elements the zone's monsters and boss hit with (researched per zone),
// most important first. The tooltip says why, the boss, the 75% cap after
// this area's penalty, and what recorded gear gives.
(() => {
  const pill = $("penalty-pill");
  if (!pill) return;
  const NAMES = { fire: "Fire", cold: "Cold", lightning: "Lightning", chaos: "Chaos" };

  function tooltip(r) {
    const f = r.focus || {};
    const parts = [`${r.zone || "This zone"}${f.boss ? ` (boss: ${f.boss})` : ""}: ${f.why || "no data"}`];
    if (f.confidence === "low") parts.push("(best guess: not confirmed by a guide)");
    if (r.need != null) parts.push(`To be capped at 75% here you need ${r.need}% (${r.penalty}% penalty).`);
    if (r.gear_slots) parts.push(`Recorded gear and rewards: fire ${r.fire}%, cold ${r.cold}%, lightning ${r.lightning}%, chaos ${r.chaos}% (passive tree not counted).`);
    return parts.join("\n");
  }

  async function refresh() {
    let r;
    try { r = await invoke("resistances"); } catch (_) { return; }
    const focus = (r.focus?.resists || []).filter(k => NAMES[k]);
    pill.title = tooltip(r);
    if (!focus.length && r.focus?.physical) {
      pill.className = "stat res-pill";
      pill.innerHTML = `<span class="rp-label">${escapeHtml(r.zone || "This zone")}: physical</span><span class="rp phys">(armour, life)</span>`;
      return;
    }
    if (!focus.length) {
      pill.className = "stat";
      pill.textContent = r.zone ? `${r.zone}${r.need != null ? `: res cap ${r.need}%` : ""}` : (r.need != null ? `Res cap: ${r.need}%` : "Res —");
      return;
    }
    pill.className = "stat res-pill";
    pill.innerHTML = `<span class="rp-label">${escapeHtml(r.zone || "This zone")}: resist</span>` + focus.map(k => `<span class="rp ${k}">${NAMES[k]}</span>`).join(`<span class="rp-sep">·</span>`);
  }

  // After app.js draws the character line, so this wins.
  listen("character", () => setTimeout(refresh, 0));
  listen("equipped", refresh);
  refresh();
})();
