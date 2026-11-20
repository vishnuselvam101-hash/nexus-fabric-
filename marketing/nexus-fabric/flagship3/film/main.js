// Start-up and the frame function. window.seek(t) draws the frame at film time t; render.js calls it for every frame.
// Revision 5: no dissolves. An outgoing scene leaves between 0.233 s and 0.1 s before its bar line (fade to zero and rise
// 12 px in four frames); the incoming scene starts on the bar. One text zone per scene element, so nothing overlaps at a change.
(async function () {
  'use strict';
  const F = window.F3, ctx = F.ctx, C = F.C;
  const STARTS = [0, 4, 8, 12, 20, 26, 30, 42, 46, 50];   // the locked slot grid (bar lines)
  const EXIT = 0.233, GAP = 0.1;                            // outgoing: from bar-0.233 (fade and rise) to bar-0.1 (gone)
  const REAL = [[10.0, 12.0], [37.0, 40.0]];                // the two real-application moments: no lockup over them

  function paper(t) {
    ctx.setTransform(F.S, 0, 0, F.S, 0, 0);
    ctx.fillStyle = C.paper; ctx.fillRect(0, 0, F.W, F.H);
    // a drafting grid that drifts slowly: something is in motion in every frame
    ctx.save(); ctx.strokeStyle = 'rgba(168,177,189,0.10)'; ctx.lineWidth = 1; ctx.beginPath();
    const ox = (t * 9) % 120, oy = (t * 5) % 120;
    for (let x = ox - 120; x < F.W + 120; x += 120) { ctx.moveTo(x, 0); ctx.lineTo(x, F.H); }
    for (let y = oy - 120; y < F.H + 120; y += 120) { ctx.moveTo(0, y); ctx.lineTo(F.W, y); }
    ctx.stroke(); ctx.restore();
  }
  function drawScene(i, time, dy, alpha) {
    F.t = time; ctx.save(); ctx.translate(0, dy); ctx.globalAlpha = Math.max(0, Math.min(1, alpha)); F.scenes[i](time); ctx.restore();
  }
  function lockup(t) {
    if (t < 2.0 || t >= 50.0) return;
    if (REAL.some(([a, b]) => t >= a && t < b)) return;
    const a = F.SP(t, 2.0, 10, 0.8);
    if (a <= 0.001) return;
    const mk = window.NFG && window.NFG.mark ? window.NFG.mark() : null;
    ctx.save(); ctx.globalAlpha = Math.min(1, a);
    if (mk) {
      const S = 3.2, mx = (x) => 120 + (x - 2) * S, my = (y) => 50 + (y - 2) * S;
      F.line(mk.legs.map((p) => [mx(p[0]), my(p[1])]), { color: C.ink, lw: 5 });
      F.line(mk.arc.map((p) => [mx(p[0]), my(p[1])]), { color: C.ink, lw: 5 });
    }
    F.txt(F.on('brand_name')[0], 176, 86, { size: 40, w: 700, color: C.ink, lang: 'en', tr: 0.06 });
    ctx.restore();
  }
  window.seek = (t) => {
    F.t = t;
    paper(t);
    let i = 0; while (i < STARTS.length - 1 && t >= STARTS[i + 1]) i++;
    const nb = i + 1 < STARTS.length ? STARTS[i + 1] : Infinity;   // the bar after scene i
    if (nb - EXIT <= t && t < nb - GAP) {
      const k = (t - (nb - EXIT)) / (EXIT - GAP);                   // 0 -> 1 over four frames
      drawScene(i, nb - EXIT - 1 / 120, -12 * k, 1 - k);            // the outgoing scene, frozen, fading and rising
    } else if (!(nb - GAP <= t && t < nb)) {                         // between the two: nothing (background and lockup only)
      drawScene(i, t, 0, 1);
    }
    lockup(t);
  };
  try {
    await Promise.all([
      document.fonts.load('600 40px "Inter Variable"'), document.fonts.load('500 40px "Inter Variable"'),
      document.fonts.load('700 40px "Noto Sans JP"'), document.fonts.load('500 40px "Noto Sans JP"'),
    ]);
    await new Promise((res, rej) => { const s = document.createElement('script'); s.src = `capture/footage/${F.LANG}/footage.js`; s.onload = res; s.onerror = () => rej(new Error('no footage for ' + F.LANG)); document.head.appendChild(s); });
    const Dft = window.NF_FOOTAGE[F.LANG];
    if (!String(Dft.html_lang || '').startsWith(F.LANG)) throw new Error('footage captured in ' + Dft.html_lang);
    F.foot = new window.NFE.Footage(Dft, `capture/footage/${F.LANG}`);
    await F.foot.load();
    window.NF_CUES = window.NF_CUES_FIXED;
    window.NF_INFO = { lang: F.LANG, duration: 54, frames: 1620 };
    window.seek(0);
    window.READY = true;
  } catch (e) {
    console.error('film failed: ' + ((e && e.stack) || e));
    window.READY = 'error';
  }
})();
