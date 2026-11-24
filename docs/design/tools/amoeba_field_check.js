// Field check: every field that web/js/views/amoeba.js reads must be in the server's JSON.
// Usage: NF_BASE=http://127.0.0.1:8795 node docs/design/tools/amoeba_field_check.js   (server seeded by seed_demo.js)
'use strict';
const B = (process.env.NF_BASE || 'http://127.0.0.1:8795').replace(/\/$/, '');
const H = { 'X-NF-Request': '1', 'Content-Type': 'application/json' };
let cookie = '';
async function get(p) {
  const r = await fetch(B + p, { headers: { ...H, Cookie: cookie } });
  const sc = r.headers.getSetCookie ? r.headers.getSetCookie() : [];
  if (sc.length) cookie = sc.map((c) => c.split(';')[0]).join('; ');
  if (!r.ok) throw new Error(`${p} -> ${r.status}`);
  return r.json();
}
const need = (name, obj, keys) => {
  const miss = keys.filter((k) => !(obj && k in obj));
  if (miss.length) { fails.push(`${name}: missing ${miss.join(', ')}`); }
  else console.log(`ok  - ${name}`);
};
const fails = [];
(async () => {
  const login = await fetch(B + '/api/login', { method: 'POST', headers: H, body: JSON.stringify({ username: 'admin', password: 'correct-horse-9' }) });
  if (!login.ok) throw new Error('login ' + login.status);
  const sc = login.headers.getSetCookie ? login.headers.getSetCookie() : [];
  cookie = sc.map((c) => c.split(';')[0]).join('; ');
  const a = await get('/api/amoeba/summary');
  need('summary: top-level keys', a, ['records_by_kind', 'activities', 'recent_snapshots', 'obligations', 'implemented', 'not_implemented']);
  if (a.records_by_kind[0]) need('summary: records_by_kind[]', a.records_by_kind[0], ['family', 'kind', 'count']);
  if (a.activities[0]) need('summary: activities[]', a.activities[0], ['type', 'distinct_activities']);
  if (a.recent_snapshots[0]) need('summary: recent_snapshots[]', a.recent_snapshots[0], ['project_id', 'snapshot']);
  need('summary: obligations', a.obligations, ['open', 'total', 'open_on_superseded']);
  const lessons = await get('/api/lessons');
  if (lessons[0]) need('lessons[]', lessons[0], ['id', 'kind', 'family', 'parameter', 'wrong_value', 'right_value']);
  const obs = await get('/api/obligations');
  if (obs[0]) need('obligations[]', obs[0], ['id', 'target_calc', 'direction', 'via', 'state']);
  const cs = await get('/api/calculations');
  if (cs[0]) need('calculations[]', cs[0], ['id', 'state']);
  const withEdge = cs.find((c) => c.state !== 'REJECTED');
  if (withEdge) {
    const dep = await get(`/api/calculations/${withEdge.id}/dependencies`);
    need('dependencies', dep, ['edges', 'rule_table']);
    if (dep.edges[0]) need('dependencies.edges[]', dep.edges[0], ['from', 'rel', 'to']);
  }
  if (fails.length) { console.log(fails.map((f) => 'FAIL - ' + f).join('\n')); process.exit(1); }
  console.log('ok: AMOEBA fields read by the view are all present in the server output');
})().catch((e) => { console.log('FAIL - ' + e.message); process.exit(1); });
