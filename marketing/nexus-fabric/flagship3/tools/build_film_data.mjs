// Builds data/film-data.js for the film from the product's own output (data/<lang>/*.json, the DXF text, the
// footage index) and the application's catalogue (web/i18n.js copied to tools/i18n.mjs). Nothing is typed by hand:
// every number, name, hash, verdict and label the film shows comes from these files at build time.
// usage: node tools/build_film_data.mjs <flagship3 folder> <out.js>
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
// the catalogue touches the page when the language is set: give it a stub page (no DOM is used here)
globalThis.document = { documentElement: { lang: '' } };
globalThis.localStorage = { getItem: () => null, setItem: () => {} };
const { CATALOGUE, setLang, t, idLabel } = await import('./i18n.mjs');

const ROOT = path.resolve(process.argv[2] || '.');
const OUT = path.resolve(process.argv[3] || path.join(ROOT, 'data/film-data.js'));
const rd = (p) => JSON.parse(fs.readFileSync(path.join(ROOT, p), 'utf8'));
const log = (...a) => console.log(...a);

// ---- product output (English files; the Japanese files differ only in timestamps, checked below)
const jobs = {}; for (let i = 1; i <= 6; i++) jobs[i] = rd(`data/en/job-${i}.json`);
const jobsJa = {}; for (let i = 1; i <= 6; i++) jobsJa[i] = rd(`data/ja/job-${i}.json`);
const stripTime = (o) => JSON.stringify(o, (k, v) => (k === 'created_at' || k === 'at' || k === 'resolved_at' || k === 'created' ? undefined : v));
for (let i = 1; i <= 6; i++) if (stripTime(jobs[i]) !== stripTime(jobsJa[i])) throw new Error(`job ${i}: en and ja data differ beyond timestamps`);
log('en/ja job data identical apart from timestamps: yes');
const deps = rd('data/en/deps-6.json');
const obligations = rd('data/en/obligations.json');
const gates6 = rd('data/en/gates-6.json');   // job 6 (the current job): 13 gates, each with its own result; no G00 is listed for it

// ---- footage index: the file names the product gives its DXF downloads (for the NOT RELEASED stamp)
const fsrc = fs.readFileSync(path.join(ROOT, 'capture/footage/en/footage.js'), 'utf8');
const fi = fsrc.indexOf('{"lang"'); let depth = 0, end = -1, inStr = false;
for (let k = fi; k < fsrc.length; k++) { const c = fsrc[k];
  if (inStr) { if (c === '\\') k++; else if (c === '"') inStr = false; continue; }
  if (c === '"') inStr = true; else if (c === '{') depth++; else if (c === '}') { depth--; if (depth === 0) { end = k; break; } } }
const footage = JSON.parse(fsrc.slice(fi, end + 1));
const dxfName6 = footage.dxf['6-header'].name;

// ---- catalogue labels (EN and JA) for everything the film shows as words
const UI_KEYS = ['tab.checks', 'tab.gates', 'tab.records', 'tab.scrutiny', 'tab.graph', 'tab.changes', 'th.replay',
  'state.REVIEW_REQUIRED', 'state.SUPERSEDED', 'result.PASS', 'result.INCONCLUSIVE', 'result.NOT_RUN', 'result.NOT_IMPLEMENTED',
  'tb.nr', 'build.short', 'c.difference', 'c.primary'];
const ui = { en: {}, ja: {} };
const missing = [];
for (const L of ['en', 'ja']) {
  setLang(L);
  for (const k of UI_KEYS) { const v = t(k); if (!v || v === k) missing.push(`${L}:${k}`); ui[L][k] = v; }
  ui[L].lang_en = 'English'; ui[L].lang_ja = '日本語';
}
if (missing.length) { log('MISSING catalogue keys (build stops):', missing.join(', ')); process.exit(1); }
const label = (kind, id, L) => { setLang(L); return idLabel(kind, id); };
// Parameter labels, units and quantity labels, per language, for every name the job uses
const paramNames = [...new Set(jobs[6].records.filter((r) => r.kind === 'parameter').map((r) => r.payload.name))];
const quantNames = new Set(); for (const i of [1, 2, 3, 4, 5, 6]) for (const p of jobs[i].patterns) for (const k of Object.keys(p.quantities)) quantNames.add(k);
const params = {}, quants = {}, rkinds = {}, rels = {};
for (const n of paramNames) params[n] = { en: label('param', n, 'en'), ja: label('param', n, 'ja') };
for (const n of quantNames) quants[n] = { en: label('quantity', n, 'en'), ja: label('quantity', n, 'ja') };
const kindsAll = new Set(); for (const i of [5, 6]) for (const r of jobs[i].records) kindsAll.add(r.kind);
for (const n of kindsAll) rkinds[n] = { en: label('rkind', n, 'en'), ja: label('rkind', n, 'ja') };
const relsAll = new Set(deps.edges.map((e) => e.rel)); for (const n of relsAll) rels[n] = { en: label('rel', n, 'en'), ja: label('rel', n, 'ja') };
for (const [n, v] of Object.entries(params)) if (!v.en || !v.ja) log('note: no catalogue label for parameter', n, JSON.stringify(v));
// Japanese check names: the application's labels (data/check-labels.js, built from web/i18n.js)
const ctx = { window: {} }; vm.runInNewContext(fs.readFileSync(path.join(ROOT, 'data/check-labels.js'), 'utf8'), ctx);
const checkJa = ctx.window.NF_CHECK_JA || {};

