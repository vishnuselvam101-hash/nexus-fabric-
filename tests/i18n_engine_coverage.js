// Engine/server prose coverage: every English sentence a job screen can show must have a reviewed Japanese
// rendering (product rule: prose is translated only when EVERY sentence matches a known template; otherwise it
// is shown in English, marked as engine output). No browser: the JSON of every job is walked and checked against
// the client catalogue itself.
// Usage: NF_BASE=http://127.0.0.1:8793 [NF_USER=admin NF_PASS=...] node tests/i18n_engine_coverage.js [out.json]
// Exits 1 if any sentence has no Japanese rendering.
const fs = require('fs');
const path = require('path');

const B = (process.env.NF_BASE || 'http://127.0.0.1:8793').replace(/\/+$/, '');
const USER = process.env.NF_USER || 'admin';
const PASS = process.env.NF_PASS || 'correct-horse-9';
const OUT = process.argv[2];

// ---- session
const jar = new Map();
let cookie = '';
async function req(method, p, body) {
  const headers = { 'X-NF-Request': '1' };
  if (cookie) headers.cookie = cookie;
  if (body !== undefined) headers['content-type'] = 'application/json';
  const res = await fetch(B + p, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  const set = typeof res.headers.getSetCookie === 'function' ? res.headers.getSetCookie() : (res.headers.get('set-cookie') ? [res.headers.get('set-cookie')] : []);
  for (const c of set) {
    const pair = c.split(';')[0], i = pair.indexOf('=');
    if (i > 0) jar.set(pair.slice(0, i).trim(), pair.slice(i + 1).trim());
  }
  cookie = [...jar].map(([k, v]) => `${k}=${v}`).join('; ');
  const text = await res.text();
  let json = null; try { json = JSON.parse(text); } catch { /* not JSON */ }
  return { status: res.status, ok: res.ok, json, text };
}

// ---- every input set the legacy browser test uses (tests/ui_smoke.js), per family
const SURFACES = ['outside', 'inside', 'neutral', 'unknown'];
const HEADER = { header_radius: 150, header_dev_radius: 150, header_length: 1200, branch_station: 600, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400 };
const STUB = { header_radius: 150, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400 };
const PLAIN = {
  cylinder_shell: [{ r_dev: 150, length: 1200 }, { r_dev: 1, length: 1 }, { r_dev: 100, length: 1e9 }, { r_dev: 1e9, length: 1200 }],
  cone_frustum: [{ r_large_dev: 200, r_small_dev: 100, h: 300 }, { r_large_dev: 200, r_small_dev: 100, h: 301.25 }, { r_large_dev: 200, r_small_dev: 100, h: 150 }, { r_large_dev: 200, r_small_dev: 100, h: 1e-7 }],
  eccentric_reducer: [{ r_large_dev: 200, r_small_dev: 100, h: 400, offset: 60 }, { r_large_dev: 200, r_small_dev: 100, h: 400, offset: 0 }, { r_large_dev: 100, r_small_dev: 100, h: 300, offset: 0 }],
};
const SURFACED = { header_with_branch: HEADER, branch_stub: STUB };

function requests() {
  const out = [];
  for (const [family, sets] of Object.entries(PLAIN)) for (const s of sets) out.push({ family, body: { family, ...s } });
  for (const [family, base] of Object.entries(SURFACED)) {
    for (const surface_reference of SURFACES) out.push({ family, body: { family, ...base, surface_reference } });
    // the declared set-on joint, and the two pairings the engine refuses (legacy 14a)
    out.push({ family, body: { family, ...base, surface_reference: 'outside', branch_surface_reference: 'inside', joint: 'set_on' } });
    out.push({ family, body: { family, ...base, surface_reference: 'outside', branch_surface_reference: 'outside', joint: 'set_on' } });
    out.push({ family, body: { family, ...base, surface_reference: 'unknown', joint: 'set_on' } });
  }
  // refusal-producing and boundary inputs the legacy test posts or types
  out.push({ family: 'header_with_branch', body: { family: 'header_with_branch', ...HEADER, s_end: 150, surface_reference: 'unknown' } });
  out.push({ family: 'header_with_branch', body: { family: 'header_with_branch', ...HEADER, s_end: 0.5, surface_reference: 'unknown' } });
  out.push({ family: 'header_with_branch', body: { family: 'header_with_branch', ...HEADER, header_length: 900, branch_station: 450, surface_reference: 'outside' } });
  out.push({ family: 'header_with_branch', body: { family: 'header_with_branch', ...HEADER, branch_radius: 400, surface_reference: 'outside' } });
  out.push({ family: 'header_with_branch', body: { family: 'header_with_branch', ...HEADER, header_radius: -150, surface_reference: 'outside' } });
  out.push({ family: 'branch_stub', body: { family: 'branch_stub', header_radius: 150, branch_radius: 100, branch_dev_radius: 100, beta_deg: 179.997, delta: 0, s_end: 5.2e6, surface_reference: 'outside' } });
  return out;
}

// ---- prose detection: a sentence, not an identifier, number, date, file name, hash or user-entered text
const SKIP_KEYS = new Set(['id', 'record_id', 'hash', 'closure_hash', 'engine', 'name', 'key', 'label', 'from', 'to', 'rel', 'unit', 'path', 'url', 'href',
  'file', 'filename', 'created_at', 'updated_at', 'resolved_at', 'at', 'rule_table', 'created_by', 'resolved_by', 'run_by', 'username', 'reason', 'note', 'description']);
const ISO = /^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:?\d{2})?)?$/;
const HASH = /^[0-9a-f]{16,}$/i;
const FILE = /\.(pdf|dxf|png|jpe?g|tiff?|step|stp|json|txt|csv|svg)$/i;
const NUMISH = /^[\s\d.,+\-eE%°/]+$/;
const isProse = (s) => s.length >= 25 && /\s/.test(s.trim()) && /[A-Za-z]/.test(s) &&
  !ISO.test(s) && !HASH.test(s) && !FILE.test(s) && !NUMISH.test(s) && !/^https?:\/\//.test(s) && !/^\/api\//.test(s);

