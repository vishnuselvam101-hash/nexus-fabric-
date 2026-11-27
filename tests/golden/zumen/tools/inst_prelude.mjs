// Preloaded (node --import) into the instrumented copy of ZK's test_build.mjs.
// Captures every "OK "/"NG " result line with the call-site stack and the pending helper data. No ZK file is touched.
import fs from "node:fs";
const orig = console.log.bind(console);
const recs = [];
const files = [];
const origRead = fs.readFileSync;
fs.readFileSync = function (p, ...r) { const s = String(p); const m = s.match(/readings[\\/]([^\\/]+\.json)$/); if (m) files.push(m[1]); return origRead.call(this, p, ...r); };
console.log = (...a) => {
  const s = String(a[0]);
  if (/^(OK|NG) /.test(s)) {
    const frames = [];
    for (const l of new Error().stack.split("\n")) { const m = l.match(new RegExp(process.env.ZK_INST.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + ":(\\d+):(\\d+)")); if (m) frames.push([+m[1], +m[2]]); }
    recs.push({ line: s, pend: globalThis.__pend || null, frames, readings: [...new Set(files.splice(0))] });
    globalThis.__pend = null;
  } else orig(...a);
};
process.on("exit", () => fs.writeFileSync(process.env.ZK_RECS, JSON.stringify(recs)));