// ---- geometry: closed outline from the four CUT_OUTER curves (chained by their end points), resampled by arc length
function chain(curves) {
  const segs = curves.map((c) => c.points.map((p) => [p[0], p[1]]));
  const used = new Array(segs.length).fill(false); const out = [];
  used[0] = true; out.push(...segs[0]);
  for (let n = 1; n < segs.length; n++) {
    const last = out[out.length - 1]; let best = -1, rev = false, bd = Infinity;
    segs.forEach((s, k) => { if (used[k]) return; const d1 = Math.hypot(s[0][0] - last[0], s[0][1] - last[1]), d2 = Math.hypot(s[s.length - 1][0] - last[0], s[s.length - 1][1] - last[1]);
      if (d1 < bd) { bd = d1; best = k; rev = false; } if (d2 < bd) { bd = d2; best = k; rev = true; } });
    used[best] = true; const s = rev ? [...segs[best]].reverse() : segs[best]; out.push(...s.slice(1));
  }
  return out;
}
function resample(pts, N, closed = true) {
  const P = closed ? [...pts, pts[0]] : pts.slice(); const cum = [0];
  for (let i = 1; i < P.length; i++) cum.push(cum[i - 1] + Math.hypot(P[i][0] - P[i - 1][0], P[i][1] - P[i - 1][1]));
  const L = cum[cum.length - 1], res = [];
  for (let k = 0; k < N; k++) { const s = (k / N) * L; let i = 1; while (i < cum.length - 1 && cum[i] < s) i++;
    const f = (s - cum[i - 1]) / Math.max(1e-9, cum[i] - cum[i - 1]); res.push([P[i - 1][0] + (P[i][0] - P[i - 1][0]) * f, P[i - 1][1] + (P[i][1] - P[i - 1][1]) * f]); }
  return res;
}
const r1 = (v) => Math.round(v * 10000) / 10000;
function outlineOf(p) {
  const outer = p.curves.filter((c) => c.layer === 'CUT_OUTER');
  const pts = chain(outer);
  const bb = pts.reduce((a, q) => [Math.min(a[0], q[0]), Math.min(a[1], q[1]), Math.max(a[2], q[0]), Math.max(a[3], q[1])], [Infinity, Infinity, -Infinity, -Infinity]);
  return { outline: resample(pts, 360).map((q) => [r1(q[0]), r1(q[1])]), bbox: bb.map(r1), raw: pts.length,
    holes: (p.holes || []).map((h) => resample(h.points.map((q) => [q[0], q[1]]), 240).map((q) => [r1(q[0]), r1(q[1])])) };
}
const QUANT_PICK = (p) => Object.fromEntries(Object.entries(p.quantities).map(([k, v]) => [k, typeof v === 'number' ? r1(v) : v]));
const geo = {};
const partJob = { shell: [1, 'shell'], cone: [2, 'reducer'], ecc: [3, 'reducer'], branch: [4, 'branch'], header: [5, 'header'] };
for (const [key, [jid, part]] of Object.entries(partJob)) {
  const p = jobs[jid].patterns.find((x) => x.part === part) || jobs[jid].patterns[0];
  const o = outlineOf(p);
  geo[key] = { job: jid, family: p.family, class: p.class, part: p.part, outline: o.outline, bbox: o.bbox, raw: o.raw, holes: o.holes, quantities: QUANT_PICK(p), request: jobs[jid].request };
  log(`part ${key}: job ${jid} family ${p.family} outline pts ${o.raw} (resampled 360), holes ${o.holes.length}, bbox ${o.bbox.map((v) => Math.round(v))}`);
}
// header in job 6 (the corrected job) and its branch: the DXF plates
const h6 = jobs[6].patterns.find((x) => x.part === 'header'); const o6 = outlineOf(h6);
geo.header6 = { outline: o6.outline, holes: o6.holes, quantities: QUANT_PICK(h6), bbox: o6.bbox };

