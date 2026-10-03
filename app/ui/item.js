// Gear drawn the way the game draws its item tooltip: rarity-coloured title
// bar, class and requirements, gold separators, implicits, then mods.
// Reads the game's copied item text (Ctrl+C or Ctrl+Alt+C), build-plan
// (Path of Building) item text, and trade listings.

const ITEM_SINGULAR = { "Body Armours": "Body Armour", "Foci": "Focus", "Quarterstaves": "Quarterstaff", "Staves": "Staff", "Gloves": "Gloves", "Boots": "Boots" };
const singularClass = cls => ITEM_SINGULAR[cls] || cls.replace(/s$/, "");

// "27(20-30)%" → "27%": advanced copy (Ctrl+Alt+C) adds each roll's range and notes.
const dropRanges = l => l.replace(/(\d)\(\d+(?:\.\d+)?-\d+(?:\.\d+)?\)/g, "$1").replace(/\s+—\s+Unscalable Value$/, "");
const MOD_TAG = / \((implicit|rune|enchant|crafted|fractured|desecrated|augmented)\)$/;
const DAMAGE_COLOR = { cold: "#3f80c4", lightning: "#ffd700", fire: "#d33b2c", chaos: "#d02090" };

function parseGameItem(text) {
  const sections = String(text).replace(/\r/g, "").split(/\n-{8,}[ \t]*(?:\n|$)/)
    .map(s => s.split("\n").map(l => l.trim()).filter(Boolean)).filter(s => s.length);
  const head = sections.shift() || [];
  let cls = "", rarity = "Normal";
  const names = [];
  for (const l of head) {
    if (l.startsWith("Item Class:")) cls = l.slice(11).trim();
    else if (l.startsWith("Rarity:")) rarity = l.slice(7).trim();
    else names.push(l);
  }
  const item = { rarity, name: names[0] || "", base: names[1] || "", top: [], blocks: [] };
  if (cls) item.top.push({ kind: "cls", text: singularClass(cls) });
  let sawMods = false;
  for (const s of sections) {
    const headers = s.some(l => l.startsWith("{"));
    const implicitHeader = s[0].startsWith("{ Implicit");
    const lines = s.filter(l => !l.startsWith("{")).map(dropRanges);
    if (!lines.length) continue;
    const all = re => lines.every(l => re.test(l));
    // Shown only while holding Alt in game, or not at all.
    if (all(/^(Item Level|Sockets|Note|Stack Size):/)) continue;
    if (lines[0].startsWith("Requires") || lines[0] === "Requirements:") { item.top.push(requirement(lines)); continue; }
    if (all(/^(Corrupted|Unidentified|Mirrored|Split|Unmodifiable)$/)) { item.blocks.push(lines.map(t => ({ kind: "corr", text: t }))); continue; }
    if (all(/ \((rune|enchant)\)$/)) { item.blocks.push(lines.map(t => ({ kind: "rune", text: t.replace(MOD_TAG, "") }))); continue; }
    if (all(/^Grants Skill:/)) { item.blocks.push(lines.map(t => ({ kind: "grant", text: t }))); continue; }
    if (implicitHeader || all(/ \(implicit\)$/)) { item.blocks.push(lines.map(t => ({ kind: "mod", text: t.replace(MOD_TAG, "") }))); continue; }
    if (!sawMods && !headers && all(/^[^:]{2,40}: \S/)) { item.top.push(...lines.map(t => ({ kind: "prop", text: t }))); continue; }
    // The first other section is the mods; later ones on a unique are its flavour text.
    if (sawMods && !headers && rarity === "Unique") { item.blocks.push(lines.map(t => ({ kind: "flav", text: t }))); continue; }
    sawMods = true;
    item.blocks.push(lines.map(t => {
      const tag = t.match(MOD_TAG)?.[1];
      return { kind: tag === "crafted" ? "craft" : tag === "fractured" ? "frac" : tag === "desecrated" ? "desc" : "mod", text: t.replace(MOD_TAG, "") };
    }));
  }
  return item;
}

function requirement(lines) {
  if (lines[0] !== "Requirements:") return { kind: "req", text: lines[0].replace(/^Requires:?\s*/, "") };
  // Older copies: "Requirements:" then "Level: 15", "Str: 20".
  const parts = lines.slice(1).map(l => l.split(":").map(x => x.trim())).map(([k, v]) => k === "Level" ? `Level ${v}` : `${v} ${k}`);
  return { kind: "req", text: parts.join(", ") };
}

