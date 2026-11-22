// Film 4 page: loads fonts, footage stills and data, builds the stage, and exposes
//   window.seek(t)                 draw film time t for the page's own variant (?lang, ?labels)
//   window.frameVariants(t, list)  draw t once in 3D, then each {lang, labels} overlay; returns PNG data URLs (renderer)
'use strict';
import { Stage } from './stage.js';
import { Ov } from './ov.js';
import { Shots } from './shots.js';
const Q = new URLSearchParams(location.search);
const LANG = Q.get('lang') === 'ja' ? 'ja' : 'en', LABELS = Q.get('labels') !== '0', SCALE = Number(Q.get('scale') || 1) === 2 ? 2 : 1;
window.READY = undefined;
(async () => {
  try {
    const O = window.NF_ONSCREEN4;
    const strings = (lang) => { const S = {}; for (const [k, v] of Object.entries(O)) S[k] = v[lang]; return S; };
    // every character the film sets, so the right font subsets are loaded before the first frame
    const D = window.NF4; let all = JSON.stringify(O) + JSON.stringify(D.base.ui) + JSON.stringify(D.base.params) + JSON.stringify(D.base.checkJa) + JSON.stringify(D.base.simple) + JSON.stringify(D.base.gates6) + D.dxf.name;
    for (const w of [500, 560, 600, 650, 700, 750]) { await document.fonts.load(`${w} 48px 'Inter Variable'`, all); await document.fonts.load(`${w} 48px 'Noto Sans JP'`, all); }
    await document.fonts.load(`500 40px 'JetBrains Mono'`, all);
    await document.fonts.ready;
    const img = (src) => new Promise((res, rej) => { const im = new Image(); im.onload = () => im.decode().then(() => res(im), () => res(im)); im.onerror = () => rej(new Error('image ' + src)); im.src = src; });
    const foot = {};
    for (const l of ['en', 'ja']) foot[l] = { result: await img(`footage/${l}/result-33.webp`), graph: await img(`footage/${l}/am_post-250.webp`) };
    const st = new Stage(SCALE);
    const shots = new Shots(st, foot);
    const cv = document.getElementById('gfx'); cv.width = 1920 * SCALE; cv.height = 1080 * SCALE;
    const ctx = cv.getContext('2d', { alpha: false });
    // the set: a calm bright backdrop (soft top light, gentle floor gradient), painted behind the 3D layer
    const bg = document.createElement('canvas'); bg.width = cv.width; bg.height = cv.height; { const b = bg.getContext('2d'); b.scale(SCALE, SCALE);
      const g = b.createLinearGradient(0, 0, 0, 1080); g.addColorStop(0, '#f8f9fb'); g.addColorStop(0.55, '#f1f2f5'); g.addColorStop(1, '#e6e8ec'); b.fillStyle = g; b.fillRect(0, 0, 1920, 1080);
      const r = b.createRadialGradient(1100, 380, 0, 1100, 380, 1100); r.addColorStop(0, 'rgba(255,255,255,0.75)'); r.addColorStop(1, 'rgba(255,255,255,0)'); b.fillStyle = r; b.fillRect(0, 0, 1920, 1080); }
    const ov = new Ov(ctx);
    const compose = (draw, lang, labels) => {
      ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.globalAlpha = 1; ctx.drawImage(bg, 0, 0); ctx.drawImage(st.canvas, 0, 0);
      ctx.setTransform(SCALE, 0, 0, SCALE, 0, 0); ov.lang = lang;
      draw(ov, { lang, labels, S: strings(lang) });
    };
    window.seek = (t) => { const draw = shots.frame(t); compose(draw, LANG, LABELS); };
    window.frameVariants = (t, list) => { const draw = shots.frame(t); return list.map((v) => { compose(draw, v.lang, v.labels); return cv.toDataURL('image/png'); }); };
    // WebGL check for the renderer: share of the 3D layer's pixels that are drawn (not transparent), and their mean value
    window.glCheck = (t) => { shots.frame(t); const c = document.createElement('canvas'); c.width = 480; c.height = 270; const x = c.getContext('2d'); x.drawImage(st.canvas, 0, 0, 480, 270);
      const d = x.getImageData(0, 0, 480, 270).data; let n = 0, sum = 0; for (let i = 0; i < d.length; i += 4) if (d[i + 3] > 8) { n++; sum += (d[i] + d[i + 1] + d[i + 2]) / 3; } return { drawn: n / (480 * 270), mean: n ? sum / n : 0 }; };
    window.seek(0);
    window.READY = true;
  } catch (e) { console.error(e); window.READY_ERROR = String(e && e.stack || e); window.READY = false; }
})();