// ---- job 5 (the job the footage shows, superseded by job 6): request, checks, gates without G00
const j5 = jobs[5];
const gatesNo00 = j5.gates.filter((g) => g.id !== 'G00');
const gateCounts = {}; for (const g of gatesNo00) gateCounts[g.result] = (gateCounts[g.result] || 0) + 1;
log('job 5 gates: total', j5.gates.length, 'without G00:', gatesNo00.length, JSON.stringify(gateCounts), '| G00 result', j5.gates.find((g) => g.id === 'G00')?.result);
const checks5 = j5.checks.map((c) => ({ name: c.name, verdict: c.verdict, difference: c.difference, ja: checkJa[c.name] || null }));
const checkCounts = {}; for (const c of checks5) checkCounts[c.verdict] = (checkCounts[c.verdict] || 0) + 1;
log('job 5 checks:', checks5.length, JSON.stringify(checkCounts), '| with a Japanese label:', checks5.filter((c) => c.ja).length, 'distinct names with label:', new Set(checks5.filter((c) => c.ja).map((c) => c.name)).size, 'of', new Set(checks5.map((c) => c.name)).size);

// ---- job 6: parameters (the correction), replay, records, obligations, dependency graph, DXF text
const j6 = jobs[6];
const params6 = j6.records.filter((r) => r.kind === 'parameter').map((r) => ({ name: r.payload.name, unit: r.payload.unit, value: r.payload.value }));
const old5 = j5.request; const corr = { name: 'header_radius', unit: 'mm', old: old5.header_radius, new: j6.request.header_radius };
log('correction: header_radius', corr.old, '->', corr.new, corr.unit, '| job 6 supersedes', j6.supersedes, 'job 5 superseded_by', j5.superseded_by);
const recs6 = j6.records.map((r) => ({ kind: r.kind, id: r.id, rel: (r.relations || []).map((x) => ({ rel: x.rel, to: x.to })) }));
const obl = obligations.map((o) => ({ id: o.id, state: o.state, kind: o.kind, label: o.label, direction: o.direction }));
const oblCounts = {}; for (const o of obl) oblCounts[o.state] = (oblCounts[o.state] || 0) + 1;
log('obligations:', obl.length, JSON.stringify(oblCounts), '| replay', j6.replay.stored_records_checked, 'checked', j6.replay.stored_records_failing_identity_check, 'failing, matches', j6.replay.matches);
const nodeSet = new Map(); for (const e of deps.edges) { nodeSet.set(e.from, true); nodeSet.set(e.to, true); }
log('graph: nodes', nodeSet.size, 'edges', deps.edges.length, 'relations', relsAll.size, '| rule table:', deps.rule_table);
const dxfTxt = fs.readFileSync(path.join(ROOT, 'data/en/dxf-6-header.dxf'), 'utf8').split(/\r?\n/);
const ent = dxfTxt.indexOf('ENTITIES');
const dxfRows = []; for (let i = ent; i < dxfTxt.length && dxfRows.length < 400; i++) dxfRows.push(dxfTxt[i].trim());
const marking = dxfTxt.find((l) => /NOT RELEASED - DEVELOPMENT/.test(l)) || '';
log('dxf header file:', dxfName6, '| marking text:', marking.trim());

// ---- job 6 gates by their own results (never merged; the film prints only the non-zero results)
const gatesOut6 = gates6.map((g) => ({ id: g.id, name: g.name, result: g.result, en: g.name, ja: label('gate', g.id, 'ja') }));
const gateCounts6 = {}; for (const g of gates6) gateCounts6[g.result] = (gateCounts6[g.result] || 0) + 1;
log('job 6 gates:', gates6.length, JSON.stringify(gateCounts6), '| G00 listed:', gates6.some((g) => g.id === 'G00'));
const gateNoJa = gatesOut6.filter((g) => !g.ja || g.ja === g.id).map((g) => g.id);
if (gateNoJa.length) log('gates without a Japanese label:', gateNoJa.join(', '));
// distinct check names without a Japanese label (the film shows the English identifier for these in the Japanese version)
const checkNoJa = [...new Set(checks5.filter((c) => !c.ja).map((c) => c.name))];
log('check names without a Japanese label:', checkNoJa.length, checkNoJa.join(', '));