// Path of Building / build-plan item text ("Rarity: RARE", "Implicits: 1", "{crafted}" tags).
function parsePobItem(text, slot) {
  const lines = String(text).replace(/\r/g, "").split("\n").map(l => l.trim()).filter(Boolean);
  const rarityRaw = (lines.shift() || "").replace(/^Rarity:\s*/, "");
  const rarity = rarityRaw.charAt(0) + rarityRaw.slice(1).toLowerCase();
  const meta = /^[A-Za-z][A-Za-z ]{1,30}:/;
  const names = [];
  while (lines.length && !meta.test(lines[0]) && names.length < (rarity === "Rare" || rarity === "Unique" ? 2 : 1)) names.push(lines.shift());
  const item = { rarity, name: names[0] || "", base: names[1] || "", top: [], blocks: [] };
  if (slot) item.top.push({ kind: "cls", text: slot });
  let implicits = -1;
  const imp = [], mods = [];
  let corrupted = false;
  for (const raw of lines) {
    const l = raw.replace(/^(\{[^}]*\})+/, "");
    const req = raw.match(/^LevelReq:\s*(\d+)/);
    if (req) { item.top.push({ kind: "req", text: `Level ${req[1]}` }); continue; }
    const n = raw.match(/^Implicits:\s*(\d+)/);
    if (n) { implicits = Number(n[1]); continue; }
    if (implicits < 0) continue; // metadata before the mods
    if (l === "Corrupted") { corrupted = true; continue; }
    const crafted = raw.includes("{crafted}");
    (imp.length < implicits ? imp : mods).push({ kind: crafted ? "craft" : "mod", text: l });
  }
  if (imp.length) item.blocks.push(imp);
  if (mods.length) item.blocks.push(mods);
  if (corrupted) item.blocks.push([{ kind: "corr", text: "Corrupted" }]);
  return item;
}

// A trade listing (lifeline-trade Listing): no rarity field, so it comes from the art path and the name.
function listingItem(c) {
  let unique = false;
  try {
    const seg = (c.icon || "").split("/image/")[1]?.split("/")[0] || "";
    unique = atob(seg.replace(/-/g, "+").replace(/_/g, "/")).includes("Uniques");
  } catch (_) { /* not base64: no art to read */ }
  const rarity = unique ? "Unique" : c.name && c.name !== c.base ? "Rare" : (c.mods || []).length ? "Magic" : "Normal";
  const item = { rarity, name: c.name || c.base, base: c.name && c.name !== c.base ? c.base : "", top: [], blocks: [] };
  if (c.requires) item.top.push({ kind: "req", text: c.requires });
  if ((c.mods || []).length) item.blocks.push(c.mods.map(t => ({ kind: "mod", text: t.replace(MOD_TAG, "") })));
  if (c.corrupted) item.blocks.push([{ kind: "corr", text: "Corrupted" }]);
  return item;
}

// Escaped text with numbers kept full height (small caps shrink digits in
// the serif fonts used here; the game shows them full size).
const fmt = text => escapeHtml(text).replace(/(?<![#\w&])(\d+(?:\.\d+)?)/g, `<span class="num">$1</span>`);

function propLine(text) {
  const i = text.indexOf(": ");
  if (i < 0) return fmt(text);
  const label = text.slice(0, i), value = text.slice(i + 2);
  if (label === "Elemental Damage") {
    const parts = value.split(/,\s*/).map(p => {
      const m = p.match(/^(.*) \((\w+)\)$/);
      return m ? `<span style="color:${DAMAGE_COLOR[m[2]] || "inherit"}">${fmt(m[1])}</span>` : fmt(p);
    });
    return `<span class="k">${escapeHtml(label)}: </span><span class="v">${parts.join(", ")}</span>`;
  }
  const aug = / \(augmented\)$/.test(value);
  return `<span class="k">${escapeHtml(label)}: </span><span class="v${aug ? " aug" : ""}">${fmt(value.replace(/ \(augmented\)$/, ""))}</span>`;
}

function itemLine(l) {
  switch (l.kind) {
    case "prop": case "grant": return `<div class="pl prop">${propLine(l.text)}</div>`;
    case "req": return `<div class="pl req"><span class="k">Requires: </span><span class="v">${fmt(l.text)}</span></div>`;
    default: return `<div class="pl ${l.kind}">${fmt(l.text)}</div>`;
  }
}

/** HTML for one item. `model` from parseGameItem / parsePobItem / listingItem. */
function itemCard(model, { equipped = false } = {}) {
  const r = (model.rarity || "Normal").toLowerCase();
  const sections = [model.top, ...model.blocks].filter(b => b.length);
  const body = sections.map(b => b.map(itemLine).join("")).join(`<div class="sep"></div>`);
  return `<div class="poe-item r-${escapeHtml(r)}">
    <div class="ph"><div class="n">${escapeHtml(model.name)}</div>${model.base ? `<div class="n">${escapeHtml(model.base)}</div>` : ""}</div>
    <div class="pb">${body}${equipped ? `<div class="sep"></div><div class="pl foot">Equipped</div>` : ""}</div>
  </div>`;
}

/** Item card straight from item text, game copy or build-plan format. */
function itemCardFromText(text, opts = {}) {
  const t = String(text || "");
  const pob = /^Rarity:/.test(t.trimStart()) && !/\n-{8,}/.test(t);
  return itemCard(pob ? parsePobItem(t, opts.slot) : parseGameItem(t), opts);
}
