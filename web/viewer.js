// NEXUS FABRIC pattern viewer. Dependency-free SVG view of a computed flat pattern.
// Everything drawn or read out comes from the pattern data returned by the server:
// curve vertices, curve lengths, curve names, and extents derived from those vertices. Nothing is invented.
// The viewer knows nothing about the application or its language: text comes in through `opts.t`,
// number formatting through `opts.fmt`.

const NS = 'http://www.w3.org/2000/svg';
const el = (name, attrs, parent) => {
  const e = document.createElementNS(NS, name);
  if (attrs) for (const k in attrs) e.setAttribute(k, attrs[k]);
  if (parent) parent.appendChild(e);
  return e;
};
export const f3 = (n) => (Math.abs(n) < 5e-4 ? 0 : n).toFixed(3);
const stepLabel = (v, step) => (step >= 1 ? String(Math.round(v)) : v.toFixed(Math.min(4, Math.ceil(-Math.log10(step)))));
// ruler labels: thousands and millions of mm are abbreviated (12k = 12 000 mm, 3M = 3 000 000 mm) so they never clip
const rulerLabel = (v, step) => (step >= 1e6 ? `${+(v / 1e6).toFixed(3)}M` : step >= 1e3 ? `${+(v / 1e3).toFixed(3)}k` : stepLabel(v, step));
const px = (v) => Math.round(v) + 0.5; // 1 px rules land on device pixels
// A curve the engine names as a seam edge. The name is the only record of it: neither the result nor the DXF
// carries a seam attribute (review RB-003), so the viewer marks exactly the curves so named and nothing else.
export const isSeam = (curve) => /^seam(_|$)/.test(curve.name);

export class PatternViewer {
  constructor(host, callbacks = {}, opts = {}) {
    this.host = host; this.cb = callbacks;
    this.t = opts.t || ((k) => k); this.fmt = opts.fmt || f3;
    this.tool = 'select'; this.showGrid = true; this.showDims = true; this.showLabels = true;
    this.curves = []; this.sel = null; this.hov = null; this.snap = null; this.meas = null; this.cursor = null; this.cursorPx = null;
    this.view = { s: 1, tx: 0, ty: 0 }; this.fitted = true; this.fitScale = 1; this.W = 0; this.H = 0;
    const svg = this.svg = el('svg', { class: 'pv', tabindex: '0', role: 'application', 'aria-roledescription': this.t('pv.role') }, host);
    this.gGrid = el('g', { 'aria-hidden': 'true' }, svg);
    this.gWorld = el('g', {}, svg);
    this.gOver = el('g', { 'aria-hidden': 'true', 'pointer-events': 'none' }, svg);
    this.gRule = el('g', { 'aria-hidden': 'true', 'pointer-events': 'none' }, svg); // rulers lie over the geometry, as on a drawing board
    this.ro = new ResizeObserver(() => this.resize()); this.ro.observe(host);
    this.bind();
  }

  destroy() { this.ro.disconnect(); cancelAnimationFrame(this.raf); this.svg.remove(); }

