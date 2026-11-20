// NEXUS FABRIC flagship film 2 — the timeline and the drawing. window.seek(seconds) draws one exact frame; nothing depends on a clock.
// Look (owner's decision): bright and clean, product-launch rhythm. Off-white wallpaper with a slow cool wash; one large
// product window (87 % of the frame width, 18 px corners, a soft wide shadow); one statement line per scene (96–132 px,
// centred, 350 ms fade with a 24 px rise); small print 22 px mid-grey; steel drawn as solid light metal; one accent blue
// for the cut; amber only where the product itself carries NOT RELEASED. The camera has no tilt and always covers the window.
'use strict';
(async () => {
  const { clamp, lerp, seg, E, spring, springTrack, track, Footage, Reel, flatProject, rigTransform } = window.NFE;
  const G = window.NFG, $ = (s) => document.querySelector(s);
  const QS = new URLSearchParams(location.search), LANG = QS.get('lang') === 'ja' ? 'ja' : 'en';
  const stage = $('#stage'); stage.classList.add(LANG); document.documentElement.lang = LANG;
  const DPR = window.devicePixelRatio || 1;

  // ---------------------------------------------------------------- look
  const C = { paper: '#f5f5f7', ink: '#1d1d1f', muted: '#6e6e73', faint: '#c7c7cc', acc: '#2f6bff', amber: '#c77800' };
  const WIN = { L: 125, T: 36, W: 1670, H: 940 }; WIN.R = WIN.L + WIN.W; WIN.B = WIN.T + WIN.H;
  const REST = { z: WIN.W / 1920, sx: 960, sy: WIN.T + WIN.H / 2 };          // the whole product frame fits the window
  const WT = LANG === 'ja' ? { st: 700, reg: 400 } : { st: 600, reg: 400 };
  const FAM = LANG === 'ja' ? '"Noto Sans JP", "Inter Variable"' : '"Inter Variable", "Noto Sans JP"';
  const fnt = (px, w = WT.reg, it = false) => `${it ? 'italic ' : ''}${w} ${px}px ${FAM}`;

  // ---------------------------------------------------------------- data: the footage of this language only
  const loadScript = (src) => new Promise((res) => { const s = document.createElement('script'); s.src = src; s.onload = () => res(true); s.onerror = () => res(false); document.head.appendChild(s); });
  if (!(await loadScript(`capture/footage/${LANG}/footage.js`))) throw new Error(`no footage for ${LANG}`);
  const D = window.NF_FOOTAGE[LANG], foot = new Footage(D, `capture/footage/${LANG}`);
  if (!String(D.html_lang || '').startsWith(LANG)) throw new Error(`the ${LANG} footage was captured in interface ${D.html_lang}`);
  const TM = window.NF_TIMING, DUR = TM.duration, SL = TM.slots.map((s) => [s.start, s.end]), L = (i) => SL[i - 1];
  const TXT = window.NF_SCRIPT[LANG].onscreen, JOB = D.job, REQ = JOB.request;
  await Promise.all([foot.load(), document.fonts.ready]);
  const allText = JSON.stringify(TXT) + 'φβ√−·×→°0123456789.,:-–—()[]/%';
  await Promise.all([fnt(132, WT.st), fnt(120, WT.st), fnt(48), fnt(32), fnt(30), fnt(26), fnt(24), fnt(22), fnt(72), fnt(72, WT.reg, true), fnt(30, WT.reg, true)].map((f) => document.fonts.load(f, allText)));

  // ---------------------------------------------------------------- geometry of the job (product output)
  const J = G.joint(REQ);
  const PB = JOB.patterns.branch, PH = JOB.patterns.header;
  const shB = G.branchSheet(J, PB), shH = G.headerSheet(J, PH);
  const SAD = shB.saddle, NS = SAD.length;                                 // the saddle cut's vertices [x = r*phi, y = s(phi)]
  const PRE = { ...REQ, beta_deg: 90, branch_station: 450 };               // the joint the form starts from (for the opening illustration)
  const synth = (Jx, Jpos) => { const n = 360, C2 = 2 * Math.PI * Jx.r, sad = []; for (let i = 0; i <= n; i++) { const p = (2 * Math.PI * i) / n; sad.push([Jx.r * p, Jx.s(p)]); }
    const hole = sad.slice(0, n).map(([x, y]) => { const P = Jpos.branch(x, y, 1); return [P[0], Math.PI * Jpos.R + Jpos.R * Math.atan2(P[1], P[2])]; });
    return { branch: { curves: [{ name: 'saddle_cut', points: sad }, { name: 'top', points: [[0, Jx.send], [C2, Jx.send]] }], holes: [] }, header: { curves: [], holes: [{ points: hole }] } }; };
  const J0 = G.joint(PRE), P0 = synth(J0, J0), sh0B = G.branchSheet(J0, P0.branch), sh0H = G.headerSheet(J0, P0.header);
  const J0w = G.joint({ ...PRE, header_radius: PRE.header_radius * 0.86 }), sh0W = G.branchSheet(J0, synth(J0w, J0).branch);   // a wrong cut: wrong header radius
  const DXF = D.dxf && D.dxf.header && D.dxf.header.text ? G.parseDXF(D.dxf.header.text) : null;
  const dxfRows = DXF ? D.dxf.header.text.split(/\r?\n/) : [];
  const chkRow = (() => { const tk = D.takes.checks; if (!tk || !tk.rows) return null; return tk.rows[tk.iface_row ?? tk.rows.length - 1]; })();
  const fmt3 = (v) => v.toLocaleString('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
  const humanName = (s) => String(s).replace(/\s*[A-Za-z]+(?:_[A-Za-z0-9]+)+\s*$/, '');

  // ---------------------------------------------------------------- the reel: film time -> captured frame
  const R = new Reel(foot);
  const sequence = (t0, t1, items) => { const d = items.map(([tk, a, b, r = 1]) => Math.max(1, Math.abs(foot.mark(tk, b) - foot.mark(tk, a))) / (30 * r)); const k = (t1 - t0) / d.reduce((x, y) => x + y, 0); let t = t0; items.forEach(([tk, a, b], i) => { R.fit(t, t + d[i] * k, tk, a, b); t += d[i] * k; }); return k; };
  const tMatch = L(2)[0] + 0.26;                       // the illustration's lines land on the schematic
  const tPress = L(2)[0] + 3.52;                       // Run is pressed
  const tHero0 = L(3)[0] - 0.55;                       // the curve comes out of the canvas
  const tTrace0 = tHero0 - 0.05, tTrace1 = L(4)[0] - 0.12;
  const tF0 = L(4)[0] - 0.10;                          // the plates arrive
  const tR0 = tF0 + 1.15, tC = tR0 + 2.25;             // lift and roll; the edges close at tC
  const tChk = tC + 1.05;                              // the product's interface check
  const tRec0 = L(5)[0] - 0.42, tRecPress = L(5)[0] + 0.38;
  const tGate0 = L(6)[0] - 0.30, tGate1 = L(7)[0] - 0.42;
  const tDxfPress = L(7)[0] + 1.02, tDxfOut = tDxfPress + 0.55, tPlot0 = tDxfOut + 0.35, tPlot1 = L(7)[1] - 0.25;
  const tEnd0 = L(8)[0] - 0.30, tBrand = L(9)[0] - 0.22;
  R.hold(0, tMatch + 0.34, 'form', 'start');
  sequence(tMatch + 0.34, tPress, [['form', 'station:move+18', 'station:done', 1.25], ['form', 'beta:move', 'sweep:start', 1.9], ['form', 'sweep:start', 'sweep:end', 1.0], ['form', 'sweep:end', 'run:move', 2.6], ['form', 'run:move', 'run:down', 1.9], ['form', 'run:down', 'run:up', 1.0]]);
  const tRes = R.play(tPress, 'form', 'run:up', 'end', 1.0);
  const tCanvas0 = R.play(tRes, 'result', 0, 'end', 1.0);
  sequence(tCanvas0, tTrace0, [['canvas', 'start', 'trace:start', 2.6], ['canvas', 'trace:start', 'zoom:start', 1.7], ['canvas', 'zoom:start', 'zoom:end+7', 1.0], ['canvas', 'fit:move', 'fitted+2', 2.6], ['canvas', 'tab:move', 'shown+5', 2.2], ['canvas', 'saddle:approach', 'saddle:start', 2.4]]);
  R.fit(tTrace0, tTrace1, 'canvas', 'saddle:start', 'saddle:end');
  R.hold(tTrace1, tC - 0.2, 'canvas', 'saddle:end');
  R.hold(tC - 0.2, tChk + 0.35, 'checks', 'iface:shown+19');
  R.fit(tChk + 0.35, tRec0, 'checks', 'iface:point', 'iface:hold+13');
  sequence(tRec0, tRecPress, [['records', 'replay:move', 'replay:down', 2.0], ['records', 'replay:down', 'replay:up', 1.0]]);
  const tRec1 = R.play(tRecPress, 'records', 'replay:up', 'replayed+22', 1.0);
  sequence(tRec1, tGate0, [['records', 'scroll:move', 'scroll:start', 2.2], ['records', 'scroll:start', 'scroll:end', 1.15], ['records', 'rec:move', 'rec:open+16', 1.5]]);
  sequence(tGate0, tGate1, [['gates', 'tab:move', 'shown+17', 1.9], ['gates', 'scroll:move', 'scroll:start', 2.0], ['gates', 'scroll:start', 'scroll:end+13', 1.0], ['gates', 'state:move', 'state+15', 1.35]]);
  sequence(tGate1, tDxfPress, [['dxf', 'header:move', 'header:shown+7', 2.2], ['dxf', 'dxf:move', 'dxf:down', 1.5], ['dxf', 'dxf:down', 'dxf:up', 1.0]]);
  R.play(tDxfPress, 'dxf', 'dxf:up', 'dxf:written+12', 1.0);
  const W = (take, m) => R.when(take, m);

  // ---------------------------------------------------------------- the product window: an ease-in-out camera, always covering the window
  const rcAt = (take, m, key, fx = 0.5, fy = 0.5) => foot.centre(take, foot.mark(take, m), key, fx, fy);
  const skR = foot.rect('form', 0, 'sketch_svg') || [577, 300, 858, 505];
  const zMatch = 1240 / skR[2], cMatch = { fx: skR[0] + skR[2] / 2, fy: skR[1] + skR[3] / 2, z: zMatch, sx: REST.sx, sy: REST.sy };
  const pvR = foot.rect('canvas', 0, 'pv') || [577, 155, 858, 875], pvC = [pvR[0] + pvR[2] / 2, pvR[1] + pvR[3] / 2];
  const stn = rcAt('form', 'station:done', 'station'), bet = rcAt('form', 'sweep:start', 'beta'), run = rcAt('form', 'run:down', 'run');
  const fEnd = foot.mark('canvas', 'saddle:end'), vS = foot.view('canvas', fEnd) || [1, 0, 0, pvR[0], pvR[1], pvR[2], pvR[3]];
  const sadMid = foot.toApp('canvas', fEnd, shB.C / 2, 170);
  const zDive = clamp(1500 / (shB.C * vS[0]), REST.z, 2.0);
  const cell = chkRow && chkRow.cells[2] ? chkRow.cells[2].rect : [1700, 300, 80, 20], cellC = [cell[0] + cell[2] / 2, cell[1] + cell[3] / 2];
  const rep = rcAt('records', 'replay:down', 'replay'), tabsR = foot.rect('gates', foot.mark('gates', 'shown'), 'tabs') || [1440, 117, 480, 34];
  const paneC = [tabsR[0] + tabsR[2] / 2, 560], stateC = rcAt('gates', 'state', 'state'), dxfC = rcAt('dxf', 'dxf:down', 'dxf');
  const fDxf = foot.mark('dxf', 'dxf:written+12'), vD = foot.view('dxf', fDxf) || vS;
  const plateC = foot.toApp('dxf', fDxf, J.L / 2, shH.C / 2 - 40), zPlate = clamp(900 / (J.L * vD[0]), REST.z, 2.0);
  const CAM = springTrack([
    { t: 0, ...cMatch },
    { t: tMatch + 0.42, w: 4.6, z: REST.z, fx: 960, fy: 540, sx: REST.sx, sy: REST.sy },
    { t: W('form', 'station:move+18') - 0.10, w: 5.4, fx: stn[0] + 300, fy: stn[1] + 60, z: 1.8 },
    { t: W('form', 'beta:move') + 0.02, w: 5.6, fx: bet[0] + 330, fy: bet[1] - 60, z: 1.9 },
    { t: W('form', 'sweep:start') + 0.25, w: 3.2, fx: bet[0] + 470, fy: bet[1] - 150, z: 2.0 },
    { t: W('form', 'run:move') - 0.02, w: 6.2, fx: run[0] + 250, fy: run[1] - 190, z: 1.9 },
    { t: tPress - 0.02, w: 5.0, z: REST.z, fx: 960, fy: 540 },
    { t: tCanvas0 - 0.25, w: 4.4, fx: pvC[0], fy: pvC[1] + 10, z: 1.4 },
    { t: W('canvas', 'tab:move') - 0.05, w: 5.0, fx: pvC[0] - 30, fy: pvC[1] - 120, z: 1.4 },
    { t: tHero0 - 0.12, w: 5.4, fx: sadMid[0], fy: sadMid[1], z: zDive },
    { t: tC - 0.3, w: 40, z: REST.z, fx: 960, fy: 540 },                       // hidden until the checks arrive: snap to the whole frame
    { t: tChk + 0.42, w: 4.6, fx: cellC[0] - 150, fy: cellC[1] - 60, z: 1.9 },
    { t: tRec0 + 0.02, w: 5.4, fx: rep[0] - 190, fy: rep[1] + 210, z: 1.9 },
    { t: tRecPress + 0.30, w: 4.8, fx: paneC[0] - 60, fy: 390, z: 2.0 },
    { t: W('records', 'rec:move') - 0.05, w: 4.4, fx: paneC[0] - 40, fy: 560, z: 2.0 },
    { t: tGate0 + 0.10, w: 5.2, fx: paneC[0] - 70, fy: 330, z: 1.9 },
    { t: W('gates', 'scroll:start') - 0.1, w: 4.0, fx: paneC[0] - 50, fy: 520, z: 1.9 },
    { t: W('gates', 'state:move') + 0.02, w: 5.0, fx: stateC[0] + 330, fy: stateC[1] + 150, z: 2.0 },
    { t: tGate1 + 0.02, w: 5.2, fx: dxfC[0] - 150, fy: dxfC[1] + 200, z: 1.9 },
    { t: tDxfPress + 0.06, w: 5.6, fx: plateC[0], fy: plateC[1], z: zPlate },
  ], ['z']);
  // the window always covers the frame: zoom never goes below the whole product frame, and the focus stays inside it
  const clampCam = (c) => { const z = Math.max(c.z, REST.z);
    return { ...c, z, fx: clamp(c.fx, (c.sx - WIN.L) / z, 1920 - (WIN.R - c.sx) / z), fy: clamp(c.fy, (c.sy - WIN.T) / z, 1080 - (WIN.B - c.sy) / z) }; };
  const winA = track([[0, 0], [tMatch - 0.10, 0], [tMatch + 0.22, 1, 'io2'], [tHero0 + 0.42, 1], [tHero0 + 0.92, 0, 'io2'], [tC + 0.45, 0], [tC + 0.95, 1, 'io2'], [tPlot0 - 0.30, 1], [tPlot0 + 0.25, 0, 'io2'], [DUR, 0]]);

  // ---------------------------------------------------------------- the layouts of the film's own drawings (on the wallpaper)
  const layH2 = { s: 0.80, ox: 194, oy: 960 }, layB2 = { s: 0.80, ox: 1224, oy: 960 };   // the two plates, flat, 80 % of the width
  const layHero = { s: 2.45, ox: (1920 - shB.C * 2.45) / 2, oy: 930 };                  // the curve: 80 % of the width
  const PANEL = { x0: 120, y0: 290, w: 960, h: 660 }, sDXF = Math.min(0.8 * PANEL.w / J.L, 0.8 * PANEL.h / shH.C);
  const layDXF = { s: sDXF, ox: PANEL.x0 + (PANEL.w - sDXF * J.L) / 2, oy: PANEL.y0 + PANEL.h - (PANEL.h - sDXF * shH.C) / 2 };                                          // the pattern panel: the plate fitted to 80 % of it and centred; the file's text in the right third
  const layAppAt = (take, f, c) => { const o = flatProject(c, ...foot.toApp(take, f, 0, 0)), v = foot.view(take, f); return { s: c.z * v[0], ox: o[0], oy: o[1] }; };
  const mixLay = (a, b, u) => ({ s: Math.exp(lerp(Math.log(a.s), Math.log(b.s), u)), ox: lerp(a.ox, b.ox, u), oy: lerp(a.oy, b.oy, u) });
  function heroLay(t, c) { const a = layAppAt('canvas', fEnd, c), u = spring(t - (tHero0 + 0.62), 4.6, 1); let l = mixLay(a, layHero, u); const v = spring(t - tF0, 8, 1); return v > 0 ? mixLay(l, layB2, v) : l; }

  // ---------------------------------------------------------------- DOM
  const win = $('#win'), rig = $('#rig'), curEl = $('#cur'), stmt = $('#stmt'), sec = $('#sec'), eqEl = $('#eq'), eqv = $('#eqv'), dxfname = $('#dxfname'), endEl = $('#end');
  rig.insertBefore(foot.cv, curEl); foot.cv.style.cssText = 'position:absolute;left:0;top:0;width:1920px;height:1080px';
  const gfx = $('#gfx'); gfx.width = 1920 * DPR; gfx.height = 1080 * DPR; const g = gfx.getContext('2d');
  const bgc = $('#bg'), bgx = bgc.getContext('2d');
  let curKind = '';
  const setTxt = (el, html) => { if (el._h !== html) { el.innerHTML = html; el._h = html; } };
  const style = (el, o) => { for (const k in o) if (el.style[k] !== o[k]) el.style[k] = o[k]; };
  const mctx = document.createElement('canvas').getContext('2d');
  const fitSize = (text, maxW, lo, hi, w) => { for (let px = hi; px >= lo; px -= 2) { mctx.font = fnt(px, w); if (mctx.measureText(text).width <= maxW) return px; } return lo; };

  // wallpaper: off-white with a very soft, slow, cool wash (the wash moves slowly; no black anywhere)
  function drawWallpaper(t) {
    bgx.fillStyle = C.paper; bgx.fillRect(0, 0, 1920, 1080);
    const blobs = [[430 + 170 * Math.sin(0.045 * t), 200 + 90 * Math.cos(0.038 * t), 900, 'rgba(200,220,242,0.55)'],
      [1420 + 150 * Math.cos(0.041 * t + 1), 760 + 120 * Math.sin(0.033 * t), 1050, 'rgba(214,226,242,0.50)'],
      [960 + 220 * Math.sin(0.029 * t + 2), 1180 + 70 * Math.cos(0.05 * t), 1000, 'rgba(226,234,246,0.45)']];
    for (const [cx, cy, r, col] of blobs) { const gr = bgx.createRadialGradient(cx, cy, 0, cx, cy, r); gr.addColorStop(0, col); gr.addColorStop(1, 'rgba(245,245,247,0)'); bgx.fillStyle = gr; bgx.fillRect(0, 0, 1920, 1080); }
  }

  // statement lines: one per scene, centred, 96–132 px; 350 ms fade with a 24 px rise; 250 ms clean exit
  const STM = [
    { text: TXT.hook, a: 1.55, b: 3.25, y: 118 },
    { text: TXT.hook2, a: 3.45, b: L(1)[1] - 0.2, y: 118 },
    { text: TXT.cap_fold, a: tF0 + 0.5, b: tChk - 0.45, y: 40, max: 112 },
    { text: TXT.cap_dxf, a: tPlot0 + 0.4, b: tEnd0 - 0.2, y: 56 },
  ];
  const stmState = (s, t) => { if (t < s.a || t > s.b + 0.25) return null; const i = E.io3(seg(t, s.a, s.a + 0.35)), o = 1 - E.io2(seg(t, s.b, s.b + 0.25)); return { op: i * o, y: (1 - i) * 24 }; };
  // secondary lines under the window (32 px) with a small-print source label (22 px); the film's own drawings carry their label here too
  const CAPS = [
    [1.2, L(1)[1], '', TXT.label_film],
    [tMatch + 0.95, tPress - 0.15, TXT.cap_form, TXT.label_actual],
    [tRes + 0.30, tHero0 - 0.15, TXT.cap_result, TXT.label_actual],
    [tCanvas0 + 0.25, tHero0 - 0.15, '', TXT.label_actual],
    [tHero0 + 1.25, tF0 - 0.10, TXT.cap_eq, TXT.label_output],
    [tF0 + 0.45, tChk - 0.35, '', TXT.label_film],
    [tChk - 0.2, tRec0 - 0.10, TXT.check_title, TXT.label_actual],
    [L(5)[0] - 0.05, L(6)[0] - 0.42, TXT.cap_records, TXT.label_actual],
    [L(6)[0] - 0.08, tGate1 - 0.25, TXT.cap_gates, TXT.label_actual],
    [tPlot0 + 0.4, tEnd0 - 0.2, '', TXT.label_output],
  ];
  const inOut = (t, a, b, din = 0.35, dout = 0.25) => (t < a || t > b ? null : { in: E.io3(seg(t, a, a + din)), out: 1 - E.io2(seg(t, b - dout, b)) });

  // the equation: s(phi) = (sqrt(R^2 - (r cos phi)^2) - r sin phi cos beta) / sin beta
  eqEl.innerHTML = `<span class="t" data-k="0"><i>s</i>(<i>φ</i>)</span><span class="t" data-k="1">&nbsp;=&nbsp;</span><span class="frac"><span class="numr"><span class="t sq" data-k="2"><svg viewBox="0 0 30 84" preserveAspectRatio="none"><path d="M1.5 50 L7 47 L14.5 80 L28.5 1.5 L30 1.5"/></svg><span class="in"><i>R</i><sup>2</sup> − (<i>r</i><span class="fn">cos</span><i>φ</i>)<sup>2</sup></span></span><span class="t" data-k="3">&nbsp;−&nbsp;<i>r</i><span class="fn">sin</span><i>φ</i><span class="fn">cos</span><i>β</i></span></span><span class="bar t" data-k="4"></span><span class="t" data-k="4"><span class="fn" style="padding-left:0">sin</span><i>β</i></span></span>`;
  const eqT = [...eqEl.querySelectorAll('.t')];
  const nums = {}; const num = (id) => nums[id] || (nums[id] = stage.insertBefore(Object.assign(document.createElement('div'), { className: 'sec' }), endEl));

  // end card: the lockup (mark + wordmark) on the centre line
  const WORD = 'NEXUS FABRIC';
  const wm = $('#wm'); wm.textContent = WORD;
  setTxt($('#pos'), TXT.positioning); setTxt($('#plan'), `<i></i>${TXT.planned_ai}`); setTxt($('#brand'), TXT.brand); setTxt($('#cta'), `<span>${TXT.cta}</span>`); setTxt($('#disc'), TXT.disclaimer);
  const tick = `<svg viewBox="0 0 20 20"><path d="M4 10.5l4 4 8-9" fill="none" stroke="${C.ink}" stroke-width="2.4" stroke-linecap="square" stroke-linejoin="miter"/></svg>`;
  setTxt($('#today'), `<span class="lead">${TXT.today}</span>` + TXT.today_items.map((s) => `<span class="it">${tick}<b>${s}</b></span>`).join(''));
  setTxt($('#credit'), TXT.credit || '');
  mctx.font = fnt(132, WT.st, false); mctx.letterSpacing = '-3px';
  const wordW = mctx.measureText(WORD).width;
  const MARK = G.mark(), mkS = 9.6, markW = 10.32 * mkS, lockGap = 40, lockW = markW + lockGap + wordW;
  const lockX = 960 - lockW / 2, lockY = 400;                                             // the mark's left edge and centre line
  const lockMarkX = lockX - 2 * 0.86 * mkS;                                               // the mark's own origin (its left edge sits at lockX)
  wm.style.transform = `translate(${(lockX + markW + lockGap).toFixed(1)}px,${(lockY - 66).toFixed(1)}px)`;

  // ---------------------------------------------------------------- sound cues (read by audio/make_music.py)
  const CUES = []; const cue = (t, k, o = {}) => { if (t !== null && t !== undefined && t >= 0 && t < DUR) CUES.push({ t: +t.toFixed(3), k, ...o }); };
  cue(0.15, 'line_draw', { d: 1.0 }); cue(1.32, 'drop_in', { d: 0.5 }); cue(1.86, 'seat'); cue(2.55, 'ignite', { d: 0.9 }); cue(4.05, 'misfit'); cue(5.50, 'seat');
  cue(L(1)[1] + 0.22, 'turn', { d: tMatch - L(1)[1] - 0.2 }); cue(tMatch, 'match'); cue(tMatch + 0.4, 'window_out', { d: 0.8 });
  cue(W('form', 'station:down'), 'click'); for (const m of ['station:key0', 'station:key1', 'station:key2']) cue(W('form', m), 'key');
  cue(W('form', 'beta:down'), 'click'); { const a = foot.mark('form', 'sweep:start'), b = foot.mark('form', 'sweep:end'); for (let f = a; f < b; f++) cue(W('form', f), 'detent', { i: f - a, n: b - a }); }
  cue(W('form', 'run:down'), 'press'); cue(tPress, 'release'); cue(tRes, 'result'); cue(tRes + 0.45, 'toast');
  cue(W('canvas', 'trace:start'), 'trace', { d: W('canvas', 'zoom:start') - W('canvas', 'trace:start') });
  { const a = foot.mark('canvas', 'zoom:start'); for (let i = 0; i < 7; i++) cue(W('canvas', a + 2 * i), 'notch', { i }); }
  cue(W('canvas', 'fit:down'), 'click'); cue(W('canvas', 'tab:down'), 'click'); cue(W('canvas', 'shown'), 'swap');
  cue(tHero0 - 0.1, 'dive', { d: 1.0 }); cue(tHero0 + 0.95, 'eq_on', { d: 1.2 });
  for (const q of [0.25, 0.5, 0.75, 1]) cue(lerp(tTrace0, tTrace1, q), 'quarter', { i: Math.round(q * 4) });
  cue(tTrace0 + 0.5, 'trace', { d: tTrace1 - tTrace0 - 0.6 });
  cue(tF0 + 0.1, 'plate_in', { d: 0.8 }); cue(tR0, 'lift', { d: 1.0 }); cue(tR0 + 0.5, 'roll', { d: 1.7 }); cue(tC, 'lock'); cue(tC + 0.18, 'numbers');
  cue(tChk - 0.25, 'land'); cue(tChk + 0.05, 'window_in', { d: 0.5 });
  cue(W('records', 'replay:down'), 'press'); cue(tRecPress, 'release'); cue(tRecPress + 0.14, 'stamp');
  cue(W('records', 'scroll:start'), 'scroll', { d: W('records', 'scroll:end') - W('records', 'scroll:start') }); cue(W('records', 'rec:down'), 'click'); cue(W('records', 'rec:open'), 'open');
  cue(W('gates', 'tab:down'), 'click'); cue(W('gates', 'shown'), 'chips'); cue(W('gates', 'scroll:start'), 'scroll', { d: W('gates', 'scroll:end') - W('gates', 'scroll:start') }); cue(W('gates', 'state') - 0.1, 'state');
  cue(W('dxf', 'header:shown') - 0.07, 'click'); cue(W('dxf', 'dxf:down'), 'press'); cue(tDxfPress, 'release'); cue(tDxfOut, 'stream', { d: tPlot1 - tDxfOut }); cue(tPlot0, 'pen', { d: tPlot1 - tPlot0 - 0.5 }); cue(tPlot1 - 0.42, 'stamp_nr');
  cue(tEnd0 + 0.05, 'morph', { d: 0.9 }); cue(tEnd0 + 0.85, 'mark'); cue(tEnd0 + 1.0, 'word', { d: 0.7 }); cue(tEnd0 + 1.7, 'line_in'); cue(tEnd0 + 2.05, 'line_in');
  for (let i = 0; i < 3; i++) cue(L(8)[0] + 2.35 + i * 0.42, 'tick', { i }); cue(tBrand, 'brand'); cue(tBrand + 0.7, 'cta');
  CUES.sort((a, b) => a.t - b.t); window.NF_CUES = CUES;
  window.NF_INFO = { lang: LANG, footage: LANG, duration: DUR, frames: TM.frames, look: 'light', anchors: { tMatch, tPress, tRes, tCanvas0, tHero0, tTrace0, tTrace1, tF0, tR0, tC, tChk, tRec0, tRecPress, tGate0, tGate1, tDxfPress, tPlot0, tPlot1, tEnd0, tBrand }, slots: SL };

  // ---------------------------------------------------------------- drawing: light metal, one accent
  const mix = (a, b, u) => a.map((v, i) => v + (b[i] - v) * u);
  const rgb = (c, a = 1) => `rgba(${c[0] | 0},${c[1] | 0},${c[2] | 0},${a})`;
  const SH_LIGHT = [255, 255, 255], SH_MID = [222, 227, 235], SH_DARK = [150, 160, 176];
  const LIGHT = (() => { const v = [-0.45, 0.55, 0.70], n = Math.hypot(...v); return v.map((x) => x / n); })();
  const SHADE_RGB = (n) => { const d = Math.max(0, n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]); const u = Math.pow(d, 0.75); return u < 0.5 ? mix(SH_DARK, SH_MID, u * 2) : mix(SH_MID, SH_LIGHT, (u - 0.5) * 2); };
  const shadeOf = (n, a) => { const d = Math.max(0, n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]); const u = Math.pow(d, 0.75); return u < 0.5 ? rgb(mix(SH_DARK, SH_MID, u * 2), a) : rgb(mix(SH_MID, SH_LIGHT, (u - 0.5) * 2), a); };
  // a flat sheet is light metal too, in the joint's palette: light at the top, darker at the bottom
  const FLAT_N = (v) => { const a = lerp(0.8, 0.35, v); return [0, -Math.sin(a), Math.cos(a)]; };
  const flatMetal = (v) => SHADE_RGB(FLAT_N(v));
  const line = (pts, col, w, a = 1) => { if (pts.length < 2 || a <= 0.003) return; g.save(); g.globalAlpha = a; g.strokeStyle = col; g.lineWidth = w; g.lineJoin = 'round'; g.lineCap = 'round'; g.beginPath(); g.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) g.lineTo(pts[i][0], pts[i][1]); g.stroke(); g.restore(); };
  const text = (s, x, y, o = {}) => { g.save(); g.globalAlpha = o.alpha ?? 1; g.fillStyle = o.color || C.ink; g.font = o.font || fnt(30); g.textAlign = o.align || 'left'; g.textBaseline = o.base || 'alphabetic'; g.fillText(s, x, y); g.restore(); };
  const interp = (xs, ys, x) => { if (x <= xs[0]) return ys[0]; if (x >= xs[xs.length - 1]) return ys[ys.length - 1]; let lo = 0, hi = xs.length - 1; while (hi - lo > 1) { const m = (lo + hi) >> 1; if (xs[m] <= x) lo = m; else hi = m; } const u = (x - xs[lo]) / (xs[hi] - xs[lo] || 1); return lerp(ys[lo], ys[hi], u); };
  const sadOf = (sheet) => ({ xs: sheet.saddle.map((p) => p[0]), ys: sheet.saddle.map((p) => p[1]) });
  const SADJ = sadOf(sh0B), SADW = sadOf(sh0W), SADF = sadOf(shB);
  // a solid surface sampled on a grid: quads on the front, shaded by the normal; morph m blends flat pattern -> rolled sheet
  function sheetQuads(cam, nu, nv, Pflat, Pround, Nround, lay, m, k, flatH = 1) {
    const out = []; const P = (u, v) => { const pf = Pflat(u, v); const flat = lay ? [lay.ox + lay.s * pf[0], lay.oy - lay.s * pf[1]] : null;
      if (m <= 0) return { s: flat, n: FLAT_N(clamp(pf[1] / flatH, 0, 1)), f: true, flat: true };
      const r = cam(Pround(u, v, k)), n = Nround(u, v, k); const sc = [lerp(flat ? flat[0] : r[0], r[0], m), lerp(flat ? flat[1] : r[1], r[1], m)];
      const nn = [lerp(0, n[0], m), lerp(0, n[1], m), lerp(1, n[2], m)];
      return { s: sc, n: nn, f: m < 0.5 || cam.front(n), m }; };
    for (let i = 0; i < nu; i++) for (let j = 0; j < nv; j++) {
      const u0 = i / nu, u1 = (i + 1) / nu, v0 = j / nv, v1 = (j + 1) / nv, um = (u0 + u1) / 2, vm = (v0 + v1) / 2;
      const c = P(um, vm); if (!c.f) continue;
      const q = [P(u0, v0).s, P(u1, v0).s, P(u1, v1).s, P(u0, v1).s];
      out.push({ q, fill: c.flat ? rgb(SHADE_RGB(c.n)) : rgb(mix([206, 213, 224], SHADE_RGB(c.n), clamp(c.m * 1.3))) });
    }
    return out;
  }
  const drawQuads = (quads, a) => { g.save(); g.globalAlpha = a; for (const z of quads) { g.beginPath(); g.moveTo(z.q[0][0], z.q[0][1]); for (let i = 1; i < 4; i++) g.lineTo(z.q[i][0], z.q[i][1]); g.closePath(); g.fillStyle = z.fill; g.fill(); g.strokeStyle = z.fill; g.lineWidth = 1; g.stroke(); } g.restore(); };
  // the branch's parametrisation: u around the branch (x = u * circumference), v from the saddle (bottom) to the square end (top)
  const branchP = (sheet, SA, off, m0) => (u, v, k) => { const x = u * sheet.C; const sb = interp(SA.xs, SA.ys, x); const y = sb + (sheet.top - sb) * v; const p = (k === undefined ? J0 : J0).branch(x, y, k === undefined ? 1 : k); return [p[0] + off[0], p[1] + off[1], p[2] + off[2]]; };
  const branchN = (sheet) => (u, v, k) => J0.branchN(u * sheet.C, k === undefined ? 1 : k);
  const headerP = (sheet, off) => (u, v, k) => { const p = sh0H.pos(u * J0.L, v * sheet.C, k === undefined ? 1 : k); return [p[0] + off[0], p[1] + off[1], p[2] + off[2]]; };
  const headerN = (sheet) => (u, v, k) => J0.headerN(v * sheet.C, k === undefined ? 1 : k);
  // the contact shadow: a soft, low, wide shadow on the wallpaper under the joint
  function contactShadow(xs0, xs1, yb, a) { if (a <= 0.01) return; g.save(); g.globalAlpha = a; g.fillStyle = '#000'; g.shadowColor = 'rgba(20,24,34,0.30)'; g.shadowBlur = 70; g.shadowOffsetY = 0; g.beginPath(); g.ellipse(-4000 + (xs0 + xs1) / 2, yb, (xs1 - xs0) * 0.42, 26, 0, 0, 7); g.shadowOffsetX = 4000; g.fill(); g.restore(); }
  function sheetShadow(poly, a, dy = 16, blur = 40) { if (a <= 0.01 || poly.length < 3) return; g.save(); g.globalAlpha = a; g.fillStyle = '#000'; g.shadowColor = 'rgba(20,24,34,0.26)'; g.shadowBlur = blur; g.shadowOffsetX = 4000; g.shadowOffsetY = dy; g.beginPath(); poly.forEach((p, i) => (i ? g.lineTo(p[0] - 4000, p[1]) : g.moveTo(p[0] - 4000, p[1]))); g.closePath(); g.fill(); g.restore(); }
  // the joint's view: fitted to 70 % of the frame width at the start (a fit, not a typed scale)
  const fitJoint = (base, targetW, targetH, cx0, cy0, pts) => { const probe = G.camera({ ...base, scale: 1, cx: 0, cy: 0 }); let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
    for (const p of pts) { const q = probe(p); x0 = Math.min(x0, q[0]); x1 = Math.max(x1, q[0]); y0 = Math.min(y0, q[1]); y1 = Math.max(y1, q[1]); }
    const s = Math.min(targetW / (x1 - x0), targetH / (y1 - y0)); return { scale: s, cx: cx0 - s * (x0 + x1) / 2, cy: cy0 - s * (y0 + y1) / 2 }; };
  const jointPts = []; for (const x of [0, J0.L]) for (let k = 0; k <= 40; k++) jointPts.push(sh0H.pos(x, sh0H.C * k / 40, 1));
  for (let k = 0; k <= 40; k++) jointPts.push(sh0B.pos(sh0B.C * k / 40, sh0B.top, 1));
  const JH = fitJoint({ yaw: -0.65, pitch: 0.45, dist: 3600, target: [520, 0, 90] }, 1344, 700, 960, 650, jointPts);

  // ================================================================= shot 1: the joint, solid, in light metal, the cut in the accent
  function drawHook(t) {
    if (t > tMatch + 0.30) return;
    const turn = E.io3(seg(t, L(1)[1] + 0.22, tMatch - 0.02)), drift = seg(t, 0, L(1)[1] + 0.2);
    const cam = G.camera({ yaw: lerp(lerp(-0.74, -0.56, E.ioSine(drift)), 0, turn), pitch: lerp(lerp(0.50, 0.40, drift), 0, turn), dist: lerp(3600, 60000, E.i3(turn)),
      scale: lerp(JH.scale, schScale, turn), cx: lerp(JH.cx, schO[0], turn), cy: lerp(JH.cy, schO[1], turn), target: [lerp(520, PRE.branch_station, turn), 0, lerp(90, 0, turn)] });
    const out = 1 - seg(t, tMatch + 0.02, tMatch + 0.28), flat = E.io2(seg(t, tMatch - 0.5, tMatch));
    const aH = E.o3(seg(t, 0.55, 1.5)) * out;
    // the line of light: the header's top generator, drawn in the accent
    const u = E.io3(seg(t, 0.12, 1.15)); if (u > 0 && t < 2.2) { const a = cam([0, 0, J0.R]), b = cam([J0.L * u, 0, J0.R]); line([a, b], C.acc, 3.5, (1 - seg(t, 1.3, 2.1)) * out); }
    // the branch arrives along its own axis, seats; later it sits wrong, then right again
    const drop = 320 * (1 - spring(t - 1.32, 9.5, 1)), wrong = spring(t - 4.02, 11, 1) * (1 - spring(t - 5.42, 16, 1)), lift = drop + 36 * wrong;
    const aB = E.o3(seg(t, 1.25, 1.75)) * out, off = [lift * J0.cb, 0, lift * J0.sb];
    const ign = E.o3(seg(t, 2.5, 3.3));
    if (flat < 0.999) {
      const footY = Math.max(...[sh0H.pos(0, 0, 1), sh0H.pos(J0.L, 0, 1)].map((p) => cam(p)[1]));
      if (aH > 0.02) contactShadow(cam(sh0H.pos(0, 0, 1))[0], cam(sh0H.pos(J0.L, 0, 1))[0], footY + 10, aH);
      if (aH > 0.01) drawQuads(sheetQuads(cam, 40, 120, (u2, v2) => [u2 * J0.L, v2 * sh0H.C], headerP(sh0H, [0, 0, 0]), headerN(sh0H), null, 1, 1), aH);
      if (aB * (1 - wrong) > 0.01) drawQuads(sheetQuads(cam, 110, 26, (u2, v2) => [u2 * sh0B.C, v2], branchP(sh0B, SADJ, off), branchN(sh0B), null, 1, 1), aB * (1 - wrong));
      if (aB * wrong > 0.01) drawQuads(sheetQuads(cam, 110, 26, (u2, v2) => [u2 * sh0W.C, v2], branchP(sh0W, SADW, off), branchN(sh0W), null, 1, 1), aB * wrong * 0.7);
      // rims at the ends of the header and the square end of the branch: 2.5 px, the steel's edge
      const rimH = (uu) => Array.from({ length: 121 }, (_, i) => cam(sh0H.pos(uu * J0.L, sh0H.C * i / 120, 1)));
      if (aH > 0.02) { line(rimH(0), C.muted, 2.5, aH * 0.85); line(rimH(1), C.muted, 2.5, aH * 0.85); }
      if (aB > 0.02) { const top = Array.from({ length: 121 }, (_, i) => { const p = branchP(sh0B, SADJ, off)(i / 120, 1, 1); return cam(p); }); line(top, C.muted, 2.5, aB * (1 - wrong) * 0.85); }
      // the one curve: where the two surfaces meet, in the accent, thick
      if (ign > 0) { const pts = [], nrm = []; for (let i = 0; i <= 200; i++) { const x = sh0B.C * i / 200; const y = interp(SADJ.xs, SADJ.ys, x); const p = J0.branch(x, y + 0.0, 1); pts.push(cam([p[0] + off[0], p[1] + off[1], p[2] + off[2]])); nrm.push(J0.branchN(x, 1)); }
        let run = []; for (let i = 0; i < pts.length; i++) { const f = cam.front(nrm[i]); if (f) run.push(pts[i]); else { line(run, C.acc, 6, aB * (1 - wrong) * ign); run = []; } } line(run, C.acc, 6, aB * (1 - wrong) * ign); }
      if (wrong > 0.001) { const pts = []; for (let i = 0; i <= 200; i++) { const x = sh0W.C * i / 200; const y = interp(SADW.xs, SADW.ys, x); const p = J0.branch(x, y, 1); pts.push(cam([p[0] + off[0], p[1] + off[1], p[2] + off[2]])); } line(pts, C.muted, 4, aB * wrong * 0.8); }
    }
    // side elevation: the branch's outline as the schematic draws it (from the axis to the square end)
    if (flat > 0) { const r = J0.r, P = (s, k) => cam([J0.st + s * J0.cb - k * r * J0.sb, 0, s * J0.sb + k * r * J0.cb]); line([P(0, 1), P(J0.send, 1), P(J0.send, -1), P(0, -1), P(0, 1)], C.muted, 2.5, flat * out); }
  }
  let schScale = 0.8, schO = [960, 540];
  { const sk = (D.takes.form || {}).sketch0; if (sk) { const nums = (d) => d.match(/-?\d+(\.\d+)?/g).map(Number); const a = nums(sk.paths[0][1]), b = nums(sk.paths[1][1]); const s = (a[2] - a[0]) / PRE.header_length, yAxis = (a[1] + b[1]) / 2; const c = sk.ctm;
    const sch = (x, z) => flatProject(cMatch, c[0] * (a[0] + x * s) + c[4], c[3] * (yAxis - z * s) + c[5]); schScale = (sch(1000, 0)[0] - sch(0, 0)[0]) / 1000; schO = sch(PRE.branch_station, 0); } }

  // ================================================================= shot 6: the equation and the curve (4 px, accent, 80 % of the width)
  const layHeroAt = (t, c) => heroLay(t, c);
  function drawHero(t, c) {
    if (t < tHero0 + 0.18 || t > tR0 + 0.05) return;
    const lay = layHeroAt(t, c), P = (p) => [lay.ox + lay.s * p[0], lay.oy - lay.s * p[1]];
    const a = E.io2(seg(t, tHero0 + 0.22, tHero0 + 0.62)), toFold = seg(t, tF0, tF0 + 0.7), hero = spring(t - (tHero0 + 0.62), 4.6, 1) * (1 - toFold);
    const u = clamp((t - tTrace0) / (tTrace1 - tTrace0)), vi = clamp(Math.round(u * (NS - 1)), 0, NS - 1), pt = SAD[vi], phi = pt[0] / J.r;
    const outl = 1 - hero * 0.86;
    line([[0, SAD[0][1]], [0, shB.top], [shB.C, shB.top], [shB.C, SAD[NS - 1][1]]].map(P), C.faint, 2, a * outl);
    const base = hero * (1 - toFold);
    if (base > 0.01) { line([P([0, 0]), P([shB.C, 0])], C.muted, 2, base * 0.9);
      for (let q = 0; q <= 4; q++) { const x = (shB.C * q) / 4, p = P([x, 0]); line([[p[0], p[1] - 9], [p[0], p[1] + 9]], C.muted, 2, base); text(`${q * 90}°`, p[0], p[1] + 46, { font: fnt(30), align: 'center', color: C.muted, alpha: base }); }
      text('φ', P([shB.C, 0])[0] + 36, P([0, 0])[1] + 14, { font: fnt(30, WT.reg, true), color: C.muted, alpha: base, align: 'center' }); }
    line(SAD.map(P), C.faint, 2, a * lerp(1, 0.6, hero));
    if (hero > 0.01) line(SAD.slice(0, vi + 1).map(P), C.acc, 4, hero);
    const p = P(pt), b0 = P([pt[0], 0]);
    if (base > 0.01) { line([b0, p], C.ink, 1.6, base * 0.6); g.save(); g.globalAlpha = base; g.fillStyle = C.ink; g.beginPath(); g.arc(b0[0], b0[1], 4, 0, 7); g.fill(); g.restore(); }
    if (hero > 0.02) { g.save(); g.globalAlpha = hero; g.fillStyle = C.acc; g.beginPath(); g.arc(p[0], p[1], 8, 0, 7); g.fill(); g.restore(); }
    return { p, phi, s: pt[1], vi, base, hero };
  }

  // ================================================================= shot 7: the two plates, flat (filled sheets), then rolled into shells
  let CAM_NOW = null;
  function drawFold(t) {
    if (t < tF0 - 0.02 || t > tC + 0.9) return null;
    const out = 1 - E.io2(seg(t, tC + 0.1, tC + 0.6));
    const m = E.io4(seg(t, tR0, tR0 + 1.15)), k = E.io3(seg(t, tR0 + 0.55, tC)), orbit = seg(t, tR0, tC);
    const cam = G.camera({ yaw: lerp(-0.70, -0.52, E.ioSine(orbit)), pitch: lerp(0.50, 0.42, orbit), dist: 5200, scale: lerp(0.92, 1.02, E.o3(seg(t, tR0, tC + 0.6))), cx: 960, cy: 600, target: [J.L / 2, 0, 110] });
    const inH = spring(t - tF0, 8, 1), layH = { ...layH2, ox: layH2.ox - 260 * (1 - inH) }, layB = heroLay(t, CAM_NOW || CAM(t));
    const lock = clamp((t - tC) * 6);
    const aH = clamp(inH * 1.4) * out, aB = out;
    const flatPoly = (lay, pts) => pts.map((p) => [lay.ox + lay.s * p[0], lay.oy - lay.s * p[1]]);
    sheetShadow(flatPoly(layH, [[0, 0], [J.L, 0], [J.L, shH.C], [0, shH.C]]), aH * (1 - m), 16, 40);
    sheetShadow(flatPoly(layB, [...SAD, [shB.C, shB.top], [0, shB.top]]), aB * (1 - m), 16, 40);
    // header: a filled sheet in light metal; its cut in the accent
    drawQuads(sheetQuads(cam, 30, 60, (u2, v2) => [u2 * J.L, v2 * shH.C], (u2, v2, kk) => { const p = shH.pos(u2 * J.L, v2 * shH.C, kk); return p; }, (u2, v2, kk) => J.headerN(v2 * shH.C, kk), layH, m, k, shH.C), aH);
    if (shH.hole.length) { const hole = shH.hole.map((p) => { const pf = [layH.ox + layH.s * p[0], layH.oy - layH.s * p[1]]; if (m <= 0) return pf; const pr = cam(shH.pos(p[0], p[1], k)); return [lerp(pf[0], pr[0], m), lerp(pf[1], pr[1], m)]; }); line([...hole, hole[0]], C.acc, 4.5 + 0.5 * lock, aH); }
    // branch: the same, with its saddle cut in the accent
    drawQuads(sheetQuads(cam, 90, 24, (u2, v2) => { const x = u2 * shB.C, sb = interp(SADF.xs, SADF.ys, x); return [x, sb + (shB.top - sb) * v2]; }, (u2, v2, kk) => { const x = u2 * shB.C, sb = interp(SADF.xs, SADF.ys, x); const y = sb + (shB.top - sb) * v2; return J.branch(x, y, kk); }, (u2, v2, kk) => J.branchN(u2 * shB.C, kk), layB, m, k, shB.top), aB);
    const sadPts = SAD.map((p) => { const pf = [layB.ox + layB.s * p[0], layB.oy - layB.s * p[1]]; if (m <= 0) return pf; const pr = cam(J.branch(p[0], p[1], k)); return [lerp(pf[0], pr[0], m), lerp(pf[1], pr[1], m)]; });
    line(sadPts, C.acc, 4.5 + 0.5 * lock, aB);
    const edgeH = [[0, 0], [J.L, 0], [J.L, shH.C], [0, shH.C], [0, 0]].map((p) => { const pf = [layH.ox + layH.s * p[0], layH.oy - layH.s * p[1]]; if (m <= 0) return pf; const pr = cam(shH.pos(p[0], p[1], k)); return [lerp(pf[0], pr[0], m), lerp(pf[1], pr[1], m)]; });
    line(edgeH, C.muted, 2.5, aH * 0.9);
    const edgeB = [...SAD, [shB.C, shB.top], [0, shB.top], SAD[0]].map((p) => { const pf = [layB.ox + layB.s * p[0], layB.oy - layB.s * p[1]]; if (m <= 0) return pf; const pr = cam(J.branch(p[0], p[1], k)); return [lerp(pf[0], pr[0], m), lerp(pf[1], pr[1], m)]; });
    line(edgeB, C.muted, 2.5, aB * 0.9);
    // plate dimensions while flat: the product's own quantities, above nothing, below the plates
    const dA = clamp((1 - m) * 1.6) * out; if (dA > 0.01) { const q = PH.quantities || {};
      text(`${TXT.part_header}  ${fmt3(q.length ?? J.L)} × ${fmt3(q.developed_width ?? shH.C)} ${TXT.unit_mm}`, layH.ox + (layH.s * J.L) / 2, layH.oy + 36, { font: fnt(30), align: 'center', color: C.muted, alpha: dA * clamp(inH) });
      text(`${TXT.part_branch}  ${fmt3(shB.C)} × ${fmt3(shB.top)} ${TXT.unit_mm}`, layB2.ox + (layB2.s * shB.C) / 2, layB2.oy + 36, { font: fnt(30), align: 'center', color: C.muted, alpha: dA * seg(t, tF0 + 0.5, tF0 + 0.9) }); }
    return { lock, out };
  }

  // ================================================================= shot 10: the DXF — the pattern on the left, the file's text in the right third, read back in the accent
  const dxfPath = (() => { if (!DXF) return []; const segs = []; for (const l of DXF.lines) segs.push({ layer: l.layer, pts: [[l.x1, l.y1], [l.x2, l.y2]], row: l.row }); for (const p of DXF.polylines) segs.push({ layer: p.layer, pts: [...p.pts, p.pts[0]], row: p.row }); let tot = 0; for (const s of segs) { s.len = 0; for (let i = 1; i < s.pts.length; i++) s.len += Math.hypot(s.pts[i][0] - s.pts[i - 1][0], s.pts[i][1] - s.pts[i - 1][1]); s.a = tot; tot += s.len; } segs.total = tot; return segs; })();
  const layPlate = (c) => { const lc = layAppAt('dxf', fDxf, c); return mixLay(lc, layDXF, E.io2(seg(c._t ?? 0, tPlot0 - 0.3, tPlot0 + 0.25))); };
  function plateOutline(lay) { const P = (p) => [lay.ox + lay.s * p[0], lay.oy - lay.s * p[1]]; return { P, rect: [[0, 0], [J.L, 0], [J.L, shH.C], [0, shH.C], [0, 0]].map(P), hole: [...shH.hole, shH.hole[0]].map(P) }; }
  function drawDXF(t, c) {
    if (!DXF || t < tDxfOut - 0.05 || t > tEnd0 + 1.2) return;
    const lay = mixLay(layAppAt('dxf', fDxf, c), layDXF, E.io2(seg(t, tPlot0 - 0.3, tPlot0 + 0.25))), o = plateOutline(lay), toMark = E.io3(seg(t, tEnd0, tEnd0 + 0.95));
    const aPlot = E.io2(seg(t, tPlot0 - 0.2, tPlot0 + 0.2));
    if (toMark < 1) {
      const pr = o.rect, fa = aPlot * (1 - toMark); sheetShadow(pr, fa, 18, 46);
      g.save(); g.globalAlpha = fa; g.beginPath(); g.moveTo(pr[0][0], pr[0][1]); for (const p of pr) g.lineTo(p[0], p[1]); g.closePath();
      const lg = g.createLinearGradient(0, o.P([0, shH.C])[1], 0, o.P([0, 0])[1]); lg.addColorStop(0, rgb(flatMetal(1))); lg.addColorStop(1, rgb(flatMetal(0))); g.fillStyle = lg; g.fill(); g.restore();
    }
    if (toMark <= 0) {
      line(o.rect, C.muted, 2.5, aPlot); line(o.hole, C.acc, 5, aPlot);
      // the file itself, as text, in the right third (columns 9 to 11), 26 px; the read-back pen runs beside it
      const a = E.o3(seg(t, tDxfOut, tDxfOut + 0.35)), x0 = 1180, lh = 34, first = DXF.lines[0] ? DXF.lines[0].row - 8 : 40;
      const u = E.io2(seg(t, tPlot0, tPlot1 - 0.5)), d = u * dxfPath.total; let rowNow = first;
      for (const s of dxfPath) if (d >= s.a) rowNow = s.row + (s.pts.length > 2 ? Math.round(((d - s.a) / s.len) * (s.pts.length - 1)) * 6 : 0);
      const run = (t - tDxfOut), scroll = lerp(first, rowNow, seg(t, tPlot0 - 0.3, tPlot0 + 0.2)) + (t < tPlot0 ? run * 34 - 20 : 0);
      g.save(); g.beginPath(); g.rect(x0 - 8, 430, 1800 - x0 + 8, 540); g.clip();
      for (let i = -12; i <= 12; i++) { const r = Math.round(scroll) + i; if (r < 0 || r >= dxfRows.length) continue; const y = 690 + (i - (scroll - Math.round(scroll))) * lh; const fade = clamp(1 - Math.abs(y - 690) / 340); const isCode = r % 2 === 0, hot = Math.abs(r - rowNow) <= 3 && t > tPlot0 - 0.1;
        text(dxfRows[r].trim(), x0 + (isCode ? 0 : 96), y, { font: fnt(26), color: hot ? C.acc : isCode ? C.muted : C.ink, alpha: a * fade * (hot ? 1 : 0.9) }); }
      g.restore();
      // read back: the film draws from the file's own coordinates, in the accent
      let pen = null;
      for (const s of dxfPath) { if (d <= s.a) break; const f = clamp((d - s.a) / s.len), n = s.pts.length; const cnt = Math.max(2, Math.ceil(f * (n - 1)) + 1), pts = s.pts.slice(0, cnt).map(o.P);
        if (f < 1 && n > 1) { const i = Math.min(n - 2, Math.floor(f * (n - 1))), ff = f * (n - 1) - i; pts[pts.length - 1] = o.P([lerp(s.pts[i][0], s.pts[i + 1][0], ff), lerp(s.pts[i][1], s.pts[i + 1][1], ff)]); pen = pts[pts.length - 1]; }
        line(pts, C.acc, 4, 1); }
      if (pen && u < 1) { g.save(); g.fillStyle = C.acc; g.beginPath(); g.arc(pen[0], pen[1], 7, 0, 7); g.fill(); g.restore(); }
      // the file's own annotations (layer NF_NOT_RELEASED), at their coordinates: the product's NOT RELEASED in amber
      const an = E.o3(seg(t, tPlot1 - 0.5, tPlot1 - 0.15));
      DXF.texts.slice(0, 2).forEach((tx, i) => { const p = o.P([tx.x1, tx.y1]); text(tx.text, p[0], p[1], { font: fnt(26, WT.st), color: C.amber, alpha: an * (i ? 0.8 : 1) }); });
    }
    return { lay, o, toMark };
  }

  // ================================================================= the mark: the plate's outline becomes the product's mark
  function drawMark(t, dx) {
    if (t < tEnd0) return;
    const u = E.io3(seg(t, tEnd0, tEnd0 + 0.95)), N = 24, S = mkS, X = lockMarkX, Y = lockY - 8 * S, M = (p) => [X + p[0] * S * 0.86, Y + p[1] * S];
    const edge = (a, b) => Array.from({ length: N + 1 }, (_, i) => [lerp(a[0], b[0], i / N), lerp(a[1], b[1], i / N)]);
    if (dx && u < 1) { const r = dx.o.rect, legs = MARK.legs.map(M);
      const from = [edge(r[0], r[3]), edge(r[3], r[2]), edge(r[2], r[1])], to = [edge(legs[0], legs[1]), edge(legs[1], legs[2]), edge(legs[2], legs[3])];
      const pts = []; for (let e = 0; e < 3; e++) for (let i = 0; i <= N; i++) pts.push([lerp(from[e][i][0], to[e][i][0], u), lerp(from[e][i][1], to[e][i][1], u)]);
      line(pts, u > 0.5 ? C.acc : C.ink, lerp(3, 1.55 * S, u), 1);
      const h = dx.o.hole, n = h.length - 1, arc = MARK.arc.map(M); let iL = 0, iR = 0; for (let i = 0; i < n; i++) { if (h[i][0] < h[iL][0]) iL = i; if (h[i][0] > h[iR][0]) iR = i; }
      const halves = [[], []]; for (let i = 0; i <= 40; i++) { const stepA = ((iR - iL + n) % n) * (i / 40), stepB = ((iL - iR + n) % n) * (i / 40); halves[0].push(h[Math.round(iL + stepA) % n]); halves[1].push(h[Math.round(iR + stepB) % n]); }
      halves[1].reverse();
      for (const hf of halves) line(hf.map((p, i) => [lerp(p[0], arc[i][0], u), lerp(p[1], arc[i][1], u)]), u > 0.5 ? C.acc : C.ink, lerp(3, 1.55 * S, u), 1);
    } else { const pop = 1 + 0.03 * Math.exp(-(t - tEnd0 - 0.95) * 7) * Math.cos((t - tEnd0 - 0.95) * 20);
      g.save(); g.translate(X + 8 * S * 0.86, Y + 8 * S); g.scale(pop, pop); g.translate(-(X + 8 * S * 0.86), -(Y + 8 * S));
      line(MARK.legs.map(M), C.acc, 1.55 * S, 1); line(MARK.arc.map(M), C.acc, 1.55 * S, 1); g.restore(); }
  }

  // ---------------------------------------------------------------- one frame
  let lastFoot = -1;
  window.seek = (t) => {
    t = clamp(t, 0, DUR - 1e-6);
    drawWallpaper(t);
    // ---- the product window: one large window on the wallpaper; the camera always covers it
    const at = R.at(t), c = clampCam(CAM(t)); const life = winA(t); CAM_NOW = c;
    if (life > 0.002) {
      foot.draw(at.take, at.f);
      style(win, { opacity: life.toFixed(4), visibility: 'visible' });
      const rc = { ...c, sx: c.sx - WIN.L, sy: c.sy - WIN.T };
      style(rig, { transform: rigTransform(rc) });
      const p = foot.pointer(at.take, at.f), kind = window.NFE.cursorKind(p.cursor), hot = window.NFE.cursorHot[kind], cs = 1.25 * (p.down ? 0.88 : 1);
      if (kind !== curKind) { curEl.innerHTML = window.NFE.cursorSvg(kind); curKind = kind; }
      style(curEl, { transform: `translate(${(p.x - hot[0] * cs).toFixed(2)}px,${(p.y - hot[1] * cs).toFixed(2)}px) scale(${cs})`, opacity: t > tMatch + 0.3 ? '1' : '0' });
    } else style(win, { visibility: 'hidden' });

    // ---- the vector layer: the joint, the curve, the plates, the DXF, the mark
    g.setTransform(DPR, 0, 0, DPR, 0, 0); g.clearRect(0, 0, 1920, 1080);
    drawHook(t);
    const hero = drawHero(t, c); const fold = drawFold(t); const dx = drawDXF(t, c); drawMark(t, dx);

    // ---- the statement line of the scene
    let shownS = false;
    for (const s of STM) { const q = stmState(s, t); if (!q) continue; shownS = true; setTxt(stmt, s.text); const px = fitSize(s.text, 1500, 96, s.max || 132, WT.st); style(stmt, { fontSize: px + 'px', opacity: q.op.toFixed(3), transform: `translate(0px,${(s.y + q.y).toFixed(2)}px)` }); }
    if (!shownS) style(stmt, { opacity: '0' });

    // ---- the secondary line under the window (or on the wallpaper in the film's own scenes): text and small print
    let capTxt = '', capLbl = '', capQ = null;
    for (const [a, b, s, lbl] of CAPS) { const q = inOut(t, a, b); if (!q) continue; if (s || !capQ) { capQ = q; capLbl = lbl; if (s) capTxt = s; } else if (!capTxt) capLbl = lbl; }
    if (capQ) { setTxt(sec, `${capTxt ? `<span>${capTxt}</span>` : ''}<span class="lab"${capTxt ? '' : ' style="margin-left:0"'}>${capLbl}</span>`); style(sec, { opacity: (capQ.in * capQ.out).toFixed(3), transform: `translate(0px,${((1 - capQ.in) * 12).toFixed(2)}px)` }); }
    else style(sec, { opacity: '0' });

    // ---- the equation and its live values
    const eqA = track([[tHero0 + 0.85, 0], [tHero0 + 1.0, 1, 'lin'], [tF0 - 0.05, 1], [tF0 + 0.3, 0, 'io2']])(t);
    style(eqEl, { opacity: (t > tF0 - 0.05 ? eqA : eqA).toFixed(3) });
    if (eqA > 0.002) {
      eqT.forEach((el) => { const k = +el.dataset.k, u = E.o4(seg(t, tHero0 + 0.9 + k * 0.17, tHero0 + 1.5 + k * 0.17)); if (el.classList.contains('bar')) style(el, { transform: `scaleX(${u.toFixed(4)})`, opacity: '1' }); else style(el, { opacity: u.toFixed(3), transform: `translateY(${((1 - u) * 22).toFixed(2)}px)` }); });
      const u = E.o3(seg(t, tHero0 + 1.75, tHero0 + 2.25)), live = hero ? `<span class="live"><i>φ</i> = <b>${((hero.phi * 180) / Math.PI).toFixed(1).padStart(5, ' ')}°</b>&emsp;<i>s</i> = <b>${fmt3(hero.s)}</b> ${TXT.unit_mm}</span>` : '';
      setTxt(eqv, `<i>R</i> = <b>${REQ.header_radius}</b> ${TXT.unit_mm}&emsp;<i>r</i> = <b>${REQ.branch_radius}</b> ${TXT.unit_mm}&emsp;<i>β</i> = <b>${REQ.beta_deg}°</b>&emsp;&emsp;${live}`);
      style(eqv, { opacity: (u * eqA).toFixed(3) });
    } else style(eqv, { opacity: '0' });

    // ---- the DXF file name over the text column
    const dA = E.o3(seg(t, tDxfOut + 0.25, tDxfOut + 0.7)) * (1 - seg(t, tEnd0 - 0.25, tEnd0));
    style(dxfname, { opacity: dA.toFixed(3) });
    if (dA > 0.002 && D.dxf && D.dxf.header) { const n = D.dxf.header.name, m = /^([A-Z-]+)(.*)$/.exec(n) || [0, '', n]; setTxt(dxfname, `<b>${m[1]}</b>${m[2].replace(/_/g, '_<wbr>')}<br>${D.dxf.header.bytes.toLocaleString('en-US')} ${TXT.bytes} · ${Object.keys(DXF.layers).join(' · ')}`); }

    // ---- the end card: the lockup, the positioning lines, the brand line, the call to action, the disclaimer
    const isEnd = t > tEnd0 + 0.2;
    style(endEl, { opacity: isEnd ? '1' : '0' });
    if (isEnd) {
      const sw = E.io2(seg(t, tBrand - 0.32, tBrand - 0.02));
      const p1 = E.o4(seg(t, tEnd0 + 1.65, tEnd0 + 2.3)), p2 = E.o4(seg(t, tEnd0 + 2.0, tEnd0 + 2.65));
      style($('#pos'), { opacity: (p1 * (1 - sw)).toFixed(3), transform: `translateY(${((1 - p1) * 16 - sw * 10).toFixed(2)}px)` });
      style($('#plan'), { opacity: (p1 * (1 - sw)).toFixed(3), transform: `translateY(${((1 - p2) * 16 - sw * 10).toFixed(2)}px)` });
      const td = $('#today'), t0 = L(8)[0] + 2.2; style(td, { opacity: (E.o3(seg(t, t0 - 0.25, t0 + 0.2)) * (1 - sw)).toFixed(3) });
      [...td.querySelectorAll('.it')].forEach((el, i) => { const uu = spring(t - (t0 + 0.15 + i * 0.42), 14, 1); style(el, { opacity: clamp(uu * 1.5).toFixed(3) }); style(el.firstChild, { transform: `translateY(4px) scale(${(0.4 + 0.6 * uu).toFixed(3)})` }); });
      const b1 = E.io3(seg(t, tBrand, tBrand + 0.35)), b2 = spring(t - (tBrand + 0.62), 9, 1);
      style($('#brand'), { opacity: b1.toFixed(3), transform: `translateY(${((1 - b1) * 24).toFixed(2)}px)` });
      style($('#cta'), { opacity: clamp(b2 * 1.6).toFixed(3), transform: `translateY(${((1 - b2) * 22).toFixed(2)}px)` });
      style($('#disc'), { opacity: E.o3(seg(t, tEnd0 + 0.25, tEnd0 + 0.8)).toFixed(3) }); style($('#credit'), { opacity: E.o3(seg(t, tEnd0 + 1.2, tEnd0 + 1.8)).toFixed(3) });
      style(wm, { opacity: E.o3(seg(t, tEnd0 + 0.95, tEnd0 + 1.6)).toFixed(3) });
    }
    return true;
  };
  window.seek(0); window.READY = true;
})().catch((e) => { console.error('film failed: ' + (e && e.stack || e)); window.READY = 'error'; });
