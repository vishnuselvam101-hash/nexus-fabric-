/* NEXUS FABRIC — ad4k shots 6–10 (14.000–30.000 s). Deterministic: every property from local time.
   Every camera target and every highlight frame is derived from the MEASURED capture rectangles
   (ad4k/rects.json, re-taken captures). No estimated coordinates anywhere in this file. */
(function () {
  const S = window.SHOTS, NF = window.NF, L = NF.LANG, ja = L === 'ja';
  const clamp = NF.clamp, lerp = NF.lerp, seg = NF.seg, E = NF.ease, el = NF.el, svg = NF.svg, T = NF.text;
  const CY = '#35D6E8', AM = '#E8A33D', RULE = '#2A333C', INK = '#F2F6F8', INK2 = '#9AA7B2';
  const pick = (en, jp) => (ja ? jp : en);

  /* ---------- measured rectangles [left, top, width, height] in 1920x1080 capture space ---------- */
  const RECTS = {
    '05-job-gates': { review_banner: [496, 274, 928, 93], gate_chips: [513, 501, 894, 24], first_gate: [514, 535, 892, 92], tabs: [496, 383, 928, 38] },
    '15-ja-job-gates': { review_banner: [496, 290, 928, 100], gate_chips: [513, 528, 894, 25], first_gate: [514, 562, 892, 78], tabs: [496, 407, 928, 38] },
    '02-convert': { dropzone: [496, 88, 928, 300], scale_kind: [700, 425, 254, 40], scale_value: [966, 425, 254, 40] },
    '14-ja-convert': { dropzone: [496, 88, 928, 300], scale_kind: [700, 427, 254, 40], scale_value: [966, 427, 254, 40] },
    '03-pdf-result': { pdf_pill: [535, 137, 465, 49], dxf_button: [535, 253, 219, 38] },
    '22-ja-pdf-result': { pdf_pill: [535, 137, 408, 57], dxf_button: [535, 242, 253, 38] },
    '17-dark-flat-pattern': { review_banner: [496, 274, 928, 93], pattern_svg: [515, 619, 890, 592], tabs: [496, 383, 928, 38] },
    '16-ja-flat-pattern': { review_banner: [496, 251, 928, 100], pattern_svg: [515, 423, 890, 592], tabs: [496, 368, 928, 38] }
  };
  const PAD = 8;
  const rect = (screen, key) => RECTS[screen][key];
  const cxOf = (r) => r[0] + r[2] / 2, cyOf = (r) => r[1] + r[3] / 2;
  const join = (a, b) => {
    const x = Math.min(a[0], b[0]), y = Math.min(a[1], b[1]);
    return [x, y, Math.max(a[0] + a[2], b[0] + b[2]) - x, Math.max(a[1] + a[3], b[1] + b[3]) - y];
  };
  // zoom ceiling from the measured rect: the padded element must stay inside 1700 x 980 of the 1920x1080 frame
  const cap = (r) => Math.min(1700 / (r[2] + 2 * PAD), 980 / (r[3] + 2 * PAD));
  const zoom = (r, want) => Math.min(want, cap(r));
  // closed measurement rectangle, exactly 8 px outside the measured element at rest.
  // NF.frame settles its own padding 26 -> 10 px, so the rect is inset by 2 px to land on 8.
  const box = (layer, r, p, key) => NF.frame(layer, r[0] + 2, r[1] + 2, r[2] - 4, r[3] - 4, p, key || 0);

  // caption window: 0.12 s cross-fades, optional +30 px settle
  function win(n, t, a, b, rise) {
    const o = Math.min(seg(t, a, a + .12), 1 - seg(t, b - .12, b));
    n.style.opacity = o.toFixed(3);
    n.style.transform = rise ? `translateY(${(30 * (1 - E.outCubic(seg(t, a, a + .34)))).toFixed(2)}px)` : 'none';
    return o;
  }
  const amber = (n) => { n.style.borderColor = AM; n.style.boxShadow = '0 0 0 1px rgba(11,14,17,.6),0 0 26px rgba(232,163,61,.24)'; return n; };

  /* ---------- 6 · 14.0–17.5 · release gates ---------- */
  S.push({
    a: 14, b: 17.5,
    init(st) {
      const name = pick('05-job-gates', '15-ja-job-gates');
      const sc = NF.screen(name); st.appendChild(sc); this.sc = sc;
      this.rb = rect(name, 'review_banner');
      this.rc = rect(name, 'gate_chips');
      this.rf = rect(name, 'first_gate');
      this.ru = join(this.rc, this.rf);                       // chips + first gate reason, both fully in frame
      this.sA = zoom(this.rb, 1.6); this.sB = zoom(this.rf, 1.55); this.sC = zoom(this.ru, 1.35);
      this.c1 = NF.caption(st, 's6_a'); this.c2 = NF.caption(st, 's6_b'); this.c3 = NF.caption(st, 's6_c');
    },
    render(t, u) {
      const sc = this.sc, rb = this.rb, rc = this.rc, rf = this.rf, ru = this.ru;
      const tilt = E.inOutCubic(seg(u, 1.0, 1.95)), back = E.inOutCubic(seg(u, 2.0, 2.35));
      const s = lerp(lerp(this.sA, this.sB, tilt), this.sC, back);
      const cx = lerp(lerp(cxOf(rb), cxOf(rf), tilt), cxOf(ru), back);
      const cy = lerp(lerp(cyOf(rb), cyOf(rf), tilt), cyOf(ru), back);
      sc.cam(s, cx, cy);
      // three closed rectangles: REVIEW REQUIRED banner -> first gate reason -> all four filter chips
      box(sc, rb, seg(u, .05, .4) * (1 - seg(u, 1.0, 1.25)), 0);
      box(sc, rf, seg(u, 1.15, 1.45) * (1 - seg(u, 2.05, 2.3)), 1);
      box(sc, rc, seg(u, 2.25, 2.55), 2);
      win(this.c1, u, .15, 1.15); win(this.c2, u, 1.15, 2.0); win(this.c3, u, 2.0, 3.4);
      NF.status(true);
    }
  });

  /* ---------- 7 · 17.5–21.0 · PDF → DXF, amber ---------- */
  S.push({
    a: 17.5, b: 21,
    init(st) {
      const na = pick('02-convert', '14-ja-convert'), nb = pick('03-pdf-result', '22-ja-pdf-result');
      const A = NF.screen(na), Bs = NF.screen(nb);
      st.appendChild(A); st.appendChild(Bs); this.A = A; this.B = Bs;
      this.dz = rect(na, 'dropzone');
      this.scl = join(rect(na, 'scale_kind'), rect(na, 'scale_value'));   // kind + value framed together
      this.pill = rect(nb, 'pdf_pill'); this.dxf = rect(nb, 'dxf_button');
      this.res = join(this.pill, this.dxf);                               // pill and Download DXF both kept in frame
      this.sDrop = zoom(this.dz, 1.42); this.sScale = zoom(this.scl, 1.5); this.sRes = zoom(this.res, 1.8);
      this.c1 = NF.caption(st, 's7_a'); this.c2 = NF.caption(st, 's7_b');
      this.sc = el('div', { class: 'mono', style: 'position:absolute;left:80px;top:64px;font-size:34px;font-weight:700;color:' + AM + ';letter-spacing:.06em;opacity:0', text: T('s7_scale') }, st);
    },
    render(t, u) {
      const first = u < 1.5, A = this.A, Bs = this.B;
      A.style.display = first ? 'block' : 'none'; Bs.style.display = first ? 'none' : 'block';
      if (first) {
        const dz = this.dz, sl = this.scl;
        const k1 = E.outCubic(seg(u, .1, .75)), k2 = E.inOutCubic(seg(u, .8, 1.4));
        const s = lerp(lerp(1.08, this.sDrop, k1), this.sScale, k2);
        A.cam(s, lerp(cxOf(dz), cxOf(sl), k2), lerp(cyOf(dz), cyOf(sl), k2));
        amber(box(A, dz, seg(u, .15, .5) * (1 - seg(u, .8, 1.05)), 0));
        amber(box(A, sl, seg(u, .85, 1.15), 1));
        this.sc.style.opacity = seg(u, .9, 1.15).toFixed(3);
        this.sc.style.transform = `translateY(${(14 * (1 - E.outCubic(seg(u, .9, 1.25)))).toFixed(2)}px)`;
        win(this.c1, u, .2, 1.5, true); this.c2.style.opacity = 0;
      } else {
        const v = u - 1.5, res = this.res, k = E.outCubic(seg(v, 0, 1.3));
        Bs.cam(lerp(1.2, this.sRes, k), cxOf(res), cyOf(res));
        const f = amber(box(Bs, this.pill, seg(v, .05, .3), 0));
        f.style.clipPath = `inset(-40% ${(100 * (1 - E.outExpo(seg(v, .05, .45)))).toFixed(2)}% -40% -4%)`;
        amber(box(Bs, this.dxf, seg(v, .55, .85), 1));
        this.sc.style.opacity = 0;
        this.c1.style.opacity = 0; win(this.c2, v, .15, 1.9, true);
      }
      NF.status(true);
    }
  });

  /* ---------- 8 · 21.0–23.5 · EN/JA, offline, phone parallax ---------- */
  S.push({
    a: 21, b: 23.5,
    init(st) {
      const name = pick('17-dark-flat-pattern', '16-ja-flat-pattern');
      const bg = NF.screen(name);
      bg.style.zIndex = '1';
      st.appendChild(bg); this.bg = bg;
      this.pat = rect(name, 'pattern_svg');
      // measured app chrome (tab bar). No measured rect exists for the language/theme control, so nothing is guessed.
      this.chrome = rect(name, 'tabs');

      // dedicated phone layer, explicitly above the screen layer.
      // 780x1688 capture drawn at 390x844 CSS px inside a 14 px dark bezel => 418 x 872 outer.
      const BW = 390, BH = 844, BZ = 14, OW = BW + 2 * BZ, OH = BH + 2 * BZ;
      const left = 1920 - 120 - OW;                 // right edge 120 px from the frame edge
      const top = Math.round((1080 - OH) / 2);      // vertically centred
      const layer = el('div', {
        style: 'position:absolute;left:0;top:0;width:1920px;height:1080px;z-index:6;pointer-events:none'
      }, st);
      const p = el('div', {
        style: `position:absolute;left:${left}px;top:${top}px;width:${OW}px;height:${OH}px;padding:${BZ}px;box-sizing:border-box;` +
          'border-radius:32px;background:linear-gradient(180deg,#1B2229,#0B0E11);opacity:0;transform-origin:50% 50%;' +
          'will-change:transform,opacity;box-shadow:0 34px 70px rgba(0,0,0,.55),0 0 0 1px rgba(242,246,248,.12),inset 0 0 0 1px rgba(242,246,248,.05)'
      }, layer);
      const inner = el('div', { style: `position:relative;width:${BW}px;height:${BH}px;border-radius:20px;overflow:hidden;background:#141A20` }, p);
      el('img', { src: 'screens/19-phone-job.png', alt: '', width: BW, height: BH, style: `display:block;width:${BW}px;height:${BH}px;object-fit:contain;object-position:50% 0` }, inner);
      this.phoneLayer = layer; this.phone = p;
      this.c1 = NF.caption(st, 's8_a');
      this.c1.style.zIndex = '8';
    },
    render(t, u) {
      const px = 12 * E.inOutCubic(seg(u, 1.2, 2.2));
      this.bg.cam(zoom(this.pat, 1.06), cxOf(this.pat) - px, cyOf(this.pat) + px * .35);
      // chrome frame retires before the phone panel arrives, so the panel never covers a measurement frame
      box(this.bg, this.chrome, seg(u, .3, .6) * (1 - seg(u, 1.0, 1.18)), 0);
      // opacity driven only by seg(local, 1.2, 1.6): fully opaque from 22.6 s and held to the end of the shot
      const o = seg(u, 1.2, 1.6);
      const rise = 60 * (1 - E.outBack(seg(u, 1.2, 1.7)));
      const ry = lerp(-12, -6, E.inOutCubic(seg(u, 1.2, 1.9)));
      this.phone.style.opacity = o.toFixed(3);
      this.phone.style.transform = `translate(${(-px * .5).toFixed(2)}px,${rise.toFixed(2)}px) perspective(2400px) rotateY(${ry.toFixed(3)}deg)`;
      win(this.c1, u, .3, 2.4, true);
      NF.status(true, u >= 1.2);
    }
  });

  /* ---------- 9 · 23.5–25.0 · five shape families ---------- */
  const ICONS = [
    ['M20 30H140', 'M20 130H140', 'M20 30C-2 30 -2 130 20 130', 'M140 30C162 30 162 130 140 130', 'M140 30C118 30 118 130 140 130'],
    ['M50 34C50 20 110 20 110 34C110 48 50 48 50 34', 'M10 126C10 108 150 108 150 126C150 144 10 144 10 126', 'M50 34L10 126', 'M110 34L150 126'],
    ['M20 34C20 22 80 22 80 34C80 46 20 46 20 34', 'M20 126C20 108 150 108 150 126C150 144 20 144 20 126', 'M20 34V126', 'M80 34L150 126'],
    ['M40 22C40 8 120 8 120 22C120 36 40 36 40 22', 'M40 22V108', 'M120 22V108', 'M40 108C60 142 100 142 120 108'],
    ['M10 92H150', 'M10 152H150', 'M10 92C-6 92 -6 152 10 152', 'M150 92C166 92 166 152 150 152', 'M62 94L92 20', 'M112 94L142 20', 'M92 20H142', 'M62 94C78 112 96 112 112 94']
  ];
  S.push({
    a: 23.5, b: 25,
    init(st) {
      this.title = el('div', { class: 'slam', text: T('s9_a'), style: 'top:196px;height:auto;font-size:' + (ja ? 88 : 96) + 'px;font-weight:800' }, st);
      const s = svg('svg', { width: 1920, height: 1080, viewBox: '0 0 1920 1080', style: 'position:absolute;left:0;top:0' }, st);
      this.base = svg('path', { d: 'M200 640H1720', stroke: CY, 'stroke-width': 2, fill: 'none', pathLength: 1, 'stroke-dasharray': '1 1' }, s);
      this.paths = []; this.labs = []; this.groups = [];
      for (let i = 0; i < 5; i++) {
        const cx = 360 + i * 300, g = svg('g', { transform: `translate(${cx - 80},468)` }, s), arr = [];
        for (const d of ICONS[i]) arr.push(svg('path', { d, stroke: CY, 'stroke-width': 2.4, fill: 'none', 'stroke-linecap': 'round', pathLength: 1, 'stroke-dasharray': '1 1', 'stroke-dashoffset': 1 }, g));
        this.paths.push(arr); this.groups.push(g);
        // identical 300 px columns, centred, two lines allowed, one shared colour and one shared opacity curve
        this.labs.push(el('div', {
          class: 'lab', text: T('s9_l' + (i + 1)),
          style: `position:absolute;left:${cx - 150}px;top:680px;width:300px;height:auto;text-align:center;` +
            `font-size:${ja ? 21 : 19}px;font-weight:600;line-height:1.3;white-space:normal;color:${INK};` +
            'word-break:normal;overflow-wrap:normal;overflow:visible;opacity:0'
        }, st));
      }
    },
    render(t, u) {
      NF.slam(this.title, seg(u, .02, .36), 'mask');
      const out = 1 - seg(u, 1.36, 1.5);
      this.base.setAttribute('stroke-dashoffset', (1 - E.outCubic(seg(u, .05, .7))).toFixed(4));
      this.base.setAttribute('opacity', out.toFixed(3));
      for (let i = 0; i < 5; i++) {
        const a = .12 + i * .12, p = E.outCubic(seg(u, a, a + .18));
        for (const n of this.paths[i]) n.setAttribute('stroke-dashoffset', (1 - p).toFixed(4));
        this.groups[i].setAttribute('opacity', out.toFixed(3));
        const o = seg(u, a + .02, a + .16);                     // identical opacity curve for all five labels
        this.labs[i].style.opacity = (o * out).toFixed(3);
        this.labs[i].style.color = INK;
        this.labs[i].style.transform = `translateY(${(10 * (1 - p)).toFixed(2)}px)`;
      }
      this.title.style.opacity = (parseFloat(this.title.style.opacity || 0) * out).toFixed(3);
      NF.status(false);
    }
  });

  /* ---------- 10 · 25.0–30.0 · end card ---------- */
  S.push({
    a: 25, b: 30,
    init(st) {
      st.style.background = '#0B0E11';
      const s = svg('svg', { width: 1920, height: 1080, viewBox: '0 0 1920 1080', style: 'position:absolute;left:0;top:0' }, st);
      const pts = NF.geo.saddle(200);
      let x0 = 1e9, x1 = -1e9, y0 = 1e9, y1 = -1e9;
      for (const p of pts) { x0 = Math.min(x0, p[0]); x1 = Math.max(x1, p[0]); y0 = Math.min(y0, p[1]); y1 = Math.max(y1, p[1]); }
      const k = 620 / (x1 - x0), ox = 960 - (x0 + x1) / 2 * k, oy = 268 - (y0 + y1) / 2 * k;
      const d = pts.map((p, i) => (i ? 'L' : 'M') + (ox + p[0] * k).toFixed(2) + ' ' + (oy + p[1] * k).toFixed(2)).join('');
      this.mark = svg('path', { d, stroke: CY, 'stroke-width': 4, fill: 'none', 'stroke-linecap': 'round', pathLength: 1, 'stroke-dasharray': '1 1', 'stroke-dashoffset': 1 }, s);
      this.mrule = svg('path', { d: 'M650 356H1270', stroke: RULE, 'stroke-width': 2, fill: 'none', pathLength: 1, 'stroke-dasharray': '1 1', 'stroke-dashoffset': 1 }, s);
      const ctr = 'position:absolute;left:0;width:1920px;text-align:center;opacity:0;';
      this.wm = el('div', { text: T('s10_wm'), style: ctr + 'top:404px;font-size:104px;font-weight:800;letter-spacing:.16em;color:' + INK }, st);
      this.line = el('div', { text: T('s10_line'), style: ctr + 'top:556px;font-size:' + (ja ? 60 : 64) + 'px;font-weight:600;letter-spacing:' + (ja ? '.02em' : '-.015em') + ';color:' + CY }, st);
      const cw = el('div', { style: ctr + 'top:672px;display:flex;justify-content:center' }, st); this.ctaWrap = cw;
      this.cta = el('div', { text: T('s10_cta'), style: 'padding:20px 46px;border-radius:12px;border:2px solid ' + CY + ';background:rgba(14,124,138,.16);font-size:' + (ja ? 38 : 40) + 'px;font-weight:700;letter-spacing:' + (ja ? '.02em' : '-.01em') + ';color:' + INK }, cw);
      this.contact = el('div', { class: 'mono', text: T('s10_contact'), style: ctr + 'top:806px;font-size:30px;color:' + INK2 }, st);
      const dw = el('div', { style: ctr + 'top:986px;display:flex;justify-content:center;align-items:center;gap:12px' }, st); this.disc = dw;
      el('div', { style: 'width:9px;height:9px;border-radius:50%;background:' + AM }, dw);
      el('div', { text: T('s10_disc'), style: 'font-size:24px;font-weight:600;letter-spacing:' + (ja ? '.02em' : '.04em') + ';color:' + INK2 }, dw);
    },
    render(t, u) {
      this.mark.setAttribute('stroke-dashoffset', (1 - E.inOutCubic(seg(u, .05, .75))).toFixed(4));
      this.mrule.setAttribute('stroke-dashoffset', (1 - E.outCubic(seg(u, .5, 1.0))).toFixed(4));
      const w = seg(u, .3, .7);
      this.wm.style.opacity = w.toFixed(3);
      this.wm.style.transform = `translateY(${(18 * (1 - E.outCubic(w))).toFixed(2)}px)`;
      const ln = seg(u, .85, 1.2);
      this.line.style.opacity = ln.toFixed(3);
      this.line.style.transform = `translateY(${(14 * (1 - E.outCubic(ln))).toFixed(2)}px)`;
      const c = seg(u, 1.5, 2.0);
      this.ctaWrap.style.opacity = clamp(c * 5).toFixed(3);
      this.cta.style.transform = `scale(${lerp(.86, 1, E.outBack(c)).toFixed(4)})`;
      this.contact.style.opacity = seg(u, 1.9, 2.3).toFixed(3);
      this.disc.style.opacity = seg(u, .2, .8).toFixed(3);
      NF.status(false);
    }
  });
})();
