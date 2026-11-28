// Case adapter: evaluates golden cases (inputs + got.expr) against an engine and prints what the engine answers.
// Engine = anything offering build(reading, rules, over, std) and Z.{unfold,summary,dxf,bbox,...}; here it is ZK itself, as a proof that the
// recorded inputs are complete (no hand mapping). A real engine adapter replaces `engine` below.
//   stdin mode (replay engine; golden_zumen.rs does not call it yet): {"zk_root":..,"golden":..,"cases":[case,...]} -> [{id, got | error}]
//   --all mode: node adapter_zk.mjs <zk_root> <golden_dir> --all  -> judges every case with ZK's tolerance rules
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const realLog = console.log.bind(console);
let capture = null;
console.log = (...a) => { if (capture) capture.push(a.map(String).join(" ")); };

export async function loadEngine(zkRoot) { const m = await import(pathToFileURL(path.join(zkRoot, "reader/build.mjs")).href); return { build: m.build, Z: m.Z }; }

const accessorSteps = (p) => { const steps = []; for (const m of p.matchAll(/\.([A-Za-z_$][\w$]*)|\[(\d+)\]|\["((?:[^"\\]|\\.)*)"\]/g)) steps.push(m[1] ?? (m[2] !== undefined ? +m[2] : JSON.parse('"' + m[3] + '"'))); return steps; };

export function evalCase(c, engine, golden) {
  const results = {};
  const revive = (v) => {
    if (v === null || typeof v !== "object") return v;
    if (Array.isArray(v)) return v.map(revive);
    if ("$ref" in v) { let x = results[v.$ref]; for (const k of accessorSteps(v.$path)) x = x[k]; return x; }
    if ("$reading" in v) return JSON.parse(fs.readFileSync(path.join(golden, "readings", v.$reading), "utf8"));
    if ("$num" in v) return Number(v.$num);
    if ("$undef" in v) return undefined;
    if ("$engine" in v) return v.$engine === "build" ? engine.build : v.$engine === "Z" ? engine.Z : engine.Z[v.$engine.slice(2)];
    if ("$builtin" in v) return v.$builtin === "fs" ? fs : v.$builtin === "path" ? path : v.$builtin === "process" ? process : path.join(golden);
    if ("$str" in v) return v.$str;
    const o = {}; for (const k of Object.keys(v)) o[k] = revive(v[k]); return o;
  };
  for (const call of c.inputs.calls) {
    const f = call.fn === "build" ? engine.build : engine.Z[call.fn.slice(2)];
    results[call.id] = f.apply(call.fn === "build" ? undefined : engine.Z, call.args.map(revive));
  }
  const env = {}; for (const [k, v] of Object.entries(c.inputs.env)) env[k] = revive(v);
  const names = Object.keys(env), hn = Object.keys(c.inputs.helpers);
  const body = names.map((n) => `let ${n} = __env[${JSON.stringify(n)}];`).join("\n") + "\n" + hn.map((n) => `let ${n} = ${c.inputs.helpers[n]};`).join("\n") + "\n" + (names.includes("ng") || hn.includes("ng") ? "" : "let ng = 0;\n") + `return (${c.got.expr});`;
  const __v = (f) => { const saved = capture; capture = []; const out = {}; try { f(); } finally { for (const l of capture) { const n = l.replace(/^(OK|NG) /, "").replace(/\s{2}.*$/, "").trim(); if (!(n in out)) out[n] = l.startsWith("OK "); } capture = saved; } return out; };
  return new Function("__env", "__v", body)(env, __v);
}
const judge = (c, got) => {
  if (c.kind === "num") return typeof got === "number" && Number.isFinite(got) && Math.abs(got - c.want) <= c.tol;
  if (c.kind === "count") return got === c.want;
  return got === true;
};
const jsonGot = (g) => (typeof g === "number" && !Number.isFinite(g) ? null : g === undefined ? null : g);

if (process.argv[1] && path.resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  if (process.argv[4] === "--all") {
    const [zk, golden] = process.argv.slice(2), engine = await loadEngine(zk);
    const idx = JSON.parse(fs.readFileSync(path.join(golden, "index.json"), "utf8"));
    let n = 0, ok = 0, ng = 0, err = 0;
    for (const s of idx.sections) for (const c of JSON.parse(fs.readFileSync(path.join(golden, s.file), "utf8")).cases) {
      n++;
      try { if (judge(c, evalCase(c, engine, golden))) ok++; else { ng++; realLog("NG", c.id, c.name, c.source); } } catch (e) { err++; realLog("ERR", c.id, c.name, String(e.message).slice(0, 100)); }
    }
    realLog(`adapter(ZK) all cases: ${ok} OK, ${ng} NG, ${err} ERROR of ${n}`);
    process.exit(ng + err ? 1 : 0);
  } else {
    const chunks = []; for await (const ch of process.stdin) chunks.push(ch);
    const { zk_root, golden, cases } = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    const engine = await loadEngine(zk_root);
    const res = cases.map((c) => { try { return { id: c.id, got: jsonGot(evalCase(c, engine, golden)) }; } catch (e) { return { id: c.id, error: String(e.message).slice(0, 200) }; } });
    process.stdout.write(JSON.stringify(res));
  }
}
