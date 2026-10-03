// ---- Settings → Sounds & notifications ----
// Every sound cue gets a description and two switches: Sound (the existing
// Settings checkbox, saved with the form) and Pop-up (its 3-second HUD
// message, saved on its own). Plus a switch for the HUD's short messages.
(() => {
  const CUES = {
    level_up: ["Level up", "Your character gains a level. Pop-up: the new level and a reminder to spend the passive point."],
    new_act: ["New act", "You enter the next act. Pop-up: which act just began."],
    boss_area: ["Boss area", "You enter an area with a boss. Pop-up: the boss names, so you can get ready."],
    penalty: ["Resistance penalty", "The elemental resistance penalty gets worse (a new act or the endgame). Pop-up: the new penalty, so you check your caps."],
    death: ["Death", "Your character dies. Pop-up: where it happened."],
    ready: ["Ready", "Something you started finished: a hotkey answer, a rating, a new build, new market picks, or a recorded gear piece. Pop-up: which one."],
  };
  const tests = document.querySelector("#settings-form .sound-tests");
  if (!tests) return;

  const heading = [...document.querySelectorAll("#settings-form h2.sub-h")].find(h => h.textContent.trim() === "Sounds");
  if (heading) {
    heading.textContent = "Sounds & notifications";
    heading.insertAdjacentHTML("afterend", `<p class="hint">Each cue can play a sound, show a 3-second message under the HUD, both, or neither. The message works even with the sound off.</p>`);
  }

  tests.classList.add("cue-table");
  tests.insertAdjacentHTML("afterbegin", `<div class="cue-head"><span></span><span>Sound</span><span>Pop-up</span><span></span></div>`);
  for (const row of tests.querySelectorAll(".cue")) {
    const play = row.querySelector("button[data-cue]");
    const key = play?.dataset.cue;
    const box = row.querySelector('input[type="checkbox"]');
    if (!key || !box || !CUES[key]) continue;
    const [title, about] = CUES[key];
    row.innerHTML = "";
    row.insertAdjacentHTML("beforeend", `<div class="cue-about"><b>${title}</b><span>${about}</span></div>`);
    const sound = document.createElement("label");
    sound.className = "cue-switch";
    sound.title = `Play the ${title} sound`;
    sound.append(box);
    const pop = document.createElement("label");
    pop.className = "cue-switch";
    pop.title = `Show the ${title} message on the HUD`;
    pop.innerHTML = `<input type="checkbox" data-pop="${key}" checked>`;
    row.append(sound, pop, play);
  }

  // HUD messages (gear recorded, Travel results, problems).
  tests.insertAdjacentHTML("afterend", `
    <label class="check cue-extra"><input type="checkbox" id="hud-notices" checked>
      <span><b>HUD messages</b> <span class="hint">Short notes under the HUD: gear recorded, Travel results, and problems such as "point at an item first".</span></span></label>`);
  const answers = document.querySelector('#settings-form input[name="overlay_on_hotkey"]')?.closest("label");
  if (answers && !answers.querySelector(".hint")) {
    answers.insertAdjacentHTML("beforeend", ` <span class="hint">: hotkey answers (What next, Item check) open in a small panel under the HUD.</span>`);
  }

  function show(s) {
    tests.querySelectorAll("input[data-pop]").forEach(i => { i.checked = s.cue_popups?.[i.dataset.pop] !== false; });
    $("hud-notices").checked = s.hud_notices !== false;
  }
  invoke("notification_settings").then(show).catch(() => {});
  listen("notification-settings", ({ payload }) => show(payload));

  // These save on their own, not through the Settings form.
  const save = async (ev, kind, cue) => {
    ev.stopPropagation();
    try { show(await invoke("set_notification", { kind, cue, on: ev.target.checked })); }
    catch (e) { ev.target.checked = !ev.target.checked; toast(`Couldn't save: ${e}`, "err"); }
  };
  tests.querySelectorAll("input[data-pop]").forEach(i => i.addEventListener("change", ev => save(ev, "popup", i.dataset.pop)));
  $("hud-notices").addEventListener("change", ev => save(ev, "hud_notices", null));
})();

// ---- Settings → About: the version you have and the latest release ----
(() => {
  const LATEST = "https://github.com/RCFromCLE/Lifeline/releases/latest";
  const version = $("app-version");
  if (version) {
    Promise.resolve(window.__TAURI__?.app?.getVersion?.())
      .then(v => { version.textContent = v ? `v${v}` : "(version unknown)"; })
      .catch(() => { version.textContent = "(version unknown)"; });
  }
  $("btn-releases")?.addEventListener("click", () =>
    invoke("open_url", { url: LATEST }).catch(e => toast(`Couldn't open the browser: ${e}`, "err")));
})();