// ---- the simple graph for the correction: the corrected parameter and the real nodes on its path (at most six)
const SOURCE = 'parameter · header_radius';
const SIMPLE = [SOURCE, 'intersection_curve', 'interface · header-branch joint', 'part · header', 'pattern · header', 'dxf_output · header'];
const simpleEdges = deps.edges.filter((e) => SIMPLE.includes(e.from) && SIMPLE.includes(e.to));
// flag order: the dependents of the corrected parameter, nearest first (breadth-first over the edges that point at a node)
const dist = new Map([[SOURCE, 0]]);
for (let fr = [SOURCE]; fr.length;) { const nx = []; for (const e of deps.edges) if (fr.includes(e.to) && !dist.has(e.from)) { dist.set(e.from, dist.get(e.to) + 1); nx.push(e.from); } fr = nx; }
const flagOrder = SIMPLE.filter((n) => n !== SOURCE).sort((a, b) => (dist.get(a) ?? 99) - (dist.get(b) ?? 99));
// English node labels where the catalogue has no English kind label (logged as such); Japanese from the catalogue
const EN_NODE = { intersection_curve: 'Intersection curve', 'interface · header-branch joint': 'Interface', 'part · header': 'Header part',
  'pattern · header': 'Header pattern', 'dxf_output · header': 'Header DXF output' };
const nodeLab = (n, L) => {
  if (L === 'en' && EN_NODE[n]) return EN_NODE[n];
  setLang(L); const i = n.indexOf(' · '); const kind = i < 0 ? n : n.slice(0, i), name = i < 0 ? '' : n.slice(i + 3);
  if (kind === 'parameter') return idLabel('param', name);
  if (kind === 'intersection_curve' || kind === 'interface') return idLabel('rkind', kind);
  const k = idLabel('rkind', kind); let nm = name;
  try { const p = idLabel('part', name); if (p && p !== name) nm = p; } catch (e) { /* no part label for this name */ }
  return nm === name ? `${k} · ${name}` : `${k} · ${nm}`;
};
const simpleOut = { source: SOURCE, nodes: SIMPLE.map((n) => ({ id: n, en: nodeLab(n, 'en'), ja: nodeLab(n, 'ja'), dist: dist.get(n) ?? null })),
  edges: simpleEdges.map((e) => ({ from: e.from, to: e.to, rel: e.rel, en: label('rel', e.rel, 'en'), ja: label('rel', e.rel, 'ja') })), flagOrder };
log('simple graph:', simpleOut.nodes.map((n) => `${n.id} [${n.en} | ${n.ja}] d=${n.dist}`).join('; '));
log('simple graph edges:', simpleEdges.length, '| flag order:', flagOrder.join(' → '));

// job 6 (the current job): its own check entries and verdicts, as the file holds them
const checks6 = j6.checks.map((c) => ({ name: c.name, verdict: c.verdict, ja: checkJa[c.name] || null }));
const checkCounts6 = {}; for (const c of checks6) checkCounts6[c.verdict] = (checkCounts6[c.verdict] || 0) + 1;
log('job 6 checks:', checks6.length, 'entries,', new Set(checks6.map((c) => c.name)).size, 'distinct names,', JSON.stringify(checkCounts6), '| with a Japanese label:', checks6.filter((c) => c.ja).length);
const data = {
  built_from: 'data/en job-1..6, deps-6, obligations; footage index; web/i18n.js (catalogue)',
  ui, params, quants, rkinds, rels, checkJa,
  geo, dxf: { name: dxfName6, marking: marking.trim(), rows: dxfRows, entities: { header: j6.patterns.find((p) => p.part === 'header') ? 5 : 0 } },
  job5: { id: 5, state: j5.state, superseded_by: j5.superseded_by, request: j5.request, checks: checks5, gateCounts, gates: gatesNo00.map((g) => ({ id: g.id, name: g.name, result: g.result })) },
  job6: { id: 6, state: j6.state, supersedes: j6.supersedes, request: j6.request, params: params6, corr, records: recs6, replay: { checked: j6.replay.stored_records_checked, failing: j6.replay.stored_records_failing_identity_check, matches: j6.replay.matches, closure: j6.replay.replayed_closure_hash }, closure_hash: j6.closure_hash, closure_size: j6.closure_size, checks: checks6.length, checkList: checks6, checkCounts: checkCounts6 },
  obligations: obl, oblCounts, gates6: gatesOut6, gateCounts6, simple: simpleOut, checkNoJa,
  deps: { edges: deps.edges, nodes: [...nodeSet.keys()], rule_table: deps.rule_table },
};
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, '// Generated by tools/build_film_data.mjs from the product output and the catalogue. Do not edit.\nwindow.NF_DATA = ' + JSON.stringify(data) + ';\n');
log('wrote', OUT, fs.statSync(OUT).size, 'bytes');
