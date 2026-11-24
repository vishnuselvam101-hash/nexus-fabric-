import { PatternViewer, f3, isSeam } from '../../viewer.js';
import { esc } from '../api.js';
import { t, h, idLabel, quantityUnit, engineJa, lang } from '../i18n.js';
import { $, $$ } from '../ui.js';

const ja = () => lang() === 'ja';
const words = (s) => esc(String(s ?? '').replaceAll('_', ' '));
const mono = (s) => `<span class="mono">${esc(s)}</span>`;
const short = (hash) => `<span class="mono" title="${esc(hash)}">${esc(String(hash ?? '').slice(0, 12))}…</span>`;
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : esc(n));
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
const num = (v, unit = '') => (typeof v === 'number'
  ? `<span class="mono" title="${fmt(v)}${unit ? ` ${unit}` : ''}">${n3(v)}</span>${unit ? ` <span class="sub">${esc(unit)}</span>` : ''}`
  : esc(v));
const signed = (v, unit = '') => `<span class="mono" title="${fmt(v)}">${v > 0 ? '+' : ''}${n3(v)}</span>${unit ? ` <span class="sub">${esc(unit)}</span>` : ''}`;
const idText = (kind, id) => idLabel(kind, id) || (ja() ? String(id) : String(id).replaceAll('_', ' '));
const labelled = (kind, id) => { const l = idLabel(kind, id); return l ? esc(l) : ja() ? mono(id) : words(id); };
const partName = (p) => (ja() && idLabel('part', p) ? `${esc(idLabel('part', p))} <span class="mono">${esc(p)}</span>` : esc(p));
const sideText = (s) => idLabel('side', s) || String(s);
const kindText = (k) => idText('entity', k);
const jaHtml = (s) => esc(s).replace(/`([^`]+)`/g, '<span class="mono">$1</span>');
// Prose the engine wrote: reviewed translation when the whole text is known, otherwise the original, marked as English.
const eng = (text) => {
  const s = String(text ?? ''); if (!s) return '';
  if (!ja()) return `<span class="eng" lang="en">${esc(s)}</span>`;
  const j = engineJa(s);
  return j ? `<span lang="ja" title="${esc(s)}">${jaHtml(j)}</span>` : `<span lang="en">${esc(s)}</span>`;
};
const quantity = (k, v) => {
  const l = idLabel('quantity', k), unit = quantityUnit(k);
  const label = `${l ? esc(l) : ja() ? '' : words(k)}<small class="mono">${esc(k)}</small>`;
  if (typeof v !== 'number') return [label, esc(v)];
  if (unit === 'rad') return [label, `${num(v * 180 / Math.PI, '°')} <span class="sub">(${n3(v)} rad)</span>`];
  return [label, unit ? num(v, unit) : `${num(v)} <span class="sub" title="${esc(t('q.nounit.tip'))}">${esc(t('q.nounit'))}</span>`];
};
const extent = (p) => {
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const k of [...p.curves, ...p.holes]) for (const [x, y] of k.points) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; }
  return { w: x1 - x0, h: y1 - y0, x0, x1, y0, y1 };
};
// The engine states the status of eps_rep only in the pattern's assumptions.
const epsRepKey = (p) => {
  if (!(p.eps_rep > 0)) return 'q.epsrep.none';
  const a = (p.assumptions || []).join(' ');
  if (a.includes('eps_rep is NOT certified for this pattern')) return a.includes('its quadrature term is an ESTIMATE') ? 'q.epsrep.est' : 'q.epsrep.uncert';
  if (a.includes('eps_rep is a certified upper bound of the chord error')) return 'q.epsrep.cert';
  return 'q.epsrep.unstated';
};

let seq = 0;

export function mountPattern(host, c) {
  const patterns = (c && c.patterns) || [];
  const prev = (c && (c.prev || c.previous)) || null;
  const prevPatterns = (prev && prev.patterns) || [];
  if (!patterns.length) {
    host.innerHTML = `<div class="alert bad"><b>${esc(t('nopat.t'))}</b><p>${c && c.outcome ? eng(c.outcome) : esc(t('nopat.default'))}</p></div>`;
    return () => { host.textContent = ''; };
  }
  const uid = ++seq, pvId = `pv-${uid}`, ptId = (i) => `pt-${uid}-${i}`;
  let part = 0, ghost = false, alive = true;
  const xy = (q) => `${n3(q[0])}, ${n3(q[1])}`;
  const point = (q) => (q.free ? esc(t('pv.free')) : h('pv.on', { curve: mono(q.curve), i: q.vi }));

  host.innerHTML = `<section class="panel">
    <div class="panel-h">
      <div class="tabs" role="tablist" aria-label="${esc(t('parts.aria'))}">${patterns.map((p, i) => `<button class="btn" role="tab" id="${ptId(i)}" data-part="${i}" aria-controls="${pvId}" aria-selected="${i === 0}" tabindex="${i === 0 ? 0 : -1}">${partName(p.part)} <span class="mono">${esc(p.class)}</span></button>`).join('')}</div>
      <div class="row">
        <button class="btn" data-tool="select" aria-pressed="true" title="${esc(t('tool.select.tip'))}">${esc(t('tool.select'))}</button>
        <button class="btn" data-tool="measure" aria-pressed="false" title="${esc(t('tool.measure.tip'))}">${esc(t('tool.measure'))}</button>
        <button class="btn" data-zoom="out" aria-label="${esc(t('zoom.out'))}" title="${esc(t('zoom.out.tip'))}">−</button>
        <span class="mono" data-zoomv aria-live="off">100%</span>
        <button class="btn" data-zoom="in" aria-label="${esc(t('zoom.in'))}" title="${esc(t('zoom.in.tip'))}">+</button>
        <button class="btn" data-zoom="fit" aria-label="${esc(t('zoom.fit'))}" title="${esc(t('zoom.fit.tip'))}">${esc(t('zoom.fit'))}</button>
        <button class="btn" data-toggle="grid" aria-pressed="true">${esc(t('disp.grid'))}</button>
        <button class="btn" data-toggle="dims" aria-pressed="true" title="${esc(t('disp.dims.tip'))}">${esc(t('disp.dims'))}</button>
        <button class="btn" data-toggle="labels" aria-pressed="true" title="${esc(t('disp.labels.tip'))}">${esc(t('disp.labels'))}</button>
        ${prevPatterns.length ? `<button class="btn" data-ghost aria-pressed="false" title="${esc(t('disp.prev.tip', { id: prev.id }))}">${esc(t('disp.prev', { id: prev.id }))}</button>` : ''}
      </div>
    </div>
    <div class="panel-b">
      <div class="viewer-container" id="${pvId}" role="tabpanel" data-pv></div>
      <div class="row sub" data-notes role="note" aria-label="${esc(t('cn.aria'))}"></div>
      <div class="row sub" aria-hidden="true"><span><b>X</b> <span class="mono" data-ro-x>—</span></span><span><b>Y</b> <span class="mono" data-ro-y>—</span></span><span data-ro-info></span></div>
    </div>
  </section>
  <section class="panel"><div class="panel-h"><h2>${esc(t('sel.h'))}</h2></div><div class="panel-b"><div data-sel aria-live="polite"></div><div data-meas aria-live="polite"></div></div></section>
  <section class="panel"><div class="panel-h"><h2>${esc(t('curves.h'))}</h2><span class="sub" data-curves-part></span></div><div class="panel-b" data-curves></div></section>
  <section class="panel"><div class="panel-h"><h2>${esc(t('quant.h'))}</h2></div><div class="panel-b" data-quant></div></section>
  <section class="panel"><div class="panel-h"><h2>${esc(t('assume.h'))}</h2></div><div class="panel-b" data-assume></div></section>`;

  const pvHost = $('[data-pv]', host), selBox = $('[data-sel]', host), measBox = $('[data-meas]', host);

  const showSel = (i, k) => {
    $$('tr[data-ci]', host).forEach((tr) => tr.setAttribute('aria-selected', Number(tr.dataset.ci) === i));
    if (i === null || !k) { selBox.innerHTML = `<p class="sub">${esc(t('sel.none'))}</p>`; return; }
    const p = k.points, xs = p.map((q) => q[0]), ys = p.map((q) => q[1]), a = p[0], b = p[p.length - 1], gloss = idLabel('curve', k.name);
    selBox.innerHTML = `<dl class="kv">
      <dt>${esc(t('sel.curve'))}</dt><dd>${mono(k.name)}${gloss ? `<small>${esc(gloss)}</small>` : ''}</dd>
      <dt>${esc(t('sel.entity'))}</dt><dd>${esc(kindText(k.kind))}</dd>
      <dt>${esc(t('sel.layer'))}</dt><dd>${mono(k.layer)}</dd>
      <dt>${esc(t('sel.length'))}</dt><dd>${num(k.length, 'mm')}</dd>
      <dt>${esc(t('sel.vertices'))}</dt><dd>${p.length}</dd>
      <dt>${esc(t('sel.first'))}</dt><dd>${xy(a)} <span class="sub">mm</span></dd>
      <dt>${esc(t('sel.last'))}</dt><dd>${xy(b)} <span class="sub">mm</span></dd>
      <dt>${esc(t('sel.extent'))}</dt><dd>${n3(Math.max(...xs) - Math.min(...xs))} × ${n3(Math.max(...ys) - Math.min(...ys))} <span class="sub">mm</span></dd></dl>
      <p class="sub">${esc(t('sel.hint'))}${k.points_exact === false ? ` ${esc(t('sel.arc', { n: p.length - 1, d: n3(k.points_max_deviation || 0) }))}` : ''}</p>`;
  };

  const viewer = new PatternViewer(pvHost, {
    onSelect: (i, k) => { if (alive) showSel(i, k); },
    onTool: (tool) => {
      if (!alive) return;
      $('[data-tool="select"]', host).setAttribute('aria-pressed', tool === 'select');
      $('[data-tool="measure"]', host).setAttribute('aria-pressed', tool === 'measure');
    },
    onView: (z) => { if (alive) $('[data-zoomv]', host).textContent = `${Math.round(z * 100)}%`; },
    onReadout: (r) => {
      if (!alive) return;
      $('[data-ro-x]', host).textContent = r.cursor ? `${f3(r.cursor.x)} mm` : '—';
      $('[data-ro-y]', host).textContent = r.cursor ? `${f3(r.cursor.y)} mm` : '—';
      let text; const m = r.measure;
      if (m) text = m.b ? t(m.done ? 'ro.dist' : 'ro.dist.live', { d: n3(m.distance), dx: n3(m.dx), dy: n3(m.dy) }) : t('ro.measure2');
      else if (r.hover) {
        const hv = r.hover, base = { name: hv.curve.name, kind: kindText(hv.curve.kind), layer: hv.curve.layer, len: n3(hv.curve.length) };
        text = hv.vertex ? t('ro.hover.v', { ...base, i: hv.vertex.vi + 1, n: hv.curve.points.length, x: n3(hv.vertex.x), y: n3(hv.vertex.y) }) : t('ro.hover', base);
      } else text = t(r.tool === 'measure' ? 'ro.measure1' : 'ro.idle');
      $('[data-ro-info]', host).textContent = text;
      measBox.innerHTML = m && m.b && m.done ? `<h3>${esc(t('meas.h'))}</h3><dl class="kv">
        <dt>${esc(t('meas.dist'))}</dt><dd>${n3(m.distance)} <span class="sub">mm</span></dd>
        <dt>${esc(t('meas.dxy'))}</dt><dd>${n3(m.dx)} · ${n3(m.dy)} <span class="sub">mm</span></dd>
        <dt>${esc(t('meas.from'))}</dt><dd>${point(m.a)} (${xy([m.a.x, m.a.y])})</dd>
        <dt>${esc(t('meas.to'))}</dt><dd>${point(m.b)} (${xy([m.b.x, m.b.y])})</dd></dl>
        <p class="sub">${esc(t('meas.hint'))}</p>` : '';
    },
  }, { t, fmt: n3 });

  const ghostFor = (p) => (ghost ? prevPatterns.find((x) => x.part === p.part) || null : null);

  const showPart = (i) => {
    part = i;
    const p = patterns[i], all = [...p.curves, ...p.holes];
    $$('[data-part]', host).forEach((b, j) => { b.setAttribute('aria-selected', j === i); b.tabIndex = j === i ? 0 : -1; });
    pvHost.setAttribute('aria-labelledby', ptId(i));
    $('[data-curves-part]', host).innerHTML = h('cv.part', { part: partName(p.part) });
    $('[data-curves]', host).innerHTML = `<table class="table"><thead><tr><th>${esc(t('cv.curve'))}</th><th>${esc(t('cv.entity'))}</th><th>${esc(t('cv.length'))}</th></tr></thead><tbody>${all.map((k, j) => {
      const gloss = idLabel('curve', k.name);
      return `<tr data-ci="${j}" aria-selected="false" title="${esc(k.layer)}"><td><button class="btn-text mono" data-ci="${j}" aria-label="${esc(t('cv.select', { name: k.name }))}">${esc(k.name)}</button>${gloss ? `<small>${esc(gloss)}</small>` : ''}</td><td>${esc(kindText(k.kind))}</td><td>${num(k.length)}</td></tr>`;
    }).join('')}</tbody></table>
      <p class="sub">${mono('CUT_OUTER')} ${esc(t('cv.outer'))}${p.holes.length ? ` · ${mono('CUT_INNER')} ${esc(t('cv.inner'))}` : ''}</p>`;

    const e = extent(p), cls = idLabel('devclass', p.class);
    const hole = p.holes.length === 1 ? extent({ curves: [], holes: p.holes }) : null;
    $('[data-quant]', host).innerHTML = `${p.eps_num_governs ? `<div class="alert warn" role="note">${h('q.epsgov', { v: mono(fmt(p.eps_num)) })}</div>` : ''}
      <dl class="kv">
      <dt>${esc(t('q.part'))}</dt><dd>${partName(p.part)}<small>${labelled('family', p.family)}</small></dd>
      <dt>${esc(t('q.class'))}</dt><dd>${mono(p.class)} <span class="sub">${esc(t('q.class.note'))}</span>${cls ? `<small>${esc(cls)}</small>` : ''}</dd>
      <dt>${esc(t('q.mside'))}<small>${esc(t('q.mside.note'))}</small></dt><dd>${p.marking_side ? h('q.mside.v', { side: esc(sideText(p.marking_side)) }) : esc(t('q.mside.none'))}</dd>
      <dt>${esc(t('q.extent'))}<small>${esc(t('q.extent.note'))}</small></dt><dd><span class="mono" title="${fmt(e.w)} × ${fmt(e.h)} mm">${n3(e.w)} × ${n3(e.h)}</span> <span class="sub">mm</span></dd>
      ${Object.entries(p.quantities || {}).map(([k, v]) => { const [l, val] = quantity(k, v); return `<dt>${l}</dt><dd>${val}</dd>`; }).join('')}
      ${hole ? `<dt>${esc(t('q.opening'))}<small>${esc(t('q.opening.note'))}</small></dt><dd>${signed((hole.y0 + hole.y1) / 2 - (e.y0 + e.y1) / 2, 'mm')}</dd>` : ''}
      <dt>${esc(t('q.epsnum'))}<small>${esc(t('q.eps.note'))}</small></dt><dd>${num(p.eps_num, 'mm')}</dd>
      <dt>${esc(t('q.epsrep'))}<small>${esc(t(epsRepKey(p)))}</small></dt><dd>${num(p.eps_rep, 'mm')}</dd>
      <dt>${esc(t('q.sha'))}</dt><dd>${short(p.dxf_sha256)}</dd></dl>
      <p class="sub">${esc(t('q.hint'))}</p>`;
    $('[data-assume]', host).innerHTML = `<ul class="il">${(p.assumptions || []).map((a) => `<li>${eng(a)}</li>`).join('')}</ul>`;
    const seams = all.filter(isSeam).map((k) => k.name);
    $('[data-notes]', host).innerHTML = `<span>${p.marking_side ? h('cn.side', { side: esc(sideText(p.marking_side)) }) : esc(t('cn.side.none'))}</span>
      <span>${esc(t('cn.not'))}</span>${seams.length ? `<span title="${esc(t('cn.seams.tip'))}">${h('cn.seams', { names: seams.map(mono).join(', ') })}</span>` : ''}`;
    viewer.setPattern(p); showSel(null, null);
    viewer.setGhost(ghostFor(p));
  };

  const onClick = (e) => {
    const b = e.target.closest && e.target.closest('button');
    if (!b || !host.contains(b)) return;
    if (b.dataset.part !== undefined) { showPart(Number(b.dataset.part)); return; }
    if (b.dataset.ci !== undefined) { const i = Number(b.dataset.ci); viewer.select(i === viewer.sel ? null : i); return; }
    if (b.dataset.tool) { viewer.setTool(b.dataset.tool === 'measure' && viewer.tool === 'measure' ? 'select' : b.dataset.tool); return; }
    if (b.dataset.zoom) { const z = b.dataset.zoom; if (z === 'fit') viewer.fit(); else viewer.zoomBy(z === 'in' ? 1.25 : 0.8); return; }
    if (b.dataset.toggle) { b.setAttribute('aria-pressed', viewer.toggle(b.dataset.toggle)); return; }
    if (b.hasAttribute('data-ghost')) { ghost = !ghost; b.setAttribute('aria-pressed', ghost); viewer.setGhost(ghostFor(patterns[part])); }
  };

  const onKey = (e) => {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const el0 = e.target;
    if (el0 instanceof Element && el0.matches('input,textarea,select,[contenteditable]')) return;
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (el0.closest && el0.closest('[data-part]') && ['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(k)) {
      const n = patterns.length;
      const j = k === 'ArrowRight' ? (part + 1) % n : k === 'ArrowLeft' ? (part - 1 + n) % n : k === 'Home' ? 0 : n - 1;
      e.preventDefault(); showPart(j); $$('[data-part]', host)[j].focus(); return;
    }
    let used = true;
    if (k === 'f') viewer.fit();
    else if (k === 'm') viewer.setTool(viewer.tool === 'measure' ? 'select' : 'measure');
    else if (k === '+' || k === '=') viewer.zoomBy(1.25);
    else if (k === '-') viewer.zoomBy(0.8);
    else if (/^[1-9]$/.test(k) && Number(k) <= patterns.length) showPart(Number(k) - 1);
    else used = false;
    if (used) e.preventDefault();
  };

  host.addEventListener('click', onClick);
  host.addEventListener('keydown', onKey);
  viewer.showPart = showPart; viewer.partCount = patterns.length;
  showPart(0);

  return () => {
    alive = false;
    host.removeEventListener('click', onClick);
    host.removeEventListener('keydown', onKey);
    viewer.destroy();
    host.textContent = '';
  };
}
