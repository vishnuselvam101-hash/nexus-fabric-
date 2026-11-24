// Seeds a freshly started server (EMPTY data directory) with demonstration data through the real API.
// Usage: NF_BASE=http://127.0.0.1:8787 node docs/design/tools/seed_demo.js
// Accounts created: admin / correct-horse-9 (admin), engineer1 / engineer-pass-1 (engineer), viewer1 / read-only-pass (viewer)
const B = process.env.NF_BASE || 'http://127.0.0.1:8787';
let cookie = '';
async function call(path, json, extra) {
  const init = { method: json !== undefined || extra ? 'POST' : 'GET', headers: { 'X-NF-Request': '1', Cookie: cookie } };
  if (json !== undefined) { init.headers['Content-Type'] = 'application/json'; init.body = JSON.stringify(json); }
  if (extra) { init.body = extra.body; Object.assign(init.headers, extra.headers || {}); }
  const r = await fetch(B + path, init);
  const sc = r.headers.getSetCookie ? r.headers.getSetCookie() : [];
  if (sc.length) cookie = sc.map((c) => c.split(';')[0]).join('; ');
  let d = null; try { d = await r.json(); } catch { /* none */ }
  if (r.status === 422 && d && d.state === 'REFUSED' && d.id) return d; // a refused request is kept as a job without a result
  if (!r.ok) throw new Error(`${path}: ${r.status} ${d && d.error}`);
  return d;
}
(async () => {
  const cred = { username: 'admin', password: 'correct-horse-9' };
  // first-run setup needs the installation's one-time setup token: start the server with NF_SETUP_TOKEN and pass the same value here
  await call('/api/setup', { ...cred, setup_token: process.env.NF_SETUP_TOKEN || '' }); await call('/api/login', cred);
  const p1 = (await call('/api/projects', { name: 'Header H-204 spool study', description: 'Steam header with set-on branch, reducers' })).id;
  const p2 = (await call('/api/projects', { name: 'Reducer family check', description: 'Concentric and eccentric reducers' })).id;
  await call(`/api/projects/${p1}/documents`, undefined, { body: '%PDF-1.7\ndemonstration file, not a drawing\n', headers: { 'X-Filename': 'H-204-general-arrangement.pdf' } });
  const run = async (p, req) => (await call(`/api/projects/${p}/calculations`, req)).id;
  const cone = await run(p2, { family: 'cone_frustum', r1: 200, r2: 100, h: 300 });
  const ecc = await run(p2, { family: 'eccentric_reducer', r1: 200, r2: 100, h: 300, e: 100 });
  await run(p1, { family: 'branch_stub', header_radius: 150, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400 });
  await run(p1, { family: 'cylinder_shell', r_dev: 500, length: 1000 });
  await run(p2, { family: 'cone_frustum', r1: 200, r2: 199.99, h: 300 }); // unsupported input: no pattern
  const hb = { family: 'header_with_branch', header_radius: 150, header_dev_radius: 150, header_length: 1200, branch_station: 600, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400, surface_reference: 'outside' };
  const a1 = await run(p1, hb);
  const cr = await call(`/api/calculations/${a1}/corrections`, { parameter: 'header_radius', new_value: 158, reason: 'drawing dimension is the outside diameter of the thicker pipe' });
  const a2 = cr.new_calculation;
  const open = (await call(`/api/calculations/${a2}`)).obligations.filter((o) => o.state === 'open');
  await call(`/api/obligations/${open[0].id}/resolve`, { decision: 'confirmed', reason: 'checked against the drawing: outside' });
  await call(`/api/calculations/${ecc}/replay`, {});
  await call(`/api/calculations/${a2}/replay`, {});
  const a3 = await run(p1, { ...hb, header_length: 900, branch_station: 450, branch_radius: 90, branch_dev_radius: 90, beta_deg: 75 }); // repeats the corrected value: advisory
  await call('/api/users', { username: 'engineer1', password: 'engineer-pass-1', role: 'engineer' });
  await call('/api/users', { username: 'viewer1', password: 'read-only-pass', role: 'viewer' });
  await call(`/api/projects/${p1}/members`, { username: 'engineer1', remove: false });
  await call(`/api/projects/${p1}/members`, { username: 'viewer1', remove: false });
  console.log(JSON.stringify({ projects: [p1, p2], cone, ecc, assembly_superseded: a1, assembly_corrected: a2, assembly_advisory: a3 }));
})().catch((e) => { console.error('SEED FAILED', e.message); process.exit(1); });
