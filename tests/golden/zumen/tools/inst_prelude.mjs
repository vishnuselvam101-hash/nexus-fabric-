// Preloaded (node --import) into the instrumented copy of ZK's test_build.mjs. No ZK file is touched.
// Captures every "OK "/"NG " result line with the call site that produced it (globalThis.__sid, set by __S), the helper data
// (globalThis.__pend), and the inputs of that call site: the values of the identifiers its arguments use, serialised so that
// engine results become {"$ref": call-id, "$path": accessor} and every engine call is recorded with its exact arguments.
import fs from "node:fs";
import path from "node:path";
const orig = console.log.bind(console);
const recs = [];
const files = [];
const origRead = fs.readFileSync;
fs.readFileSync = function (p, ...r) { const s = String(p); const m = s.match(/readings[\\/]([^\\/]+\.json)$/); if (m) files.push(m[1]); return origRead.call(this, p, ...r); };

// ---- registry of engine calls and results ----
const calls = [];            // {id, fn, args (serialised)}
const reg = new WeakMap();   // engine result (and every object inside it) -> {id, path}
const engineFns = new Map(); // wrapped function -> name
const accessor = (k) => (/^[A-Za-z_$][\w$]*$/.test(k) ? "." + k : `[${JSON.stringify(k)}]`);
function index(root, id) {
  const q = [[root, ""]], seen = new Set(); let n = 0;
  while (q.length && n < 30000) {
    const [o, p] = q.shift();
    if (!o || typeof o !== "object" || seen.has(o)) continue;
    seen.add(o); n++;
    if (!reg.has(o)) reg.set(o, { id, path: p });
    if (Array.isArray(o)) {
      if (o.length && (typeof o[0] !== "object" || o[0] === null)) continue;
      if (o.length && Array.isArray(o[0]) && typeof o[0][0] === "number") continue; // point lists
      o.forEach((x, i) => q.push([x, p + `[${i}]`]));
    } else for (const k of Object.keys(o)) q.push([o[k], p + accessor(k)]);
  }
}
// reading files, to turn an inline copy of an unmodified reading into a file reference
const readingsDir = process.env.ZK_READINGS;
const readingText = new Map();
for (const f of fs.readdirSync(readingsDir)) if (f.endsWith(".json")) readingText.set(JSON.stringify(JSON.parse(origRead(path.join(readingsDir, f), "utf8"))), f);
const BUILTIN = new Map([[fs, "fs"], [path, "path"], [process, "process"]]);
function ser(v, depth = 0) {
  if (v === undefined) return { $undef: 1 };
  if (v === null || typeof v === "boolean" || typeof v === "string") return v;
  if (typeof v === "number") return Number.isFinite(v) ? v : { $num: String(v) };
  if (typeof v === "bigint" || typeof v === "symbol") return { $str: String(v) };
  if (typeof v === "function") return engineFns.has(v) ? { $engine: engineFns.get(v) } : { $fn: v.toString() };
  if (BUILTIN.has(v)) return { $builtin: BUILTIN.get(v) };
  if (v === globalThis.__Zwrapped) return { $engine: "Z" };
  const r = reg.get(v);
  if (r) return { $ref: r.id, $path: r.path };
  if (depth > 40) return { $str: "[too deep]" };
  if (Array.isArray(v)) return v.map((x) => ser(x, depth + 1));
  if (v && Array.isArray(v.parts) && v.page) { const f = readingText.get(JSON.stringify(v)); if (f) return { $reading: f }; }
  const o = {};
  for (const k of Object.keys(v)) o[k] = ser(v[k], depth + 1);
  return o;
}
let depthCall = 0;
globalThis.__zkTrack = (name, f, self) => {
  const w = function (...args) {
    if (depthCall > 0) return f.apply(self ?? this, args);
    const entry = { id: calls.length + 1, fn: name, args: args.map((a) => ser(a)) };
    calls.push(entry);
    depthCall++;
    let res;
    try { res = f.apply(self ?? this, args); } finally { depthCall--; }
    if (res && typeof res === "object") index(res, entry.id);
    return res;
  };
  engineFns.set(w, name);
  return w;
};
// call-site capture: __S(id, () => ({ident: () => value, ...}), fn, ...args) runs fn(...args) with the site's inputs recorded
globalThis.__sid = null; globalThis.__env = null;
globalThis.__S = (id, thunk, fn, ...args) => { globalThis.__sid = id; globalThis.__env = thunk; return fn(...args); };
globalThis.__S0 = (id, thunk) => { globalThis.__sid = id; globalThis.__env = thunk; };

console.log = (...a) => {
  const s = String(a[0]);
  if (/^(OK|NG) /.test(s)) {
    let env = null;
    if (globalThis.__env) {
      env = {};
      const o = globalThis.__env();
      for (const [k, get] of Object.entries(o)) { let val, has = true; try { val = get(); } catch { has = false; } if (has) env[k] = ser(val); }
    }
    recs.push({ line: s, pend: globalThis.__pend || null, sid: globalThis.__sid, env, readings: [...new Set(files.splice(0))] });
    globalThis.__pend = null;
  } else orig(...a);
};
process.on("exit", () => fs.writeFileSync(process.env.ZK_RECS, JSON.stringify({ recs, calls })));