  setPattern(p) {
    this.pattern = p;
    this.curves = [...p.curves, ...p.holes];
    this.sel = this.hov = this.snap = this.meas = null;
    let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
    for (const c of this.curves) for (const [x, y] of c.points) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; }
    this.bbox = { x0, x1, y0, y1, w: x1 - x0, h: y1 - y0 };
    this.svg.setAttribute('aria-label', this.t('pv.aria', { part: p.part, n: this.curves.length, w: f3(this.bbox.w), h: f3(this.bbox.h) }));
    const w = this.gWorld; w.textContent = ''; this.gGhost = el('g', {}, w);
    // material fill: outer profile minus openings, only if the outer curves chain into one closed loop
    const tol = 1e-6 * Math.max(1, this.bbox.w, this.bbox.h);
    const near = (a, b) => Math.abs(a[0] - b[0]) <= tol && Math.abs(a[1] - b[1]) <= tol;
    const outer = p.curves; let chained = outer.length > 0;
    for (let i = 0; i < outer.length && chained; i++) { const a = outer[i].points, b = outer[(i + 1) % outer.length].points; chained = near(a[a.length - 1], b[0]); }
    this.chained = chained;
    // signed area of the outer loop: positive = counter-clockwise, the material is then on the left of every edge
    let area2 = 0;
    if (chained) {
      const loop = (pts) => 'M' + pts.map((q) => `${q[0]} ${q[1]}`).join('L') + 'Z';
      const ring = outer.flatMap((c) => c.points);
      for (let i = 0; i < ring.length; i++) { const a = ring[i], b = ring[(i + 1) % ring.length]; area2 += a[0] * b[1] - b[0] * a[1]; }
      el('path', { class: 'fill', 'fill-rule': 'evenodd', d: loop(ring) + p.holes.map((h) => loop(h.points)).join('') }, w);
    }
    this.inward = area2 >= 0 ? 1 : -1;
    this.hits = [];
    this.lines = this.curves.map((c, i) => {
      const pts = c.points.map((q) => `${q[0]},${q[1]}`).join(' ');
      const shape = i >= p.curves.length ? 'polygon' : 'polyline'; // openings are closed profiles
      const line = el(shape, { class: 'c' + (c.layer === 'CUT_INNER' ? ' inner' : ''), points: pts }, w);
      this.hits.push(el(shape, { class: 'hit', points: pts, 'data-ci': i }, w));
      return line;
    });
    // where each curve is labelled: half-way along an outline edge, at the centre of an opening
    this.anchors = this.curves.map((c, i) => {
      const pts = c.points;
      if (i >= p.curves.length) { let sx = 0, sy = 0; for (const q of pts) { sx += q[0]; sy += q[1]; } return { x: sx / pts.length, y: sy / pts.length, nx: 0, ny: 0, hole: true }; }
      let len = 0; const seg = []; for (let k = 1; k < pts.length; k++) { const d = Math.hypot(pts[k][0] - pts[k - 1][0], pts[k][1] - pts[k - 1][1]); seg.push(d); len += d; }
      let run = 0, k = 0; while (k < seg.length - 1 && run + seg[k] < len / 2) run += seg[k++];
      const a = pts[k], b = pts[k + 1] || pts[k], f = seg[k] ? (len / 2 - run) / seg[k] : 0, d = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
      const tx = (b[0] - a[0]) / d, ty = (b[1] - a[1]) / d;
      return { x: a[0] + (b[0] - a[0]) * f, y: a[1] + (b[1] - a[1]) * f, nx: -ty * this.inward, ny: tx * this.inward, hole: false };
    });
    this.fitted = true; this.resize(true); this.emit();
  }

  // Dashed outline of another pattern (the job this one corrects) in the same coordinates, for comparison only.
  setGhost(p) {
    if (!this.gGhost) return; this.gGhost.textContent = '';
    if (p) for (const [list, shape] of [[p.curves, 'polyline'], [p.holes, 'polygon']]) for (const c of list) el(shape, { class: 'ghost', points: c.points.map((q) => `${q[0]},${q[1]}`).join(' ') }, this.gGhost);
  }

  // ---- view
  resize(force) {
    const W = this.host.clientWidth, H = this.host.clientHeight;
    if (!W || !H || (!force && W === this.W && H === this.H)) return;
    const cx = (this.W / 2 - this.view.tx) / this.view.s, cy = (this.view.ty - this.H / 2) / this.view.s;
    this.W = W; this.H = H;
    if (!this.bbox) return;
    this.computeFit();
    if (this.fitted) this.applyFit(); else { this.view.tx = W / 2 - cx * this.view.s; this.view.ty = H / 2 + cy * this.view.s; }
    this.draw();
  }
  computeFit() {
    const b = this.bbox, ml = 72, mr = 76, mt = 52, mb = 112; // room for rulers and extent dimensions
    this.fitScale = Math.max(1e-6, Math.min((this.W - ml - mr) / (b.w || 1), (this.H - mt - mb) / (b.h || 1)));
    this.fitT = { tx: ml + (this.W - ml - mr) / 2 - (b.x0 + b.w / 2) * this.fitScale, ty: mt + (this.H - mt - mb) / 2 + (b.y0 + b.h / 2) * this.fitScale };
  }
  applyFit() { this.view = { s: this.fitScale, ...this.fitT }; this.fitted = true; }
  fit() { if (!this.bbox) return; this.computeFit(); this.applyFit(); this.draw(); }
  zoomBy(f, sx = this.W / 2, sy = this.H / 2) {
    const v = this.view, s = Math.min(this.fitScale * 2000, Math.max(this.fitScale * 0.1, v.s * f)), k = s / v.s;
    v.tx = sx - (sx - v.tx) * k; v.ty = sy - (sy - v.ty) * k; v.s = s; this.fitted = false; this.draw();
  }
  panBy(dx, dy) { this.view.tx += dx; this.view.ty += dy; this.fitted = false; this.draw(); }
  toWorld(sx, sy) { return [(sx - this.view.tx) / this.view.s, (this.view.ty - sy) / this.view.s]; }
  toScreen(x, y) { return [x * this.view.s + this.view.tx, this.view.ty - y * this.view.s]; }

  // ---- state
  select(i) {
    this.sel = (i === null || i === undefined || !this.curves[i]) ? null : i;
    this.draw(); if (this.cb.onSelect) this.cb.onSelect(this.sel, this.sel === null ? null : this.curves[this.sel]);
  }
  setTool(t) { this.tool = t; if (t !== 'measure') this.meas = null; this.draw(); if (this.cb.onTool) this.cb.onTool(t); this.emit(); }
  toggle(what) { const k = what === 'grid' ? 'showGrid' : what === 'labels' ? 'showLabels' : 'showDims'; this[k] = !this[k]; this.draw(); return this[k]; }
  // What the user arranged, so that a re-render (a language switch) can put it back exactly.
  getState() { return { view: { ...this.view }, fitted: this.fitted, sel: this.sel, tool: this.tool, meas: this.meas, grid: this.showGrid, dims: this.showDims, labels: this.showLabels, size: [this.W, this.H] }; }
  setState(s) {
    if (!s || !this.bbox) return;
    this.showGrid = s.grid; this.showDims = s.dims; this.showLabels = s.labels;
    if (!s.fitted && s.size[0] === this.W && s.size[1] === this.H) { this.view = { ...s.view }; this.fitted = false; }
    this.tool = s.tool; this.meas = s.tool === 'measure' ? s.meas : null;
    if (this.cb.onTool) this.cb.onTool(this.tool);
    this.select(s.sel !== null && this.curves[s.sel] ? s.sel : null); this.emit();
  }

  nearestVertex(sx, sy, maxPx, only) {
    let best = null; const max2 = maxPx * maxPx;
    const scan = (ci) => { const pts = this.curves[ci].points; for (let vi = 0; vi < pts.length; vi++) { const [qx, qy] = this.toScreen(pts[vi][0], pts[vi][1]); const d = (qx - sx) ** 2 + (qy - sy) ** 2; if (d <= max2 && (!best || d < best.d)) best = { ci, vi, x: pts[vi][0], y: pts[vi][1], d }; } };
    if (only !== undefined && only !== null) scan(only); else for (let ci = 0; ci < this.curves.length; ci++) scan(ci);
    return best;
  }

  // ---- drawing
  draw() { cancelAnimationFrame(this.raf); this.raf = requestAnimationFrame(() => this.render()); }
  render() {
    if (!this.bbox || !this.W) return;
    const { s, tx, ty } = this.view, W = this.W, H = this.H;
    this.gWorld.setAttribute('transform', `matrix(${s} 0 0 ${-s} ${tx} ${ty})`);
    this.lines.forEach((l, i) => { l.classList.toggle('sel', i === this.sel); l.classList.toggle('hov', i === this.hov && i !== this.sel); this.hits[i].classList.toggle('halo', i === this.sel); });

    // grid: a 1-2-5 step that keeps major lines about 72 px apart at every zoom
    const g = this.gGrid; g.textContent = '';
    el('rect', { class: 'frame', x: 6.5, y: 6.5, width: W - 13, height: H - 13 }, g); // the sheet border
    const raw = 72 / s, pow = 10 ** Math.floor(Math.log10(raw)), step = [1, 2, 5, 10].find((m) => m * pow >= raw) * pow, minor = step / 5;
    const [wx0, wy1] = this.toWorld(0, 0), [wx1, wy0] = this.toWorld(W, H);
    this.step = step;
    if (this.showGrid) {
      let dm = '', dM = '';
      for (let x = Math.ceil(wx0 / minor) * minor; x <= wx1; x += minor) dm += `M${px(x * s + tx)} 0V${H}`;
      for (let y = Math.ceil(wy0 / minor) * minor; y <= wy1; y += minor) dm += `M0 ${px(ty - y * s)}H${W}`;
      for (let x = Math.ceil(wx0 / step) * step; x <= wx1; x += step) dM += `M${px(x * s + tx)} 0V${H}`;
      for (let y = Math.ceil(wy0 / step) * step; y <= wy1; y += step) dM += `M0 ${px(ty - y * s)}H${W}`;
      el('path', { class: 'g-minor', d: dm }, g); el('path', { class: 'g-major', d: dM }, g);
      el('path', { class: 'g-axis', d: `M${px(tx)} 0V${H}M0 ${px(ty)}H${W}` }, g);
    }

    // overlay
    const o = this.gOver; o.textContent = '';
    const b = this.bbox;
    if (this.showLabels && Math.max(b.w, b.h) * s > 150) this.renderNames(o);
    if (this.showDims) {
      const [ax, ay] = this.toScreen(b.x0, b.y0), [bx, by] = this.toScreen(b.x1, b.y1);
      // drafting dimensions: thin lines, extension lines with a gap from the plate, filled terminators on the dimension
      // line, each figure centred above its line (left of the vertical one, which reads bottom to top)
      const yb = px(ay + 30), xr = px(bx + 30), X0 = px(ax), X1 = px(bx), Y0 = px(ay), Y1 = px(by), g = 4, over = 3, ah = 8, aw = 2.5;
      const tipH = (x, y, dir) => `M${x} ${y}L${x + dir * ah} ${y - aw}L${x + dir * ah} ${y + aw}Z`;
      const tipV = (x, y, dir) => `M${x} ${y}L${x - aw} ${y + dir * ah}L${x + aw} ${y + dir * ah}Z`;
      el('path', { class: 'dim', d: `M${X0} ${ay + g}V${yb + over}M${X1} ${ay + g}V${yb + over}M${X0} ${yb}H${X1}`
        + `M${bx + g} ${Y0}H${xr + over}M${bx + g} ${Y1}H${xr + over}M${xr} ${Y0}V${Y1}` }, o);
      el('path', { class: 'dimh', d: tipH(X0, yb, 1) + tipH(X1, yb, -1) + tipV(xr, Y0, -1) + tipV(xr, Y1, 1) }, o);
      el('text', { class: 'dimt', x: (X0 + X1) / 2, y: yb - 7, 'text-anchor': 'middle' }, o).textContent = this.fmt(b.w);
      el('text', { class: 'dimt', 'text-anchor': 'middle', transform: `translate(${xr - 8} ${(Y0 + Y1) / 2}) rotate(-90)` }, o).textContent = this.fmt(b.h);
    }
    if (this.sel !== null) {
      const pts = this.curves[this.sel].points;
      const hole = this.sel >= this.pattern.curves.length; // openings are closed loops: mark the first vertex only
      for (const [q, lab] of hole ? [[pts[0], this.t('pv.v1')]] : [[pts[0], this.t('pv.first')], [pts[pts.length - 1], this.t('pv.last')]]) { const [X, Y] = this.toScreen(q[0], q[1]); el('rect', { class: 'endp', x: X - 3.5, y: Y - 3.5, width: 7, height: 7 }, o); el('text', { class: 'dimt', x: X + 8, y: Y - 7 }, o).textContent = lab; }
    }
    if (this.snap) { const [X, Y] = this.toScreen(this.snap.x, this.snap.y); el('circle', { class: 'snap', cx: X, cy: Y, r: 4.5 }, o); }
    if (this.meas) {
      const m = this.meas, a = m.a, bb = m.b || m.live;
      const [X1, Y1] = this.toScreen(a.x, a.y); el('circle', { class: 'measp', cx: X1, cy: Y1, r: 3.5 }, o);
      if (bb) {
        const [X2, Y2] = this.toScreen(bb.x, bb.y);
        el('path', { class: 'meas', d: `M${X1} ${Y1}L${X2} ${Y2}` }, o); el('circle', { class: 'measp', cx: X2, cy: Y2, r: 3.5 }, o);
        el('text', { class: 'meast', x: (X1 + X2) / 2 + 8, y: (Y1 + Y2) / 2 - 8 }, o).textContent = `${this.fmt(Math.hypot(bb.x - a.x, bb.y - a.y))} mm`;
      }
    }

    // rulers: bands with ticks every minor step, a label at every major step, and the cursor position
    const r = this.gRule; r.textContent = '';
    const RT = 22, lab = []; for (let y = Math.ceil(wy0 / step) * step; y <= wy1; y += step) lab.push(rulerLabel(y, step));
    const RL = Math.max(46, 14 + 6.7 * Math.max(...lab.map((t) => t.length), 2));
    this.RL = RL; this.RT = RT;
    el('rect', { class: 'ruler', x: 0, y: 0, width: W, height: RT }, r); el('rect', { class: 'ruler', x: 0, y: 0, width: RL, height: H }, r);
    let tk = '';
    for (let x = Math.ceil(wx0 / minor) * minor; x <= wx1; x += minor) { const X = px(x * s + tx); if (X > RL) tk += `M${X} ${RT - 3}V${RT}`; }
    for (let y = Math.ceil(wy0 / minor) * minor; y <= wy1; y += minor) { const Y = px(ty - y * s); if (Y > RT) tk += `M${RL - 3} ${Y}H${RL}`; }
    for (let x = Math.ceil(wx0 / step) * step; x <= wx1; x += step) { const X = x * s + tx; if (X > RL) { tk += `M${px(X)} ${RT - 7}V${RT}`; if (X > RL + 4) el('text', { x: X + 4, y: 13 }, r).textContent = rulerLabel(x, step); } }
    for (let y = Math.ceil(wy0 / step) * step; y <= wy1; y += step) { const Y = ty - y * s; if (Y > RT) { tk += `M${RL - 7} ${px(Y)}H${RL}`; if (Y > RT + 12) el('text', { x: RL - 9, y: Y - 4, 'text-anchor': 'end' }, r).textContent = rulerLabel(y, step); } }
    el('path', { class: 'ruler-tick', d: tk }, r);
    el('path', { class: 'ruler-line', d: `M0 ${RT + 0.5}H${W}M${RL + 0.5} 0V${H}` }, r);
    if (this.cursorPx) { const [cx, cy] = this.cursorPx; if (cx > RL && cy > RT) el('path', { class: 'ruler-cur', d: `M${px(cx)} 0V${RT}M0 ${px(cy)}H${RL}` }, r); }
    el('rect', { class: 'ruler', x: 0, y: 0, width: RL, height: RT }, r); el('text', { class: 'ruler-unit', x: RL - 9, y: 14, 'text-anchor': 'end' }, r).textContent = 'mm';
    // scale bar: one major step, bottom right
    const sb = step * s, sx0 = RL + 16; // top left, under the ruler: the title block takes the bottom right corner
    const sy = RT + 26;
    el('rect', { class: 'scale-bg', x: sx0 - 8, y: sy - 22, width: sb + 16, height: 26, rx: 2 }, r);
    el('path', { class: 'scale', d: `M${px(sx0)} ${sy - 6}v6h${Math.round(sb)}v-6` }, r);
    el('text', { class: 'dimt', x: sx0 + sb / 2, y: sy - 9, 'text-anchor': 'middle' }, r).textContent = `${stepLabel(step, step)} mm`;
    if (this.cb.onView) this.cb.onView(this.view.s / this.fitScale);
  }

  // Curve names from the result, and a hatch on the curves the engine names as seams. Screen space: the text
  // keeps one size at every zoom.
  renderNames(o) {
    const W = this.W, H = this.H, n = this.pattern.curves.length;
    this.curves.forEach((c, i) => {
      const a = this.anchors[i], [X, Y] = this.toScreen(a.x, a.y);
      const nx = a.nx, ny = -a.ny; // screen y points down
      if (i < n && isSeam(c)) this.hatch(o, c, nx, ny);
      if (X < this.RL || X > W || Y < this.RT || Y > H) return;
      const tx = el('text', { class: 'name mono' + (c.layer === 'CUT_INNER' ? ' inner' : '') + (i === this.sel ? ' sel' : '') }, o); tx.textContent = c.name;
      if (a.hole) { tx.setAttribute('x', X); tx.setAttribute('y', Y + 4); tx.setAttribute('text-anchor', 'middle'); return; }
      const side = Math.abs(nx) > 0.5;
      tx.setAttribute('x', X + nx * 12); tx.setAttribute('y', Y + ny * 12 + (side || ny > 0 ? 4 : 5));
      tx.setAttribute('text-anchor', side ? (nx > 0 ? 'start' : 'end') : 'middle');
    });
  }
  // Short strokes on the material side of a seam edge, drawn only for the part of the edge that is on screen.
  hatch(o, c, nx, ny) {
    const W = this.W, H = this.H, pts = c.points; let d = '', count = 0;
    for (let k = 1; k < pts.length && count < 600; k++) {
      const [x1, y1] = this.toScreen(pts[k - 1][0], pts[k - 1][1]), [x2, y2] = this.toScreen(pts[k][0], pts[k][1]);
      const len = Math.hypot(x2 - x1, y2 - y1); if (len < 1) continue;
      const ux = (x2 - x1) / len, uy = (y2 - y1) / len;
      // Liang–Barsky: the stretch of this segment inside the canvas, so that a deep zoom never loops over kilometres
      let t0 = 0, t1 = len;
      for (const [p, q] of [[-ux, x1 + 20], [ux, W + 20 - x1], [-uy, y1 + 20], [uy, H + 20 - y1]]) { if (p === 0) { if (q < 0) t0 = t1 + 1; } else { const t = q / p; if (p < 0) t0 = Math.max(t0, t); else t1 = Math.min(t1, t); } }
      // local inward normal of this segment (the anchor normal is only exact for a straight edge)
      const ix = pts.length > 2 ? uy * this.inward : nx, iy = pts.length > 2 ? -ux * this.inward : ny;
      for (let t = Math.ceil(t0 / 9) * 9 + 4.5; t <= t1 && count < 600; t += 9, count++) { const X = x1 + ux * t, Y = y1 + uy * t; d += `M${X.toFixed(1)} ${Y.toFixed(1)}l${(ix * 5 + ux * 3).toFixed(1)} ${(iy * 5 + uy * 3).toFixed(1)}`; }
    }
    if (d) el('path', { class: 'seam', d }, o);
  }

  emit() {
    if (!this.cb.onReadout) return;
    const m = this.meas, b = m && (m.b || m.live);
    this.cb.onReadout({
      cursor: this.cursor, tool: this.tool,
      hover: this.hov === null ? null : { curve: this.curves[this.hov], index: this.hov, vertex: this.snap && this.snap.ci === this.hov ? this.snap : null },
      measure: m ? { a: m.a, b: b || null, done: !!m.b, distance: b ? Math.hypot(b.x - m.a.x, b.y - m.a.y) : null, dx: b ? b.x - m.a.x : null, dy: b ? b.y - m.a.y : null } : null,
    });
  }

  // ---- input
  bind() {
    const svg = this.svg; let down = null;
    const pos = (e) => { const r = svg.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };
    const pick = (sx, sy) => { // measurement point: a real vertex if one is within reach, otherwise the free cursor position
      const v = this.nearestVertex(sx, sy, 10);
      if (v) return { x: v.x, y: v.y, free: false, curve: this.curves[v.ci].name, vi: v.vi + 1 };
      const [x, y] = this.toWorld(sx, sy); return { x, y, free: true };
    };
    svg.addEventListener('pointerdown', (e) => { if (e.button > 1) return; const [sx, sy] = pos(e); down = { sx, sy, moved: false, id: e.pointerId, target: e.target }; });
    svg.addEventListener('pointermove', (e) => {
      const [sx, sy] = pos(e);
      if (down) {
        const dx = sx - down.sx, dy = sy - down.sy;
        if (!down.moved && Math.hypot(dx, dy) > 3) { down.moved = true; svg.setPointerCapture(down.id); svg.classList.add('panning'); }
        if (down.moved) { this.cursorPx = null; this.panBy(dx, dy); down.sx = sx; down.sy = sy; }
        return;
      }
      const [x, y] = this.toWorld(sx, sy); this.cursor = { x, y }; this.cursorPx = [sx, sy];
      const ci = e.target.dataset && e.target.dataset.ci !== undefined ? Number(e.target.dataset.ci) : null;
      this.hov = ci;
      if (this.tool === 'measure') { const q = pick(sx, sy); this.snap = q.free ? null : q; if (this.meas && !this.meas.b) this.meas.live = q; }
      else this.snap = ci === null ? null : this.nearestVertex(sx, sy, 1e9, ci);
      this.draw(); this.emit();
    });
    const up = (e) => {
      if (!down) return; const d = down; down = null; svg.classList.remove('panning');
      if (d.moved || e.type === 'pointercancel') return;
      const [sx, sy] = pos(e);
      if (this.tool === 'measure') {
        const q = pick(sx, sy);
        if (!this.meas || this.meas.b) this.meas = { a: q, b: null, live: null }; else this.meas.b = q;
        this.draw(); this.emit();
      } else {
        const ci = d.target.dataset && d.target.dataset.ci !== undefined ? Number(d.target.dataset.ci) : null;
        this.select(ci);
      }
    };
    svg.addEventListener('pointerup', up); svg.addEventListener('pointercancel', up);
    svg.addEventListener('pointerleave', () => { if (down) return; this.cursor = null; this.cursorPx = null; this.hov = null; this.snap = null; this.draw(); this.emit(); });
    svg.addEventListener('wheel', (e) => { e.preventDefault(); const [sx, sy] = pos(e); this.zoomBy(Math.exp(-e.deltaY * (e.deltaMode ? 0.05 : 0.0015)), sx, sy); }, { passive: false });
    svg.addEventListener('dblclick', (e) => { e.preventDefault(); this.fit(); });
    svg.addEventListener('keydown', (e) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const step = e.shiftKey ? 200 : 40; let used = true;
      switch (e.key) {
        case 'ArrowLeft': this.panBy(step, 0); break;
        case 'ArrowRight': this.panBy(-step, 0); break;
        case 'ArrowUp': this.panBy(0, step); break;
        case 'ArrowDown': this.panBy(0, -step); break;
        case '+': case '=': this.zoomBy(1.25); break;
        case '-': case '_': this.zoomBy(0.8); break;
        case 'f': case '0': this.fit(); break;
        case 'm': this.setTool(this.tool === 'measure' ? 'select' : 'measure'); break;
        case 'Escape': if (this.meas) { this.meas = null; this.draw(); this.emit(); } else if (this.tool !== 'select') this.setTool('select'); else if (this.sel !== null) this.select(null); else used = false; break;
        default: used = false;
      }
      if (used) { e.preventDefault(); e.stopPropagation(); }
    });
  }
}
