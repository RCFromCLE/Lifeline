// Drifting embers behind the header: a little life in the frame.
(() => {
  const canvas = document.getElementById("embers");
  if (!canvas || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const ctx = canvas.getContext("2d");
  let w = 0, h = 0, embers = [];
  const spawn = (anywhere) => ({
    x: Math.random() * w,
    y: anywhere ? Math.random() * h : h + 4,
    r: 0.6 + Math.random() * 1.8,
    vy: 0.15 + Math.random() * 0.45,
    vx: (Math.random() - 0.5) * 0.25,
    life: 0.5 + Math.random() * 0.5,
    t: Math.random() * Math.PI * 2,
  });
  function resize() {
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    w = rect.width; h = rect.height;
    canvas.width = w * dpr; canvas.height = h * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    embers = Array.from({ length: Math.round(w / 28) }, () => spawn(true));
  }
  function frame() {
    if (!document.hidden) {
      ctx.clearRect(0, 0, w, h);
      for (const e of embers) {
        e.t += 0.04; e.y -= e.vy; e.x += e.vx + Math.sin(e.t) * 0.12;
        const fade = Math.max(0, Math.min(1, e.y / h)) * e.life * (0.75 + 0.25 * Math.sin(e.t * 2));
        const g = ctx.createRadialGradient(e.x, e.y, 0, e.x, e.y, e.r * 4);
        g.addColorStop(0, `rgba(255, 200, 120, ${0.9 * fade})`);
        g.addColorStop(0.4, `rgba(232, 116, 58, ${0.5 * fade})`);
        g.addColorStop(1, "rgba(232, 116, 58, 0)");
        ctx.fillStyle = g;
        ctx.beginPath(); ctx.arc(e.x, e.y, e.r * 4, 0, Math.PI * 2); ctx.fill();
        if (e.y < -8) Object.assign(e, spawn(false));
      }
    }
    requestAnimationFrame(frame);
  }
  addEventListener("resize", resize);
  resize();
  frame();
})();