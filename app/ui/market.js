// ---- Market: search it yourself, and steer the Rating upgrades ----
// A Market tab (item kind, primary and secondary modifier, price limit or
// "best possible" at any price), and a focus row on every Rating upgrade to
// re-run its search with the modifiers you care about. Modifier pickers
// list item properties (total Armour, Evasion…) and every trade-site stat;
// type to filter.
(() => {
  const CATEGORIES = [
    ["", "Any item"], ["armour.helmet", "Helmet"], ["armour.chest", "Body armour"], ["armour.gloves", "Gloves"],
    ["armour.boots", "Boots"], ["armour.shield", "Shield"], ["armour.buckler", "Buckler"], ["armour.focus", "Focus"],
    ["armour.quiver", "Quiver"], ["accessory.amulet", "Amulet"], ["accessory.ring", "Ring"], ["accessory.belt", "Belt"],
    ["weapon.spear", "Spear"], ["weapon.warstaff", "Quarterstaff"], ["weapon.bow", "Bow"], ["weapon.crossbow", "Crossbow"],
    ["weapon.onemace", "One-hand mace"], ["weapon.twomace", "Two-hand mace"], ["weapon.sceptre", "Sceptre"],
    ["weapon.wand", "Wand"], ["weapon.staff", "Staff"], ["jewel", "Jewel"],
  ];
  let MODS = null;           // [{id, label}]
  const BY_LABEL = new Map(); // label → id
  const FOCUS = {};           // upgrade index → {primary, secondary, any}

  // Stylesheet and the tab itself (app.js switches views by `view-<tab>`).
  const css = document.createElement("link");
  css.rel = "stylesheet"; css.href = "market.css";
  document.head.appendChild(css);
  const tab = document.createElement("button");
  tab.className = "tab"; tab.dataset.tab = "market"; tab.textContent = "Market";
  $("tabs").insertBefore(tab, document.querySelector('.tab[data-tab="settings"]'));
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach(b => b.classList.toggle("active", b === tab));
    document.querySelectorAll(".view").forEach(v => v.classList.toggle("active", v.id === "view-market"));
    loadModifiers();
    $("mk-travel").checked = !!settings?.show_trade_on_travel;
  });

  const view = document.createElement("section");
  view.className = "view scroll"; view.id = "view-market";
  view.innerHTML = `
    <div class="col">
      <div class="card">
        <h2>Search the market</h2>
        <div class="mk-grid">
          <label>Item<select id="mk-cat">${CATEGORIES.map(([v, l]) => `<option value="${v}">${l}</option>`).join("")}</select></label>
          <label>Primary modifier<input id="mk-primary" list="mk-mods" placeholder="Type to search, e.g. Armour" autocomplete="off"></label>
          <label>Secondary modifier<input id="mk-secondary" list="mk-mods" placeholder="Optional, e.g. Evasion" autocomplete="off"></label>
          <label>Max price (exalted)<input id="mk-price" type="number" min="0" step="1" placeholder="No limit"></label>
        </div>
        <div class="mk-row">
          <label class="check"><input type="checkbox" id="mk-best"> Best possible: any price, strongest for the primary modifier</label>
          <label class="check"><input type="checkbox" id="mk-instant" checked> Instant buyout only (Travel works)</label>
        </div>
        <div class="mk-row">
          <button class="primary" id="mk-go">Search</button>
          <span class="hint" id="mk-status">Only items your character can wear at their level.</span>
        </div>
        <datalist id="mk-mods"></datalist>
      </div>
      <div class="card hidden" id="mk-results-card">
        <div class="mk-head"><h2>Results</h2><span class="hint" id="mk-total"></span>
          <button class="ghost" id="mk-rate" title="Lifeline compares each with what you wear, in a new chat">Rate these for my build</button>
          <button class="ghost" id="mk-site" title="Open this search on pathofexile.com">Open on trade site</button></div>
        <div id="mk-results"></div>
      </div>
    </div>
    <div class="col narrow">
      <div class="card">
        <h2>Travel</h2>
        <label class="check"><input type="checkbox" id="mk-travel"> Show the trade window when travelling</label>
        <p class="hint">Off: Travel works in the background and the window only opens if you need to sign in to pathofexile.com.</p>
      </div>
    </div>`;
  document.querySelector("main")?.appendChild(view) || document.body.appendChild(view);

  async function loadModifiers() {
    if (MODS) return;
    try {
      MODS = await invoke("trade_modifiers");
      for (const m of MODS) if (!BY_LABEL.has(m.label)) BY_LABEL.set(m.label, m.id);
      $("mk-mods").innerHTML = [...BY_LABEL.keys()].map(l => `<option value="${escapeHtml(l)}">`).join("");
    } catch (e) {
      MODS = null;
      toast(`Couldn't load the modifier list (the trade site may be down): ${e}`, "err");
    }
  }
  // A typed modifier → its id (exact pick, else the first label containing it).
  function modId(text) {
    const t = (text || "").trim();
    if (!t) return null;
    if (BY_LABEL.has(t)) return BY_LABEL.get(t);
    const low = t.toLowerCase();
    for (const [label, id] of BY_LABEL) if (label.toLowerCase().includes(low)) return id;
    return null;
  }

  let LAST = null;
  $("mk-go").addEventListener("click", async () => {
    await loadModifiers();
    const best = $("mk-best").checked;
    const primary = modId($("mk-primary").value), secondary = modId($("mk-secondary").value);
    if ($("mk-primary").value.trim() && !primary) { toast("Pick the primary modifier from the list.", "err"); return; }
    if (best && !primary) { toast("Best possible needs a primary modifier to rank by.", "err"); return; }
    const price = parseFloat($("mk-price").value);
    $("mk-go").disabled = true;
    $("mk-status").textContent = "Searching…";
    try {
      LAST = await invoke("market_search", { request: {
        category: $("mk-cat").value, primary, secondary,
        max_price: Number.isFinite(price) ? price : null, best, instant_only: $("mk-instant").checked,
      } });
      showResults(LAST, best);
      $("mk-status").textContent = `Wearable at level ${LAST.level}.`;
    } catch (e) {
      $("mk-status").textContent = "";
      toast(`Search failed: ${e}`, "err");
    } finally { $("mk-go").disabled = false; }
  });

  function showResults(r, best) {
    $("mk-results-card").classList.remove("hidden");
    $("mk-total").textContent = `${r.cards.length} shown of ${r.total} listed${best ? " · best possible, any price" : ""}`;
    const el = renderMarket({ compared_to: "", cards: r.cards.map(c => ({ ...c, delta_pct: 0 })) });
    el.querySelectorAll(".badge").forEach(b => b.remove());
    el.querySelector(".market-head")?.remove();
    $("mk-results").replaceChildren(el);
    if (!r.cards.length) $("mk-results").innerHTML = "<p class='hint'>Nothing listed for that. Loosen the price or modifiers.</p>";
  }
  $("mk-site").addEventListener("click", () => LAST && invoke("open_url", { url: LAST.url }).catch(e => toast(e, "err")));
  $("mk-rate").addEventListener("click", async () => {
    if (!LAST?.cards.length) return;
    try {
      const id = await invoke("rate_market_chat", { searchId: LAST.search_id, listingIds: LAST.cards.map(c => c.listing_id) });
      document.querySelector('.tab[data-tab="play"]').click();
      if (typeof selectConv === "function") selectConv(id);
    } catch (e) { toast(e, "err"); }
  });
  $("mk-travel").addEventListener("change", async ev => {
    try {
      await invoke("set_trade_on_travel", { show: ev.target.checked });
      if (settings) settings.show_trade_on_travel = ev.target.checked;
      toast(ev.target.checked ? "Travel will show the trade window." : "Travel works in the background now.", "ok");
    } catch (e) { toast(e, "err"); }
  });

  // ---- Rating upgrades: a focus row to re-run each search ----
  function decorateUpgrades() {
    document.querySelectorAll("#r-recs .r-rec").forEach((rec, index) => {
      if (rec.querySelector(".focus-row") || index > 2) return;
      const f = FOCUS[index] || {};
      const row = document.createElement("div");
      row.className = "focus-row";
      row.innerHTML = `
        <input class="f-primary" list="mk-mods" placeholder="Primary modifier, e.g. Armour" value="${escapeHtml(f.primary || "")}" autocomplete="off">
        <input class="f-secondary" list="mk-mods" placeholder="Secondary, e.g. Evasion" value="${escapeHtml(f.secondary || "")}" autocomplete="off">
        <label class="check"><input type="checkbox" class="f-any" ${f.any ? "checked" : ""}> Best possible (any price)</label>
        <button class="f-go">Search again</button>`;
      rec.querySelector(".look")?.after(row) || rec.prepend(row);
      row.querySelectorAll("input").forEach(i => i.addEventListener("focus", loadModifiers, { once: true }));
      row.querySelector(".f-go").addEventListener("click", async () => {
        await loadModifiers();
        const p = row.querySelector(".f-primary").value.trim(), s = row.querySelector(".f-secondary").value.trim();
        const any = row.querySelector(".f-any").checked;
        if (!p && !s && !any) { toast("Pick a modifier (or Best possible) first.", "err"); return; }
        FOCUS[index] = { primary: p, secondary: s, any };
        // The AI gets the modifier text plus its trade id.
        const tag = t => { const id = modId(t); return t ? (id ? `${t} [${id.startsWith("prop.") ? `equipment filter ${id.slice(5)}` : id}]` : t) : null; };
        try {
          await invoke("rerun_upgrade", { index, focus: { primary: tag(p), secondary: tag(s) }, anyPrice: any });
          toast(`Searching again${any ? " at any price" : ""}…`, "info");
        } catch (e) { toast(e, "err"); }
      });
    });
  }
  new MutationObserver(decorateUpgrades).observe($("r-recs"), { childList: true });
  decorateUpgrades();
})();
