// Black-box dump of ZK's mitered bend pieces for comparison with verify_miterbend.py (usage: node zk_bend_dump.mjs <zk_root>)
// ZK is only imported (read only). Case list = same parameters as verify_miterbend.py --table.
import { pathToFileURL } from "node:url";
import path from "node:path";
const zk = process.argv[2];
const { build } = await import(pathToFileURL(path.join(zk, "reader/build.mjs")).href);
const P = (o) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, { value: v, text: String(v), bbox: null, source: "drawing" }]));
const mk = (params, rules) => build({ page: { kind: "drawing", title: "", drawing_no: "X", job_no: "", scale: "", company: "", qty: 1, unit_mass_kg: null }, material_table: [], parts: [{ no: "1", name: "bend", shape: "miter_bend", table_rows: [], params: P(params) }], seam: null, notes: [], unreadable: [] }, rules);
const cases = JSON.parse(process.argv[3]);
const out = [];
for (const c of cases) {
  const r = mk({ D: c.D, t: c.t, R: c.R, angle: c.A, joints: c.J, start_straight: c.s1, end_straight: c.s2 }, { basis: c.basis, amp: c.amp || "outer", allow: c.allow, seam: { from: "地", angle: 45 } });
  out.push({ case: c, needs: r.checks.filter((k) => k.level === "need").map((k) => k.kind), pieces: r.pieces.map((p) => ({ label: p.label, n: p.n, w: p.w, h: p.h, kg: p.kg, info: Object.fromEntries(Object.entries(p.info || {}).filter(([, v]) => typeof v === "number" || typeof v === "string")), npts: p.pts ? p.pts.length : 0 })) });
}
console.log(JSON.stringify(out));
