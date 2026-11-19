// NEXUS FABRIC web client. No build step, no external resources (offline-capable).
// Every value shown comes from the API. Areas without a backend are labelled
// "not implemented" instead of showing placeholder data.

const $ = (s) => document.querySelector(s);
const esc = (v) => String(v ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fmt = (n) => (typeof n === 'number' ? (Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : esc(n));
const when = (t) => new Date(t * 1000).toLocaleString();
let me = null;

async function api(path, opts = {}) {
  const init = { method: opts.method || 'GET', headers: { 'X-NF-Request': '1' }, credentials: 'same-origin' }; // anti-forgery header required by the server
  if (opts.json !== undefined) { init.headers['Content-Type'] = 'application/json'; init.body = JSON.stringify(opts.json); }
  if (opts.body !== undefined) { init.body = opts.body; Object.assign(init.headers, opts.headers || {}); }
  const res = await fetch(path, init);
  let data = null;
  try { data = await res.json(); } catch { /* non-JSON */ }
  if (res.status === 401 && path !== '/api/login') { me = null; renderLogin(false); throw new Error('Please sign in.'); }
  if (!res.ok) throw new Error((data && data.error) || `Request failed (${res.status})`);
  return data;
}

const stateBadge = (s) => `<span class="badge ${s === 'APPROVED' ? 'b-ok' : s === 'REJECTED' ? 'b-bad' : 'b-warn'}">${esc(String(s).replace('_', ' '))}</span>`;
const resultBadge = (r) => `<span class="badge ${r === 'PASS' ? 'b-ok' : r === 'FAIL' ? 'b-bad' : r === 'INCONCLUSIVE' ? 'b-warn' : 'b-na'}">${esc(String(r).replace('_', ' '))}</span>`;
// Inline message instead of a blocking browser dialog.
function say(msg, kind = 'err') { const v = $('#view'); if (!v) return; let m = $('#flash'); if (!m) { m = document.createElement('div'); m.id = 'flash'; m.setAttribute('role', 'alert'); v.prepend(m); } m.className = kind; m.textContent = msg; m.scrollIntoView({ block: 'nearest' }); }
const notImpl = (title, items) => `<div class="ni"><b>${esc(title)} — not implemented in this build</b><ul class="tight">${items.map((i) => `<li>${esc(i)}</li>`).join('')}</ul></div>`;

const NAV = [
  ['dashboard', 'Dashboard'], ['projects', 'Projects & Documents'], ['workspace', 'Engineering Workspace'], ['amoeba', 'AMOEBA Control Centre'],
  ['math', 'Mathematical Engineering'], ['manufacturing', 'Manufacturing'], ['verification', 'Verification & Quality'], ['admin', 'Administration'],
];

function setTheme(t) { if (t) document.documentElement.dataset.theme = t; try { if (t) localStorage.setItem('nf-theme', t); } catch { /* ignore */ } }
try { setTheme(localStorage.getItem('nf-theme')); } catch { /* ignore */ }

function shell(route, html) {
  $('#app').innerHTML = `<a class="skip" href="#view">Skip to content</a><div class="shell"><aside><div class="brand">NEXUS FABRIC<small>AMOEBA-native engineering</small></div>
    <nav aria-label="Main">${NAV.map(([k, l]) => `<a href="#/${k}" class="${route === k ? 'act' : ''}" ${route === k ? 'aria-current="page"' : ''}>${l}</a>`).join('')}</nav></aside>
    <main><div class="top"><span class="notice">Development build — not validated — outputs must not be used for manufacturing</span>
      <span class="row"><a href="#/account">${esc(me.username)}</a><span class="sub">${esc(me.role)}</span><button class="sec" id="theme">Theme</button><button class="sec" id="logout">Sign out</button></span></div>
      <div class="content" id="view" tabindex="-1">${html}</div></main></div>`;
  $('#theme').onclick = () => setTheme(document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark');
  $('#logout').onclick = async () => { await api('/api/logout', { method: 'POST' }); me = null; renderLogin(false); };
}

function renderLogin(setup, error) {
  $('#app').innerHTML = `<div class="login"><div class="card"><h1>NEXUS FABRIC</h1>
    <p class="sub">${setup ? 'First start: create the administrator account.' : 'Sign in'}</p>
    <div class="info">Development build. Not validated. No output may be used for manufacturing.</div>
    ${error ? `<div class="err">${esc(error)}</div>` : ''}
    <form id="f"><label>Username</label><input name="u" autocomplete="username" required>
    <label>Password${setup ? ' (at least 10 characters)' : ''}</label><input name="p" type="password" autocomplete="${setup ? 'new-password' : 'current-password'}" required>
    <p><button>${setup ? 'Create administrator' : 'Sign in'}</button></p></form></div></div>`;
  $('#f').onsubmit = async (e) => {
    e.preventDefault();
    const body = { username: e.target.u.value, password: e.target.p.value };
    try {
      if (setup) await api('/api/setup', { method: 'POST', json: body });
      await api('/api/login', { method: 'POST', json: body });
      me = await api('/api/me');
      route();
    } catch (err) { renderLogin(setup, err.message); }
  };
}

const calcTable = (rows) => rows.length ? `<table><tr><th>#</th><th>Project</th><th>Family</th><th>State</th><th>Replay</th><th>By</th><th>When</th></tr>${rows.map((c) =>
  `<tr><td><a href="#/calc/${c.id}">${c.id}</a></td><td><a href="#/project/${c.project_id}">${c.project_id}</a></td><td>${esc(c.family)}</td><td>${c.superseded ? '<span class="badge b-na">SUPERSEDED</span>' : stateBadge(c.state)}${c.open_obligations ? ` <span class="badge b-bad">${c.open_obligations} open obligations</span>` : ''}</td><td>${c.replayed ? 'run' : '—'}</td><td>${esc(c.created_by)}</td><td>${when(c.created_at)}</td></tr>`).join('')}</table>`
  : '<p class="sub">No calculations yet.</p>';

const views = {
  async dashboard() {
    const d = await api('/api/dashboard');
    return `<h1>Dashboard</h1><p class="sub">Engine ${esc(d.engine)} · up ${Math.round(d.uptime_seconds / 60)} min</p>
      <div class="tiles"><div class="tile"><b>${d.projects}</b><span>Projects</span></div><div class="tile"><b>${d.documents}</b><span>Source documents</span></div>
      <div class="tile"><b>${d.calculations}</b><span>Engineering jobs</span></div><div class="tile"><b>${d.review_required}</b><span>Review required</span></div>
      <div class="tile"><b>${d.rejected}</b><span>Rejected</span></div><div class="tile"><b>${d.open_obligations}</b><span>Open scrutiny obligations</span></div><div class="tile"><b>${d.approved}</b><span>Approved</span></div><div class="tile"><b>${d.records}</b><span>Immutable records</span></div></div>
      <div class="card"><h2 style="margin-top:0">Recent engineering jobs</h2>${calcTable(d.recent)}</div>
      ${notImpl('Notifications, alerts, processing queue', ['Jobs run synchronously; there is no queue or notification service yet.'])}`;
  },
  async projects() {
    const ps = await api('/api/projects');
    after(() => { const f = $('#np'); if (f) f.onsubmit = async (e) => { e.preventDefault(); try { const r = await api('/api/projects', { method: 'POST', json: { name: e.target.n.value, description: e.target.d.value } }); location.hash = `#/project/${r.id}`; } catch (err) { say(err.message); } }; });
    return `<h1>Projects & Documents</h1><p class="sub">Projects, source documents and their immutable snapshots.</p>
      <div class="card">${ps.length ? `<table><tr><th>#</th><th>Name</th><th>Documents</th><th>Jobs</th><th>Created</th></tr>${ps.map((p) => `<tr><td>${p.id}</td><td><a href="#/project/${p.id}">${esc(p.name)}</a><br><span class="sub">${esc(p.description)}</span></td><td>${p.documents}</td><td>${p.calculations}</td><td>${esc(p.created_by)} · ${when(p.created_at)}</td></tr>`).join('')}</table>` : '<p class="sub">No projects yet.</p>'}</div>
      ${me.role === 'viewer' ? '' : `<div class="card"><h2 style="margin-top:0">New project</h2><form id="np"><div class="grid2"><div><label>Name</label><input name="n" required maxlength="200"></div><div><label>Description</label><input name="d"></div></div><p><button>Create project</button></p></form></div>`}
      ${notImpl('Revisions, version comparison, per-project permissions', ['Drawing revision chains and diffs', 'Per-project access control (all signed-in users can read all projects)'])}`;
  },
  async project(id) {
    const p = await api(`/api/projects/${id}`);
    after(() => {
      const mem = $('#mem'); if (mem) { const send = async (remove) => { try { await api(`/api/projects/${id}/members`, { method: 'POST', json: { username: mem.u.value, remove } }); route(); } catch (err) { $('#memmsg').innerHTML = `<div class="err">${esc(err.message)}</div>`; } }; mem.onsubmit = (e) => { e.preventDefault(); send(false); }; $('#memrm').onclick = () => send(true); }
      const up = $('#up'); if (!up) return;
      up.onsubmit = async (e) => {
        e.preventDefault(); const file = e.target.file.files[0]; if (!file) return;
        try { const r = await api(`/api/projects/${id}/documents`, { method: 'POST', body: file, headers: { 'X-Filename': encodeURIComponent(file.name).slice(0, 180) } }); $('#upmsg').innerHTML = `<div class="info">Stored (${esc(r.detected_type)}). Interpretation: ${esc(r.interpretation)}</div>`; setTimeout(route, 1200); }
        catch (err) { $('#upmsg').innerHTML = `<div class="err">${esc(err.message)}</div>`; }
      };
    });
    return `<h1>${esc(p.name)}</h1><p class="sub">${esc(p.description)} · created by ${esc(p.created_by)} · ${when(p.created_at)}</p>
      <div class="card"><h2 style="margin-top:0">Source documents</h2>
      ${p.documents.length ? `<table><tr><th>File</th><th>Detected type</th><th>Size</th><th>SHA-256</th><th>Uploaded</th></tr>${p.documents.map((d) => `<tr><td>${esc((() => { try { return decodeURIComponent(d.filename); } catch { return d.filename; } })())}${d.extension_mismatch ? ' <span class="badge b-warn">extension ≠ content</span>' : ''}</td><td>${esc(d.detected_type)}</td><td class="num">${d.size}</td><td class="mono">${esc(d.sha256)}</td><td>${esc(d.created_by)} · ${when(d.created_at)}</td></tr>`).join('')}</table>` : '<p class="sub">No documents.</p>'}
      <p class="sub">Current project snapshot: <span class="mono">${esc(p.snapshot || 'none')}</span></p>
      ${me.role === 'viewer' ? '' : `<form id="up" class="row"><input type="file" name="file" required style="max-width:420px"><button>Upload</button></form><div id="upmsg"></div>
      <p class="sub">Accepted by content: PDF, PNG, JPEG, TIFF, DXF, STEP (max 50 MB). Files are hashed and stored encrypted. <b>No drawing content is read yet.</b></p>`}</div>
      <div class="card"><h2 style="margin-top:0">Engineering jobs</h2>${calcTable(p.calculations)}<p><a class="btn" href="#/math/${p.id}">New development calculation</a></p></div>
      <div class="card"><h2 style="margin-top:0">Access</h2><p>Members: ${p.members.map(esc).join(', ') || 'none'}. Administrators can see every project; other users only the projects they are members of.</p>
      ${me.role === 'admin' ? `<form id="mem" class="row"><label class="vh" for="memu">Username</label><input id="memu" name="u" placeholder="username" required style="max-width:220px"><button>Add member</button><button class="sec" type="button" id="memrm">Remove member</button></form><div id="memmsg"></div>` : ''}</div>`;
  },
  async workspace() {
    return `<h1>Engineering Workspace</h1><p class="sub">Drawing viewing, interpretation and reconstruction.</p>
      ${notImpl('Drawing intelligence', ['PDF / image viewer with zoom, pan and measurement', 'PDF page classification, OCR (Japanese / English), dimension and symbol recognition', 'View classification, projection identification, scale and unit interpretation', 'Geometry reconstruction from drawings; 2D/3D comparison with the source', 'Human confirmation workflow for readings'])}
      <div class="info">Until drawing intelligence exists, geometry parameters are entered manually in <a href="#/math">Mathematical Engineering</a>. Such results are marked as not linked to a source drawing.</div>`;
  },
  async amoeba() {
    const a = await api('/api/amoeba/summary');
    const li = (xs) => `<ul class="tight">${xs.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>`;
    return `<h1>AMOEBA Control Centre</h1><p class="sub">Engineering Organism State — live contents of the append-only record store.</p>
      <div class="card"><h2 style="margin-top:0">Records by family and kind</h2>${a.records_by_kind.length ? `<table><tr><th>Family</th><th>Kind</th><th>Count</th></tr>${a.records_by_kind.map((r) => `<tr><td>${esc(r.family)}</td><td>${esc(r.kind)}</td><td class="num">${r.count}</td></tr>`).join('')}</table>` : '<p class="sub">No records yet.</p>'}</div>
      <div class="card"><h2 style="margin-top:0">Activities and declared logical work units</h2>${a.activities.length ? `<table><tr><th>Activity type</th><th>Distinct activities</th><th>Work units</th></tr>${a.activities.map((r) => `<tr><td>${esc(r.type)}</td><td class="num">${r.distinct_activities}</td><td class="num">${r.declared_work_units}</td></tr>`).join('')}</table><p class="sub">Identical activities on identical inputs are one record (content addressing), so repeated identical jobs do not add rows.</p>` : '<p class="sub">No activities yet.</p>'}</div>
      <div class="card"><h2 style="margin-top:0">Recent project snapshots</h2>${a.recent_snapshots.length ? `<table><tr><th>Project</th><th>Snapshot id</th></tr>${a.recent_snapshots.map((s) => `<tr><td>${s.project_id}</td><td class="mono">${esc(s.snapshot)}</td></tr>`).join('')}</table>` : '<p class="sub">No snapshots yet.</p>'}</div>
      <div class="card"><h2 style="margin-top:0">Scrutiny obligations</h2><p>${a.obligations.open} open of ${a.obligations.total} recorded. <a href="#/verification">Verification &amp; Quality</a> lists them.</p></div>
      <div class="card"><h2 style="margin-top:0">Mechanisms in this build</h2><div class="grid2"><div><b>Implemented</b>${li(a.implemented)}</div><div><b>Not implemented</b>${li(a.not_implemented)}</div></div></div>`;
  },
  async math(projectId) {
    const ps = await api('/api/projects');
    if (!ps.length) return `<h1>Mathematical Engineering</h1><div class="info">Create a project first: <a href="#/projects">Projects</a>.</div>`;
    const F = {
      cylinder_shell: { label: 'Cylindrical shell', f: [['r_dev', 'Development radius (mm)', 500], ['length', 'Length (mm)', 1000]] },
      cone_frustum: { label: 'Concentric reducer (right cone frustum)', f: [['r1', 'Large-end radius (mm)', 200], ['r2', 'Small-end radius (mm)', 100], ['h', 'Axial height (mm)', 300]] },
      eccentric_reducer: { label: 'Eccentric reducer (oblique cone frustum)', f: [['r1', 'Large-end radius (mm)', 200], ['r2', 'Small-end radius (mm)', 100], ['h', 'Axial height (mm)', 300], ['e', 'Offset of small end (mm)', 100]] },
      header_with_branch: { label: 'Assembly: header shell with set-on branch (two parts, one joint)', f: [['header_radius', 'Header contact radius (mm)', 150], ['header_dev_radius', 'Header development radius (mm)', 150], ['header_length', 'Header shell length (mm)', 1200], ['branch_station', 'Branch position along header (mm)', 600], ['branch_radius', 'Branch contact radius (mm)', 100], ['branch_dev_radius', 'Branch development radius (mm)', 100], ['beta_deg', 'Angle between axes (deg)', 60], ['delta', 'Axis offset (mm)', 0], ['s_end', 'Branch square end, distance along branch axis (mm)', 400]], sref: true },
      branch_stub: { label: 'Branch stub on cylindrical header', f: [['header_radius', 'Header contact radius (mm)', 150], ['branch_radius', 'Branch contact radius (mm)', 100], ['branch_dev_radius', 'Branch development radius (mm)', 100], ['beta_deg', 'Angle between axes (deg)', 60], ['delta', 'Axis offset (mm)', 0], ['s_end', 'Square end, distance along branch axis (mm)', 400]] },
    };
    const fields = (k) => F[k].f.map(([n, l, v]) => `<div><label for="f_${n}">${l}</label><input id="f_${n}" name="${n}" type="number" step="any" value="${v}" required></div>`).join('')
      + (F[k].sref ? `<div><label for="f_sref">Entered radii refer to which wall surface?</label><select id="f_sref" name="surface_reference"><option value="unknown">unknown — to be established</option><option>outside</option><option>inside</option><option>neutral</option></select></div>` : '');
    after(() => {
      const form = $('#calc'); if (!form) return;
      form.fam.onchange = () => { $('#fields').innerHTML = fields(form.fam.value); };
      form.onsubmit = async (e) => {
        e.preventDefault(); const fam = form.fam.value; const req = { family: fam };
        for (const [n] of F[fam].f) req[n] = Number(form[n].value);
        if (F[fam].sref) req.surface_reference = form.surface_reference.value;
        try { const r = await api(`/api/projects/${form.project.value}/calculations`, { method: 'POST', json: req }); location.hash = `#/calc/${r.id}`; }
        catch (err) { $('#cmsg').innerHTML = `<div class="err">${esc(err.message)}</div>`; }
      };
    });
    return `<h1>Mathematical Engineering</h1><p class="sub">Surface development with independent verification. Parameters are entered manually.</p>
      <div class="info">Radii are used as reference-surface radii exactly as entered. Thickness, allowances, weld gap and kerf are <b>not</b> modelled. Mathematical models are provisional and not independently reviewed. Pre-filled numbers are example values, not recommendations.</div>
      ${me.role === 'viewer' ? '<div class="info">Your role can view results but not run calculations.</div>' : `<div class="card"><form id="calc"><div class="grid2"><div><label>Project</label><select name="project">${ps.map((p) => `<option value="${p.id}" ${String(p.id) === String(projectId) ? 'selected' : ''}>#${p.id} ${esc(p.name)}</option>`).join('')}</select></div>
      <div><label>Geometry family</label><select name="fam">${Object.entries(F).map(([k, v]) => `<option value="${k}">${v.label}</option>`).join('')}</select></div></div>
      <div class="grid2" id="fields">${fields('cylinder_shell')}</div><p><button>Develop, verify and generate DXF</button></p><div id="cmsg"></div></form></div>`}
      ${notImpl('Further mathematical capabilities', ['Constraint solving from drawing dimensions', 'Assemblies other than one header with one branch; several branches; flanges and supports', 'Rectangular-to-round and irregular transitions; equal-diameter and partial branch intersections; oblique cylinders; near-cylindrical cones', 'Thickness / neutral-surface treatment, bend and roll allowances from calibrated process data', 'Uncertainty and tolerance analysis; interval-certified error bounds', 'Plate division and seam optimisation'])}`;
  },
  async calc(id) {
    const [c, dep] = await Promise.all([api(`/api/calculations/${id}`), api(`/api/calculations/${id}/dependencies`)]);
    const canEdit = me.role !== 'viewer';
    after(() => {
      const b = $('#replay'); if (b) b.onclick = async () => { b.disabled = true; try { await api(`/api/calculations/${id}/replay`, { method: 'POST' }); route(); } catch (err) { say(err.message); b.disabled = false; } };
      const f = $('#corr'); if (f) f.onsubmit = async (e) => {
        e.preventDefault();
        try { const r = await api(`/api/calculations/${id}/corrections`, { method: 'POST', json: { parameter: f.p.value, new_value: Number(f.v.value), reason: f.r.value } }); location.hash = `#/calc/${r.new_calculation}`; }
        catch (err) { $('#corrmsg').innerHTML = `<div class="err">${esc(err.message)}</div>`; }
      };
      document.querySelectorAll('[data-ack]').forEach((btn) => { btn.onclick = async () => {
        const input = document.querySelector(`[data-advreason="${btn.dataset.ack}"]`); const reason = input.value.trim();
        if (reason.length < 5) { input.focus(); input.setAttribute('aria-invalid', 'true'); say('Enter why the value is right this time (recorded with your name).'); return; }
        try { await api(`/api/advisories/${btn.dataset.ack}/acknowledge`, { method: 'POST', json: { reason } }); route(); } catch (err) { say(err.message); }
      }; });
      document.querySelectorAll('[data-resolve]').forEach((btn) => { btn.onclick = async () => {
        const input = document.querySelector(`[data-reason="${btn.dataset.resolve}"]`);
        const reason = input.value.trim();
        if (reason.length < 5) { input.focus(); input.setAttribute('aria-invalid', 'true'); say('Enter what you checked (the reason is recorded with your name).'); return; }
        try { await api(`/api/obligations/${btn.dataset.resolve}/resolve`, { method: 'POST', json: { decision: btn.dataset.decision, reason } }); route(); } catch (err) { say(err.message); }
      }; });
    });
    const drawing = (p) => {
      const all = [...p.curves, ...p.holes];
      const pts = all.flatMap((k) => k.points);
      const xs = pts.map((q) => q[0]), ys = pts.map((q) => q[1]);
      const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
      const w = x1 - x0 || 1, h = y1 - y0 || 1, m = 0.04 * Math.max(w, h);
      return `<svg class="pattern" viewBox="${x0 - m} ${-(y1 + m)} ${w + 2 * m} ${h + 2 * m}" role="img" aria-label="Flat pattern of ${esc(p.part)}">${all.map((k) =>
        `<polyline fill="none" stroke="${k.layer === 'CUT_INNER' ? 'var(--warn)' : 'var(--accent)'}" stroke-width="${Math.max(w, h) / 420}" stroke-linejoin="round" points="${k.points.map((q) => `${q[0]},${-q[1]}`).join(' ')}"><title>${esc(k.name)} (${esc(k.kind)}, ${esc(k.layer)}), length ${fmt(k.length)} mm</title></polyline>`).join('')}</svg>`;
    };
    const rp = c.replay;
    const blocked = c.superseded_by || c.state === 'REJECTED';
    const dirLabel = { downstream: 'Downstream (derived from it)', lateral: 'Lateral (must still fit)', common_cause: 'Common cause (same convention)' };
    const oblState = (s) => `<span class="badge ${s === 'open' ? 'b-bad' : s === 'waived' ? 'b-warn' : 'b-ok'}">${esc(s.replaceAll('_', ' '))}</span>`;
    return `<h1>Engineering job #${c.id} ${stateBadge(c.state)}</h1>
      <p class="sub"><a href="#/project/${c.project_id}">Project ${c.project_id}</a> · ${esc(c.request.family)} · entered by ${esc(c.created_by)} · engine ${esc(c.engine)}
        ${c.supersedes ? ` · corrects <a href="#/calc/${c.supersedes}">job #${c.supersedes}</a>` : ''}</p>
      ${c.superseded_by ? `<div class="err">SUPERSEDED by <a href="#/calc/${c.superseded_by}">job #${c.superseded_by}</a> after a correction. This result must not be used; its files are withheld.</div>`
        : c.state === 'REJECTED' ? '<div class="err">REJECTED: a critical gate failed. No output of this job may be used.</div>'
        : '<div class="info">REVIEW REQUIRED: this result cannot be released for manufacturing. See the gate table for every reason.</div>'}
      ${c.outcome ? `<div class="err">No pattern produced: ${esc(c.outcome)}</div>` : ''}
      <div class="card"><h2 style="margin-top:0">Request (entered manually)</h2><table>${Object.entries(c.request).map(([k, v]) => `<tr><td>${esc(k)}</td><td class="num">${fmt(v)}</td></tr>`).join('')}</table></div>
      ${c.patterns.map((p) => `<div class="card"><h2 style="margin-top:0">Flat pattern: ${esc(p.part)} — class ${esc(p.class)} (proposed naming)</h2>${drawing(p)}
        <div class="row" style="margin-top:10px">${blocked ? '<span class="sub">DXF withheld.</span>' : `<a class="btn" href="/api/calculations/${c.id}/dxf/${encodeURIComponent(p.part)}">Download DXF (not released)</a>`}<span class="sub">Numerical error (estimated): ${fmt(p.eps_num)} mm · curve representation error (estimated): ${fmt(p.eps_rep)} mm${p.holes.length ? ' · opening shown in amber (layer CUT_INNER)' : ''}</span></div>
        <h2>Quantities</h2><table>${Object.entries(p.quantities).map(([k, v]) => `<tr><td>${esc(k)}</td><td class="num">${fmt(v)}</td></tr>`).join('')}</table>
        <h2>Curves</h2><table><tr><th>Curve</th><th>Entity</th><th>Layer</th><th>Length (mm)</th></tr>${[...p.curves, ...p.holes].map((k) => `<tr><td>${esc(k.name)}</td><td>${esc(k.kind)}</td><td>${esc(k.layer)}</td><td class="num">${fmt(k.length)}</td></tr>`).join('')}</table>
        <h2>Assumptions</h2><ul class="tight">${p.assumptions.map((a) => `<li>${esc(a)}</li>`).join('')}</ul></div>`).join('')}
      ${c.checks.length ? `<div class="card"><h2 style="margin-top:0">Independent verification</h2><table><tr><th>Scope</th><th>Check</th><th>Reference method</th><th>Primary</th><th>Reference</th><th>Difference</th><th>Tolerance</th><th></th></tr>${c.checks.map((k) =>
        `<tr><td>${esc(k.scope)}</td><td>${esc(k.name)}<br><span class="sub">${esc(k.independence)}</span></td><td>${esc(k.method)}</td><td class="num">${fmt(k.primary)}</td><td class="num">${fmt(k.reference)}</td><td class="num">${fmt(k.difference)}</td><td class="num">${fmt(k.tolerance)}</td><td>${resultBadge(k.verdict)}</td></tr>`).join('')}</table></div>` : ''}
      ${c.advisories.length ? `<div class="card"><h2 style="margin-top:0">Known-mistake check</h2>
        <p class="sub">This job was compared with the lessons recorded in this project (earlier corrections and failures). A match never changes a value; it asks an engineer to look.</p>
        <table><tr><th>#</th><th>What was seen before</th><th>State</th><th></th></tr>${c.advisories.map((a) => `<tr><td>${a.id}</td><td>${esc(a.message)}${a.note ? `<br><span class="sub">${esc(a.resolved_by)} · ${when(a.resolved_at)}: ${esc(a.note)}</span>` : ''}</td>
          <td><span class="badge ${a.state === 'open' ? 'b-bad' : 'b-ok'}">${esc(a.state)}</span></td>
          <td>${a.state === 'open' && canEdit ? `<label class="vh" for="adv${a.id}">Reason for advisory ${a.id}</label><input id="adv${a.id}" data-advreason="${a.id}" placeholder="Why is it right this time?" style="min-width:210px"><div class="row" style="margin-top:6px"><button class="sec" data-ack="${a.id}">Acknowledge</button></div>` : ''}</td></tr>`).join('')}</table></div>` : ''}
      ${c.obligations.length ? `<div class="card"><h2 style="margin-top:0">Scrutiny obligations from the correction</h2>
        <p class="sub">Created by following the dependency edges of the corrected job. Rule table: ${esc(dep.rule_table)}. Open obligations cannot be closed by time or budget.</p>
        <table><tr><th>#</th><th>Direction</th><th>Affected</th><th>Required check</th><th>State</th><th>Evidence / decision</th><th></th></tr>${c.obligations.slice().reverse().map((o) =>
        `<tr><td>${o.id}</td><td>${esc(dirLabel[o.direction] || o.direction)}<br><span class="sub">via ${esc(o.via)}</span></td><td>${esc(o.kind)} · ${esc(o.label)}</td><td>${esc(o.required_check)}</td><td>${oblState(o.state)}</td><td>${esc(o.note)}${o.resolved_by ? `<br><span class="sub">${esc(o.resolved_by)} · ${when(o.resolved_at)}</span>` : ''}</td>
         <td>${o.state === 'open' && canEdit ? `<label class="vh" for="reason${o.id}">Reason for obligation ${o.id}</label><input id="reason${o.id}" data-reason="${o.id}" placeholder="What did you check?" style="min-width:190px"><div class="row" style="margin-top:6px"><button class="sec" data-resolve="${o.id}" data-decision="confirmed">Confirm checked</button><button class="sec" data-resolve="${o.id}" data-decision="waived">Waive</button></div>` : ''}</td></tr>`).join('')}</table></div>` : ''}
      ${c.correctable_parameters.length && !c.superseded_by && canEdit ? `<div class="card"><h2 style="margin-top:0">Record a correction</h2>
        <p class="sub">A correction never edits this job. It creates a new job, marks this one superseded, and raises an obligation for every record that depends on the corrected value, mates with something affected, or was entered under the same convention.</p>
        <form id="corr"><div class="grid2"><div><label>Parameter</label><select name="p">${c.correctable_parameters.map(([k, v]) => `<option value="${esc(k)}">${esc(k)} (now ${fmt(v)})</option>`).join('')}</select></div>
        <div><label>Corrected value</label><input name="v" type="number" step="any" required></div><div><label>Reason (required)</label><input name="r" required minlength="5" placeholder="e.g. drawing dimension is inside diameter"></div></div>
        <p><button>Record correction and propagate</button></p><div id="corrmsg"></div></form></div>` : ''}
      <div class="card"><h2 style="margin-top:0">Release gates</h2><table><tr><th>Gate</th><th>Result</th><th>Detail</th></tr>${c.gates.map((g) => `<tr><td>${esc(g.id)} ${esc(g.name)}</td><td>${resultBadge(g.result)}</td><td>${esc(g.detail)}</td></tr>`).join('')}</table></div>
      ${dep.edges.length ? `<div class="card"><h2 style="margin-top:0">Dependency edges</h2><p class="sub">“From” depends on “to”. A change to a record on the right affects everything that reaches it on the left.</p>
        <table><tr><th>From</th><th>Edge</th><th>To</th></tr>${dep.edges.map((e) => `<tr><td>${esc(e.from)}</td><td class="mono">${esc(e.rel)}</td><td>${esc(e.to)}</td></tr>`).join('')}</table></div>` : ''}
      <div class="card"><h2 style="margin-top:0">Provenance and replay</h2>
        <p>Release closure: <span class="mono">${esc(c.closure_hash)}</span> (${c.closure_size} records, ${c.work_units} declared work units)</p>
        ${rp ? `<p>Replay audit: ${resultBadge(rp.matches ? 'PASS' : 'FAIL')} ${when(rp.at)} by ${esc(rp.run_by)} — ${rp.stored_records_checked} stored records checked, ${rp.stored_records_failing_identity_check} failing.</p><p class="sub">${esc(rp.limitation)}</p>` : '<p class="sub">No replay audit has been run.</p>'}
        ${canEdit ? '<button id="replay" class="sec">Run replay audit</button>' : ''}
        <h2>Records</h2><table><tr><th>Family</th><th>Kind</th><th>Id</th></tr>${c.records.map((r) => `<tr><td>${esc(r.family)}</td><td>${esc(r.kind)}</td><td class="mono">${esc(r.id)}</td></tr>`).join('')}</table></div>`;
  },
  async manufacturing() {
    const cs = (await api('/api/calculations')).filter((c) => c.state !== 'REJECTED');
    return `<h1>Manufacturing</h1><p class="sub">DXF outputs produced by engineering jobs. None is released.</p>
      <div class="card">${calcTable(cs)}<p class="sub">Open a job to inspect its pattern and download its DXF. File names carry the prefix NOT-RELEASED and the job state.</p></div>
      ${notImpl('Manufacturing planning', ['Plate division and stock fitting', 'Seam and weld planning', 'Allowance management (requires calibrated process data)', 'Manufacturing feasibility checks against machine limits', 'Bend-line, marking and inner-profile layers; kerf handling', 'CAM system acceptance testing'])}`;
  },
  async verification() {
    const [cs, obs, lessons] = await Promise.all([api('/api/calculations'), api('/api/obligations'), api('/api/lessons')]);
    const open = obs.filter((o) => o.state === 'open');
    return `<h1>Verification & Quality</h1><p class="sub">Release state of every engineering job, and scrutiny obligations raised by corrections.</p>
      <div class="card"><h2 style="margin-top:0">Open scrutiny obligations (${open.length})</h2>${open.length ? `<table><tr><th>#</th><th>Job</th><th>Affected</th><th>Required check</th><th>Raised</th></tr>${open.map((o) => `<tr><td>${o.id}</td><td><a href="#/calc/${o.target_calc}">#${o.target_calc}</a> (corrects #${o.origin_calc})</td><td>${esc(o.kind)} · ${esc(o.label)}</td><td>${esc(o.required_check)}</td><td>${when(o.created_at)}</td></tr>`).join('')}</table><p class="sub">Open a job to confirm or waive an obligation; every decision is recorded with its reason.</p>` : '<p class="sub">None open.</p>'}
        <p class="sub">${obs.length} obligations recorded in total. The rule table that creates them is provisional and unreviewed.</p></div>
      <div class="card"><h2 style="margin-top:0">Lessons learned (${lessons.length})</h2>
        <p class="sub">Recorded automatically when an engineer corrects a value, when verification rejects a result, or when a stored result fails its integrity check. Every later job in the same project is compared with them.</p>
        ${lessons.length ? `<table><tr><th>#</th><th>Project</th><th>What went wrong</th><th>From job</th><th>Matched since</th><th>Recorded</th></tr>${lessons.map((l) => `<tr><td>${l.id}</td><td>${l.project_id}</td>
          <td>${l.kind === 'corrected_value' ? `${esc(l.family)} · <b>${esc(l.parameter)}</b>: ${fmt(l.wrong_value)} was wrong, ${fmt(l.right_value)} is right<br><span class="sub">${esc(l.detail)}</span>` : `${esc(l.kind.replace('_', ' '))} · ${esc(l.family)}`}</td>
          <td><a href="#/calc/${l.source_calc}">#${l.source_calc}</a></td><td class="num">${l.times_matched}</td><td>${esc(l.created_by)} · ${when(l.created_at)}</td></tr>`).join('')}</table>` : '<p class="sub">No lessons recorded yet.</p>'}</div>
      <div class="card"><h2 style="margin-top:0">Engineering jobs</h2>${calcTable(cs)}</div>
      ${notImpl('Quality workflow', ['Engineer approval — no job can reach APPROVED in this build', 'Findings by severity (critical / major / minor) and investigations', 'Self-consistency audits A1–A4; replay on a separate machine or build', 'Obligations from drawing revisions and from verification failures (only parameter corrections raise them today)'])}`;
  },
  async account() {
    after(() => { const f = $('#pw'); f.onsubmit = async (e) => { e.preventDefault();
      if (f.n.value !== f.n2.value) { $('#pwmsg').innerHTML = '<div class="err">The two new passwords differ.</div>'; return; }
      try { await api('/api/password', { method: 'POST', json: { current: f.c.value, new: f.n.value } }); me = null; renderLogin(false, 'Password changed. All sessions were signed out; sign in again.'); }
      catch (err) { $('#pwmsg').innerHTML = `<div class="err">${esc(err.message)}</div>`; } }; });
    return `<h1>Account</h1><p class="sub">${esc(me.username)} · ${esc(me.role)}</p>
      <div class="card"><h2 style="margin-top:0">Change password</h2><form id="pw"><div class="grid2"><div><label for="pwc">Current password</label><input id="pwc" name="c" type="password" autocomplete="current-password" required></div>
      <div><label for="pwn">New password (at least 10 characters)</label><input id="pwn" name="n" type="password" autocomplete="new-password" minlength="10" required></div>
      <div><label for="pwn2">Repeat new password</label><input id="pwn2" name="n2" type="password" autocomplete="new-password" minlength="10" required></div></div>
      <p><button>Change password</button></p><div id="pwmsg"></div><p class="sub">Changing the password signs out every session of this account.</p></form></div>
      ${notImpl('Account security', ['Multi-factor authentication', 'Account recovery (an administrator must create a new account)', 'List and revoke individual sessions'])}`;
  },
  async admin() {
    if (me.role !== 'admin') return `<h1>Administration</h1><div class="info">Administrator role required.</div>`;
    const [users, audit, sec, chain] = await Promise.all([api('/api/users'), api('/api/audit'), api('/api/security/status'), api('/api/audit/verify')]);
    after(() => {
      const f = $('#nu'); f.onsubmit = async (e) => { e.preventDefault(); try { await api('/api/users', { method: 'POST', json: { username: e.target.u.value, password: e.target.p.value, role: e.target.r.value } }); route(); } catch (err) { $('#umsg').innerHTML = `<div class="err">${esc(err.message)}</div>`; } };
      document.querySelectorAll('[data-user]').forEach((b) => { b.onclick = async () => { try { await api(`/api/users/${b.dataset.user}/active`, { method: 'POST', json: { active: b.dataset.to === '1' } }); route(); } catch (err) { say(err.message); } }; });
    });
    const yn = (v) => (v === true ? '<span class="badge b-ok">yes</span>' : v === false ? '<span class="badge b-bad">no</span>' : esc(v));
    return `<h1>Administration</h1>
      <div class="card"><h2 style="margin-top:0">Users</h2><table><tr><th>#</th><th>Username</th><th>Role</th><th>Status</th><th>Created</th><th></th></tr>${users.map((u) => `<tr><td>${u.id}</td><td>${esc(u.username)}</td><td>${esc(u.role)}</td><td>${u.active ? 'active' : '<span class="badge b-na">deactivated</span>'}</td><td>${when(u.created_at)}</td>
        <td>${u.id === me.id ? '' : `<button class="sec" data-user="${u.id}" data-to="${u.active ? 0 : 1}">${u.active ? 'Deactivate' : 'Reactivate'}</button>`}</td></tr>`).join('')}</table>
      <form id="nu"><div class="grid2"><div><label for="nuu">Username</label><input id="nuu" name="u" required></div><div><label for="nup">Password (min 10)</label><input id="nup" name="p" type="password" autocomplete="new-password" required></div><div><label for="nur">Role</label><select id="nur" name="r"><option>engineer</option><option>viewer</option><option>admin</option></select></div></div><p><button>Add user</button></p><div id="umsg"></div></form>
      <p class="sub">Project access is granted per project, on the project page.</p></div>
      <div class="card"><h2 style="margin-top:0">Security configuration (as running)</h2><table>${Object.entries(sec).map(([k, v]) => `<tr><td>${esc(k.replaceAll('_', ' '))}</td><td>${yn(v)}</td></tr>`).join('')}</table></div>
      <div class="card"><h2 style="margin-top:0">Audit log</h2>
        <p>Chain check: ${chain.intact ? '<span class="badge b-ok">INTACT</span>' : `<span class="badge b-bad">BROKEN at entry ${esc(chain.first_broken_entry)}</span>`} over ${chain.entries} entries. <span class="sub">${esc(chain.limitation)}</span></p>
        <table><tr><th>When</th><th>User</th><th>Action</th><th>Detail</th></tr>${audit.map((a) => `<tr><td>${when(a.at)}</td><td>${esc(a.username)}</td><td>${esc(a.action)}</td><td>${esc(a.detail)}</td></tr>`).join('')}</table></div>
      ${notImpl('Administration', ['Multi-factor authentication, single sign-on', 'Built-in TLS, database encryption at rest, key rotation', 'Backup and restore tooling, retention policies', 'AI/model configuration, engineering standards and factory rule management', 'Separate tenants (isolation is per project within one installation)'])}`;
  },
};

let renderSeq = 0;
let pendingWire = null;
// A view registers the function that attaches its event handlers; it runs only if that view is the one shown.
function after(fn) { pendingWire = fn; }
async function route() {
  if (!me) return;
  const seq = ++renderSeq; // a slower, older navigation must never overwrite a newer one
  const [, name = 'dashboard', arg] = location.hash.split('/');
  const view = views[name] || views.dashboard;
  const navKey = name === 'project' ? 'projects' : name === 'calc' ? 'verification' : name;
  const titles = Object.fromEntries(NAV); document.title = `${titles[navKey] || 'Account'} — NEXUS FABRIC`;
  let html;
  try { html = await view(arg); } catch (err) { html = `<div class="err">${esc(err.message)}</div>`; }
  const wire = pendingWire; pendingWire = null;
  if (seq === renderSeq && me) { shell(navKey, html); if (wire) wire(); const v = $('#view'); if (v && location.hash) v.focus({ preventScroll: true }); }
}

window.addEventListener('hashchange', route);
(async () => {
  try {
    const h = await api('/api/health');
    if (h.setup_required) return renderLogin(true);
    try { me = await api('/api/me'); route(); } catch { /* login shown by api() */ }
  } catch (err) { $('#app').innerHTML = `<p class="pad err">Cannot reach the server: ${esc(err.message)}</p>`; }
})();