// Text that is written into the DXF file (plate annotations, comment lines, the not-released marking) stays English
// by design: the file is ASCII R12 read by cutting software. Engine/version strings are identifiers, not prose.
const FILE_TEXT = /(plate_convention_texts|\.marking\.|statement\.comments|running_engine|engine_version)/;
const DXF_UPPER = (s) => s === s.toUpperCase();

function walk(v, p, out) {
  if (typeof v === 'string') { if (isProse(v) && !FILE_TEXT.test(p) && !DXF_UPPER(v)) out.push({ path: p, text: v }); return; }
  if (Array.isArray(v)) { v.forEach((x, i) => walk(x, `${p}[${i}]`, out)); return; }
  if (v && typeof v === 'object') for (const [k, x] of Object.entries(v)) { if (!SKIP_KEYS.has(k)) walk(x, p ? `${p}.${k}` : k, out); }
}

// ---- the client catalogue, with the minimal globals it reads at load
function globals() {
  const root = { lang: 'en', dataset: {}, style: { setProperty() {} }, setAttribute(k, v) { this[k] = v; }, getAttribute(k) { return this[k] ?? null; } };
  const node = () => ({ style: { setProperty() {} }, dataset: {}, classList: { add() {}, remove() {}, toggle() {} }, setAttribute() {}, getAttribute() { return null; }, append() {}, appendChild() {}, addEventListener() {}, removeEventListener() {}, querySelector() { return null; }, querySelectorAll() { return []; } });
  const store = new Map();
  const def = (k, v) => Object.defineProperty(globalThis, k, { value: v, configurable: true, writable: true });
  def('window', globalThis);
  def('document', { documentElement: root, title: '', body: node(), head: node(), addEventListener() {}, removeEventListener() {}, querySelector() { return null; }, querySelectorAll() { return []; }, getElementById() { return null; }, createElement: node, createTextNode: node, createDocumentFragment: node });
  def('location', { search: '', hash: '', href: `${B}/`, origin: B, pathname: '/' });
  def('navigator', { languages: ['en-US', 'en'], language: 'en-US' });
  def('localStorage', { getItem: (k) => (store.has(k) ? store.get(k) : null), setItem: (k, v) => store.set(k, String(v)), removeItem: (k) => store.delete(k), clear: () => store.clear() });
  def('sessionStorage', globalThis.localStorage);
  def('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  def('requestAnimationFrame', (fn) => setTimeout(fn, 0));
}
const LABEL_KINDS = ['family', 'familyLong', 'part', 'param', 'quantity', 'gate', 'check', 'rkind', 'rfamily', 'sref', 'joint', 'rel', 'dlabel', 'state', 'result', 'obl', 'adv', 'dir'];

(async () => {
  const login = await req('POST', '/api/login', { username: USER, password: PASS });
  if (!login.ok) { console.error(`sign-in failed: ${login.status} ${login.text.slice(0, 200)}`); process.exit(1); }

  const projects = await req('GET', '/api/projects');
  let pid = Array.isArray(projects.json) && projects.json.length ? projects.json[0].id : null;
  if (pid === null) {
    const made = await req('POST', '/api/projects', { name: 'i18n coverage' });
    if (!made.ok || !made.json || made.json.id === undefined) { console.error(`project creation failed: ${made.status} ${made.text.slice(0, 200)}`); process.exit(1); }
    pid = made.json.id;
  }

  let jobs = 0, refusals = 0, errors = 0;
  const prose = [];
  for (const { family, body } of requests()) {
    const r = await req('POST', `/api/projects/${pid}/calculations`, body);
    const v = r.json || {};
    if (r.ok && v.id !== undefined) jobs++;
    else if (r.status === 422 || v.state === 'REFUSED' || v.code) refusals++;
    else { errors++; console.error(`unexpected answer for ${family}: ${r.status} ${r.text.slice(0, 160)}`); }
    if (!r.ok) walk(v, `refusal(${family}).${JSON.stringify(body).slice(0, 60)}`, prose);
    const id = v.id !== undefined ? v.id : v.calculation_id;
    if (id !== undefined && /^\d+$/.test(String(id))) {
      const job = await req('GET', `/api/calculations/${id}`);
      if (job.ok) walk(job.json, `job#${id}`, prose);
      else { errors++; console.error(`job ${id} not readable: ${job.status}`); }
    }
  }

  globals();
  const m = await import(path.resolve(__dirname, '../web/js/i18n.js'));
  const cache = new Map();
  const judge = (s) => {
    if (cache.has(s)) return cache.get(s);
    let verdict;
    if (m.engineJa(s)) verdict = { ok: true, bad: [] };
    else {
      const segs = m.engineSentences(s) || [];
      if (segs.length && segs.every((x) => x.ja)) verdict = { ok: true, bad: [] };
      else if (LABEL_KINDS.some((k) => m.idLabel(k, s))) verdict = { ok: true, bad: [] };
      else verdict = { ok: false, bad: segs.length ? segs.filter((x) => !x.ja).map((x) => x.en) : [s] };
    }
    cache.set(s, verdict);
    return verdict;
  };

  const missing = [], segments = new Map();
  let translated = 0;
  for (const { path: p, text } of prose) {
    const v = judge(text);
    if (v.ok) { translated++; continue; }
    missing.push({ path: p, text, untranslatedSegments: v.bad });
    for (const s of v.bad) {
      const e = segments.get(s) || { segment: s, count: 0, examplePath: p, exampleText: text };
      e.count++; segments.set(s, e);
    }
  }

  console.log(`jobs made: ${jobs}  refusals: ${refusals}  request errors: ${errors}`);
  console.log(`prose strings seen: ${prose.length}  translated: ${translated}  untranslated: ${missing.length}`);
  const distinct = [...segments.values()].sort((a, b) => b.count - a.count || a.segment.localeCompare(b.segment));
  for (const s of distinct) console.log(`UNTRANSLATED x${s.count}  ${JSON.stringify(s.segment)}  e.g. ${s.examplePath}`);
  console.log('----');
  console.log(`distinct untranslated segments: ${distinct.length}`);

  if (OUT) fs.writeFileSync(OUT, `${JSON.stringify({ base: B, project: pid, jobs, refusals, errors, seen: prose.length, translated, untranslated: missing.length, distinct, missing }, null, 1)}\n`);
  process.exit(distinct.length || errors ? 1 : 0);
})().catch((e) => { console.error('I18N COVERAGE FAILED', e && e.message ? e.message : e); process.exit(1); });
