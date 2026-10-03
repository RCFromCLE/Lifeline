// First-run welcome: what Lifeline is, connect Claude (install + sign in),
// your game and controls, a short tour, then straight into a build.
// Reopen any time from Settings → Welcome tour. Keyboard: Enter/→ next,
// ← back, Esc close.

(() => {
  const W = { step: 0, status: null, input: null, league: "", poll: null };
  const STEPS = 5;
  const esc = s => escapeHtml(s ?? "");
  const ok = (good, yes, no) => `<span class="wl-ok ${good ? "yes" : "no"}">${good ? "✓" : "○"}</span><span>${good ? yes : no}</span>`;

  const ICON = {
    playstation: `<svg viewBox="0 0 64 40"><path d="M14 6h36c7 0 12 6 12 14s-3 16-9 16c-4 0-6-4-9-8H20c-3 4-5 8-9 8-6 0-9-8-9-16S7 6 14 6z"/><circle cx="46" cy="16" r="2.5"/><circle cx="51" cy="21" r="2.5"/><circle cx="41" cy="21" r="2.5"/><circle cx="46" cy="26" r="2.5"/><path d="M14 21h8M18 17v8"/></svg>`,
    xbox: `<svg viewBox="0 0 64 40"><path d="M14 6h36c7 0 12 6 12 14s-3 16-9 16c-4 0-6-4-9-8H20c-3 4-5 8-9 8-6 0-9-8-9-16S7 6 14 6z"/><circle cx="46" cy="14" r="3"/><circle cx="46" cy="27" r="3"/><circle cx="40" cy="20.5" r="3"/><circle cx="52" cy="20.5" r="3"/><circle cx="18" cy="16" r="4"/></svg>`,
    keyboard: `<svg viewBox="0 0 64 40"><rect x="3" y="8" width="44" height="26" rx="4"/><path d="M9 15h4M17 15h4M25 15h4M33 15h4M9 21h4M17 21h4M25 21h4M33 21h4M13 27h24"/><path d="M54 8c5 0 7 4 7 9v8c0 5-3 9-7 9s-7-4-7-9v-8c0-5 2-9 7-9zM54 8v9"/></svg>`,
  };

  function dots() {
    [...$("wl-dots").children].forEach((d, i) => d.className = i === W.step ? "on" : i < W.step ? "done" : "");
    $("wl-back").style.visibility = W.step ? "visible" : "hidden";
    $("wl-next").textContent = W.step === STEPS - 1 ? "Pick a build" : W.step === 1 && !ready() ? "Skip for now" : "Next";
  }

  const ready = () => W.status && W.status.claude.installed && W.status.claude.logged_in;

  // Redraw only when a check changes, so typing in the page isn't interrupted.
  const checks = s => s ? JSON.stringify([s.claude, s.git, s.game_log]) : "";
  async function refresh() {
    const before = checks(W.status);
    W.status = await invoke("setup_status");
    if (W.input === null) { W.input = W.status.input; W.league = W.status.league; }
    if ((W.step === 1 || W.step === 2) && checks(W.status) !== before) render();
    banner();
  }

  function render() {
    const s = W.status;
    const body = $("wl-body");
    if (W.step === 0) {
      body.innerHTML = `<div class="wl-hero"><img src="logo.svg" alt="" class="wl-logo">
        <h1 id="wl-title">Welcome to Lifeline</h1>
        <p class="wl-tag">Your lifeline through Path of Exile 2, built for hardcore.</p>
        <p class="wl-note">Lifeline reads your game log and Build Planner folder and never touches the game itself. Your chats go to Claude through your own account.</p></div>
        <div class="wl-grid">
          <div><h3>Ask anything</h3><p>Gear, gems, bosses, what to buy. Answers come from current game data, not guesses.</p></div>
          <div><h3>Builds that work</h3><p>Pick from researched hardcore builds and get a full Act 1 to Endgame plan in the game's Build Planner.</p></div>
          <div><h3>A HUD over the game</h3><p>Your build grade, resistance warnings and one-click checks, without leaving the game.</p></div>
          <div><h3>Market and travel</h3><p>Find upgrades for your budget and jump to the seller's hideout.</p></div>
        </div>`;
    } else if (W.step === 1) {
      const c = s ? s.claude : {};
      const plan = c.subscription ? `${c.subscription[0].toUpperCase()}${c.subscription.slice(1)} plan` : "Claude plan";
      body.innerHTML = `<h1 id="wl-title">Connect Claude</h1>
        <p>Lifeline's AI runs on <b>your own Claude Pro or Max plan</b> through Claude Code, Anthropic's free app.
           There's no API key and no extra bill; answers use your plan's normal usage.</p>
        <ul class="wl-checks">
          <li>${ok(c.installed, `Claude Code installed <small>${esc(c.version || "")}</small>`, "Install Claude Code")}
              ${c.installed ? "" : `<button class="primary" data-act="install_claude">Install</button>`}</li>
          <li>${ok(s && s.git, "Git for Windows installed", "Install Git for Windows <small>(Claude Code needs it on Windows)</small>")}
              ${s && s.git ? "" : `<button data-act="install_git">Install</button>`}</li>
          <li>${ok(c.logged_in, `Signed in <small>${esc(plan)}${c.email ? " · " + esc(c.email) : ""}</small>`, "Sign in with your Claude account")}
              ${c.logged_in || !c.installed ? "" : `<button class="primary" data-act="login">Sign in</button>`}</li>
        </ul>
        ${c.logged_in && !c.uses_subscription ? `<p class="wl-warn">You're signed in with an API (Console) account, so usage is billed per call. To use your subscription, sign in again with your Claude account.</p>` : ""}
        <p class="wl-note">Install and sign-in open a window you can watch. This page updates on its own when each step is done.</p>`;
      body.querySelectorAll("[data-act]").forEach(b => b.addEventListener("click", async () => {
        try { await invoke("setup_action", { action: b.dataset.act }); b.textContent = "Opened…"; b.disabled = true; }
        catch (e) { toast(e, "err"); }
      }));
    } else if (W.step === 2) {
      body.innerHTML = `<h1 id="wl-title">Your game</h1>
        <ul class="wl-checks"><li>${ok(s && s.game_log, "Path of Exile 2 found", "Path of Exile 2 not found yet")}</li></ul>
        ${s && !s.game_log ? `<p class="wl-note">Start the game once; Lifeline finds it on any Steam library drive or the standalone client.</p>` : ""}
        <h3>How do you play?</h3>
        <div class="wl-inputs">${[["playstation", "PlayStation controller"], ["xbox", "Xbox controller"], ["keyboard", "Mouse and keyboard"]].map(([v, n]) =>
          `<button class="wl-input ${W.input === v ? "on" : ""}" data-v="${v}">${ICON[v]}<span>${n}</span></button>`).join("")}</div>
        <h3>League</h3>
        <input id="wl-league" list="leagues" value="${esc(W.league)}" placeholder="HC Forbidden Rites">
        <p class="wl-note">Set the game to <b>Windowed Fullscreen</b> (Options → Graphics) so the HUD can sit on top.</p>`;
      body.querySelectorAll(".wl-input").forEach(b => b.addEventListener("click", () => { W.input = b.dataset.v; render(); }));
      $("wl-league").addEventListener("input", e => { W.league = e.target.value; });
    } else if (W.step === 3) {
      const k = (settings && settings.hotkeys) || {};
      body.innerHTML = `<h1 id="wl-title">How it works</h1>
        <div class="wl-grid">
          <div><h3>HUD</h3><p>A small bar over the game, only while PoE2 is in front. Shows your grade (click to rate), resistance penalty, check item, what next, rotation and record gear. Drag the logo to move it.</p></div>
          <div><h3>Your gear</h3><p>The game doesn't share what you wear, so record it once: Play → Your gear → <b>Record my gear</b>, then in game point at each worn item and press <kbd>Ctrl+C</kbd>. Ratings and checks use it.</p></div>
          <div><h3>Hotkeys</h3><p><kbd>${esc(k.item_check)}</kbd> check the hovered item · <kbd>${esc(k.what_next)}</kbd> what next · <kbd>${esc(k.record_equipped)}</kbd> record worn gear · <kbd>${esc(k.toggle_overlay)}</kbd> HUD on/off. Change them in Settings.</p></div>
          <div><h3>Builds</h3><p>Builds → Showcase: pick class, ascendancy and what matters. <b>Create full build</b> plans every stage, and one click writes it into the game's Build Planner.</p></div>
          <div><h3>Rating and Skills</h3><p>A strict F to S+ grade for where you are, plus skills, supports and rotations for your controls.</p></div>
          <div><h3>Market</h3><p>Ask in chat ("find boots with life and cold res under 5 ex"). Press <b>Trade login</b> once so <b>Travel</b> can take you to the seller.</p></div>
          <div><h3>Sounds</h3><p>Cues for a new act, boss areas, resistance drops and answers being ready. Level-up and death sounds are optional. Turn each on or off in Settings → Sounds.</p></div>
        </div>
        <p class="wl-note">Keyboard: <kbd>Ctrl</kbd>+<kbd>1</kbd>–<kbd>5</kbd> switch tabs · <kbd>/</kbd> jump to chat · <kbd>Esc</kbd> closes panels · <kbd>Tab</kbd> moves between controls.</p>`;
    } else {
      body.innerHTML = `<div class="wl-hero"><img src="logo.svg" alt="" class="wl-logo">
        <h1 id="wl-title">You're set</h1>
        <p class="wl-tag">Pick a build to follow, or just start playing. Lifeline watches your game log and keeps up.</p></div>`;
    }
    dots();
  }

  async function finish() {
    try {
      const s = await invoke("finish_setup", { league: W.league, input: W.input || "keyboard" });
      settings = s;
      if (typeof fillSettings === "function") fillSettings(s);
    } catch (e) { toast(e, "err"); }
    close();
  }

  function go(step) {
    if (step >= STEPS) { finish().then(() => openShowcase()); return; }
    W.step = Math.max(0, step);
    render();
  }

  function open(step = 0) {
    $("welcome").classList.remove("hidden");
    W.step = step;
    render();
    refresh();
    clearInterval(W.poll);
    W.poll = setInterval(() => { if (W.step === 1 || W.step === 2) refresh(); }, 3000);
    $("wl-next").focus();
  }

  function close() {
    clearInterval(W.poll);
    $("welcome").classList.add("hidden");
    if (W.status && !W.status.onboarded) {
      invoke("finish_setup", { league: W.league, input: W.input || "keyboard" })
        .then(() => invoke("snapshot"))
        .then(snap => { settings = snap.settings; fillSettings(snap.settings); })
        .catch(e => toast(`Couldn't save the welcome answers: ${e}`, "err"));
    }
    refresh();
  }

  // Play tab banner when the AI can't run yet.
  function banner() {
    $("setup-banner").classList.toggle("hidden", !W.status || ready());
  }

  $("wl-next").addEventListener("click", () => go(W.step + 1));
  $("wl-back").addEventListener("click", () => go(W.step - 1));
  $("btn-welcome").addEventListener("click", () => open(0));
  $("setup-banner-go").addEventListener("click", () => open(1));
  document.addEventListener("keydown", ev => {
    if ($("welcome").classList.contains("hidden")) return;
    if (ev.key === "Escape") { ev.preventDefault(); close(); }
    else if ((ev.key === "ArrowRight" && ev.target.tagName !== "INPUT") || (ev.key === "Enter" && ev.target.tagName !== "INPUT" && ev.target.tagName !== "BUTTON")) { ev.preventDefault(); go(W.step + 1); }
    else if (ev.key === "ArrowLeft" && ev.target.tagName !== "INPUT") { ev.preventDefault(); go(W.step - 1); }
  });

  window.openWelcome = open;
  (async () => {
    await refresh();
    if (!W.status.onboarded) open(0);
  })();
})();

// ---- app-wide keyboard shortcuts ----
document.addEventListener("keydown", ev => {
  if (!$("welcome").classList.contains("hidden")) return;
  const typing = ["INPUT", "TEXTAREA", "SELECT"].includes(ev.target.tagName);
  if (ev.ctrlKey && !ev.altKey && ev.key >= "1" && ev.key <= "5") {
    const tab = document.querySelectorAll(".tab")[Number(ev.key) - 1];
    if (tab) { ev.preventDefault(); tab.click(); tab.focus(); }
  } else if (ev.key === "/" && !typing) {
    ev.preventDefault();
    document.querySelector('.tab[data-tab="play"]').click();
    $("ask-input").focus();
  }
});

// ---- "new version" notice ----
(async () => {
  try {
    const u = await invoke("check_update");
    if (u && u.version) {
      const b = $("update");
      b.textContent = `Lifeline ${u.version} is out · Download`;
      b.classList.remove("hidden");
      b.addEventListener("click", () => invoke("open_url", { url: u.url }));
    }
  } catch (_) { /* offline: no notice */ }
})();