// Black-box dump of ZK's cut-curve points for a mitered bend (usage: node zk_bend_curve.mjs <zk_root> '<cases json>')
// Each case: {D,t,R,A,J,basis}. The middle piece (centre length 2R tan(A/(2J))) is unfolded with lo = up = 1 (the form ZK's
// test uses for the swing), amp outer, allow 0, seam 45 from 地. Output: [{case, pts}]; pts = 361 lower + 361 upper points.
import { pathToFileURL } from "node:url";
import path from "node:path";
const zk = process.argv[2];
const { Z } = await import(pathToFileURL(path.join(zk, "reader/build.mjs")).href);
const out = [];
for (const c of JSON.parse(process.argv[3])) {
  const cl = 2 * c.R * Math.tan((c.A * Math.PI) / 180 / (2 * c.J));
  const u = Z.unfold({ shape: "bend", t: c.t, geo: { D: c.D, t: c.t, R: c.R, ang: c.A, joints: c.J, cl, lo: 1, up: 1, split: 1 } }, { basis: c.basis, amp: "outer", allow: 0, seam: { from: "地", angle: 45 } });
  out.push({ case: c, pts: u[0].pts });
}
console.log(JSON.stringify(out));
