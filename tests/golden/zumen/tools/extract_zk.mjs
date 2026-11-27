// Extract the INDEPENDENT expected values of zumen-kirizu (ZK) test_build.mjs into tests/golden/zumen/*.json.
// usage: node extract_zk.mjs <zk_root> <out_dir>
// ZK is read only. The test file is copied to a temp dir and instrumented there (helper bodies record their arguments);
// call sites are then parsed from the ORIGINAL text to see whether the expected value is a literal (independent)
// or computed by ZK's engine (shared.js / build.mjs), which is then excluded and listed in excluded.json.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";

const [zk, out] = process.argv.slice(2);
if (!zk || !out) { console.error("usage: node extract_zk.mjs <zk_root> <out_dir>"); process.exit(2); }
const here = path.dirname(fileURLToPath(import.meta.url));
const TB_REL = "design-demos/assets/kirizu/test_build.mjs";
const tbDir = path.join(zk, path.dirname(TB_REL));
const src = fs.readFileSync(path.join(zk, TB_REL), "utf8");
const lines = src.split("\n");
const zkCommit = spawnSync("git", ["-C", zk, "rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();

// ---- instrument a copy (same line numbers) ----
let inst = src
  .replace("const here = path.dirname(fileURLToPath(import.meta.url));", `const here = ${JSON.stringify(tbDir)};`)
  .replaceAll('"../../../reader/build.mjs"', JSON.stringify(pathToFileURL(path.join(zk, "reader/build.mjs")).href))
  .replace("const eq = (name, got, want, tol = 0.1) => {", "const eq = (name, got, want, tol = 0.1) => { globalThis.__pend = { k: \"eq\", got, want, tol };")
  .replace("const has = (name, r, level, word) => {", "const has = (name, r, level, word) => { globalThis.__pend = { k: \"has\", level, word };")
  .replace("const count = (name, r, level, want) => {", "const count = (name, r, level, want) => { globalThis.__pend = { k: \"count\", want };")
  .replace(/const ok = \(name, cond(, info = "")?\) => \{/g, (m) => m + ' globalThis.__pend = { k: "ok", got: !!cond };');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "zkx-"));
const instPath = path.join(tmp, "test_build_inst.mjs"), recsPath = path.join(tmp, "recs.json");
fs.writeFileSync(instPath, inst);
const r = spawnSync("nice", ["-n", "10", "node", "--import", pathToFileURL(path.join(here, "inst_prelude.mjs")).href, instPath], { env: { ...process.env, ZK_INST: instPath, ZK_RECS: recsPath }, encoding: "utf8", maxBuffer: 1 << 28 });
if (!fs.existsSync(recsPath)) { console.error("instrumented run failed", r.stderr); process.exit(1); }
const recs = JSON.parse(fs.readFileSync(recsPath, "utf8"));
const zkOk = recs.filter((x) => x.line.startsWith("OK ")).length, zkNg = recs.length - zkOk;

// ---- helper definition line ranges (frames inside them are not the user call site) ----
const helperRe = /^\s*const (eq|has|count|stopNeed|stopOk|noPieces|ok|pct)\s*=\s*\(/;
const helperLines = new Set();
for (let i = 0; i < lines.length; i++) {
  if (!helperRe.test(lines[i])) continue;
  let bal = 0, j = i;
  for (; j < lines.length; j++) { for (const ch of lines[j]) { if (ch === "{" || ch === "(") bal++; else if (ch === "}" || ch === ")") bal--; } helperLines.add(j + 1); if (bal <= 0 && /[;]\s*$/.test(lines[j].replace(/\/\/.*$/, "")) ) break; if (bal <= 0 && j > i) break; if (bal <= 0 && j === i) break; }
}

// ---- call-site parsing from the original text ----
function callAt(line, col) {
  const L = lines[line - 1];
  const names = ["eq", "pct", "count", "has", "ok", "stopNeed", "stopOk", "noPieces"];
  // V8 column points at the callee or at '('; look for the nearest known name( at or before col, else first on line after col
  let best = null;
  for (const n of names) { const re = new RegExp("(?<![\\w.])" + n + "\\("); let m; const g = new RegExp(re.source, "g"); while ((m = g.exec(L))) { const d = Math.abs(m.index + 1 - col); if (best === null || d < best.d) best = { n, idx: m.index + n.length, d }; } }
  if (!best) return null;
  // balanced args, may span lines
  let text = L.slice(best.idx + 1), l = line - 1, depth = 1, args = [], cur = "", inStr = null;
  for (;;) {
    for (let k = 0; k < text.length; k++) {
      const ch = text[k];
      if (inStr) { cur += ch; if (ch === "\\") { cur += text[++k] || ""; } else if (ch === inStr) inStr = null; continue; }
      if (ch === '"' || ch === "'" || ch === "`") { inStr = ch; cur += ch; continue; }
      if ("([{".includes(ch)) depth++; else if (")]}".includes(ch)) { depth--; if (depth === 0) { args.push(cur.trim()); return { fn: best.n, args }; } }
      if (ch === "," && depth === 1) { args.push(cur.trim()); cur = ""; continue; }
      cur += ch;
    }
    cur += " "; l++; if (l >= lines.length) return null; text = lines[l];
  }
}
const ENGINE = /\bZs\.|\b(build|pc|load|readE|readF|readR|readK|rd|unf|netKg|kgOf|brKg|brU|coneU|coneKg|mnI|tm|cyl|bend|area|perim|per|marks|sf|genAng|leftEdge|rightEdge|sum|bb|holesOf|teeY|fitStock|redBendModel|cbModel|summary|unfold)\(/;
function declRhs(name, upto) {
  for (let i = upto - 1; i >= 0; i--) { const m = lines[i].match(new RegExp("(?:const|let)\\s+" + name + "\\s*=\\s*(.*)$")); if (m) { let t = m[1]; for (let j = i + 1; j < lines.length && !/;\s*(\/\/.*)?$/.test(t); j++) t += " " + lines[j]; return t; } }
  return null;
}
// identifier bound as an arrow parameter / destructured table row / for-of variable within a few lines before the call
function isParam(id, ln) {
  const ctx = lines.slice(Math.max(0, ln - 14), ln).join("\n");
  const re = [new RegExp("\\(\\[[^\\]]*\\b" + id + "\\b[^\\]]*\\](?:\\s*,\\s*\\w+)*\\)\\s*=>"), new RegExp("\\((?:[^()]*,\\s*)?" + id + "(?:\\s*,[^()]*)?\\)\\s*=>"), new RegExp("for \\((?:const|let) \\[?[^)]*\\b" + id + "\\b"), new RegExp("=>\\s*\\{?[^\\n]*\\b" + id + "\\b")];
  return re.slice(0, 3).some((r) => r.test(ctx));
}
// literal-ness of an expression: numbers/arithmetic/Math.* constants; table-row parameters; identifiers resolved through their declarations
// closed-form expectations written out in the test from public formulas (checked by hand, not engine output)
const ANALYTIC_LINES = new Set([595, 833]);
function literalExpr(expr, line, depth = 0) {
  if (ANALYTIC_LINES.has(line)) return { lit: true };
  if (depth > 3) return { lit: false, why: "deep" };
  if (/^[-+*/().\d\se,\s]*$/.test(expr)) return { lit: true };
  if (ENGINE.test(expr)) return { lit: false, why: "engine call in expression" };
  const stripped = expr.replace(/Math\.(PI|abs|sqrt|tan|sin|cos|atan|asin|acos|hypot|min|max)\b/g, "M").replace(/\d+\.\d+/g, "0");
  const roots = [...new Set((stripped.match(/(?<![\w.])[A-Za-z_]\w*/g) || []))].filter((x) => x !== "M" && !/^(true|false|null)$/.test(x));
  for (const id of roots) {
    if (isParam(id, line)) continue;
    const rhs = declRhs(id, line);
    if (rhs === null) { if (/\.\w/.test(stripped) && new RegExp("\\b" + id + "\\.").test(stripped)) return { lit: false, why: `${id}.property of an unresolved object` }; continue; }
    const rr = literalExpr(rhs.replace(/;\s*$/, "").trim(), line, depth + 1);
    if (!rr.lit) return { lit: false, why: `${id} derived from the engine (${rhs.slice(0, 60)})` };
  }
  return { lit: true };
}

// ---- sections ----
const sectionStarts = [];
lines.forEach((l, i) => { if (/^\/\/ /.test(l)) sectionStarts.push({ line: i + 1, text: l.slice(3).trim() }); });
const sectionAt = (ln) => { let s = null; for (const x of sectionStarts) if (x.line <= ln) s = x; return s; };
const commentAt = (ln) => { for (let i = ln - 1; i >= 0; i--) { const m = lines[i].match(/^\s*\/\/ (.*)$/); if (m) return m[1].trim(); if (i < ln - 40) break; } return null; };
const verifyOf = (t) => (t && (t.match(/verify_\w+\.py/g) || [])[0]) || null;

const cases = [], excluded = [];
const bump = (o, k) => (o[k] = (o[k] || 0) + 1);
for (const rec of recs) {
  const ok = rec.line.startsWith("OK ");
  const fr = rec.frames.find(([l]) => !helperLines.has(l));
  const ln = fr ? fr[0] : 0, col = fr ? fr[1] : 0;
  const call = fr ? callAt(ln, col) : null;
  const name = rec.line.replace(/^(OK|NG) /, "").replace(/\s{2}.*$/, "").trim();
  const sec = sectionAt(ln), near = commentAt(ln);
  const vs = verifyOf(near) || verifyOf(sec && sec.text);
  const base = { name, source: `${TB_REL}:${ln}`, section_line: sec ? sec.line : 0, origin: { section_comment: sec ? sec.text : null, nearest_comment: near }, basis: vs ? `${vs} (named in the test comment)` : "literal in test_build.mjs; origin not named in the comment", reading_hint: rec.readings, zk_status: ok ? "OK" : "NG" };
  const P = rec.pend || { k: "inline" };
  let c = null, wexpr = null;
  if (P.k === "eq" && call && ["eq", "pct", "noPieces"].includes(call.fn)) {
    wexpr = call.fn === "noPieces" ? "0" : call.args[2];
    const lit = literalExpr(wexpr, ln);
    c = { kind: "num", want: P.want, tol: P.tol, zk_got: P.got, want_expr: wexpr, _lit: lit };
  } else if (P.k === "eq") {
    c = { kind: "num", want: P.want, tol: P.tol, zk_got: P.got, want_expr: null, _lit: { lit: false, why: "call site not parsed" } };
  } else if (P.k === "has") {
    c = { kind: "check_text", want: { level: P.level, contains: P.word }, zk_got: ok, want_expr: call ? call.args.slice(2).join(", ") : null, _lit: { lit: true } };
  } else if (P.k === "count") {
    const m = rec.line.match(/\s(-?\d+)（期待 (-?\d+)）/);
    wexpr = call ? call.args[3] : null;
    c = { kind: "count", want: P.want, level: null, zk_got: m ? +m[1] : null, want_expr: wexpr, _lit: wexpr ? literalExpr(wexpr, ln) : { lit: false, why: "call site not parsed" } };
  } else if (P.k === "ok" || !rec.pend) {
    const fn = call ? call.fn : "inline";
    c = { kind: fn === "stopNeed" || fn === "stopOk" ? "refusal_kind" : "bool", want: true, zk_got: ok, want_expr: call ? (call.args.slice(1).join(", ").slice(0, 200)) : null, _lit: { lit: true } };
    if (fn === "stopNeed" || fn === "stopOk") c.want = { refusal_kind: call.args[2], no_exception: true, no_nan: true };
  }
  if (!c) { excluded.push({ ...base, reason: "unclassified" }); continue; }
  const lit = c._lit; delete c._lit;
  if (!lit.lit) { excluded.push({ ...base, kind: c.kind, want_expr: c.want_expr, zk_want: c.want, reason: "self-referential: " + lit.why }); continue; }
  cases.push({ ...base, ...c });
}

// ---- write ----
fs.mkdirSync(out, { recursive: true });
for (const f of fs.readdirSync(out)) if (/^s\d+_.*\.json$|^(index|excluded)\.json$/.test(f)) fs.rmSync(path.join(out, f));
const bySec = new Map();
cases.forEach((c) => { (bySec.get(c.section_line) || bySec.set(c.section_line, []).get(c.section_line)).push(c); });
const secList = [...bySec.keys()].sort((a, b) => a - b);
const index = [];
secList.forEach((sl, i) => {
  const arr = bySec.get(sl);
  const id = "s" + String(i + 1).padStart(2, "0");
  arr.forEach((c, k) => { c.id = `${id}-${String(k + 1).padStart(4, "0")}`; delete c.section_line; });
  const file = `${id}_L${sl}.json`;
  const title = sectionStarts.find((x) => x.line === sl)?.text ?? "(before first section comment)";
  fs.writeFileSync(path.join(out, file), JSON.stringify({ zk_commit: zkCommit, section: title, section_source: `${TB_REL}:${sl}`, cases: arr }, null, 1) + "\n");
  index.push({ file, section: title, source_line: sl, cases: arr.length });
});
excluded.forEach((e) => delete e.section_line);
fs.writeFileSync(path.join(out, "excluded.json"), JSON.stringify({ zk_commit: zkCommit, note: "asserts whose expected value is computed by ZK's own engine, or not parseable; not golden", excluded }, null, 1) + "\n");
const kinds = {}; cases.forEach((c) => bump(kinds, c.kind));
const withVerify = cases.filter((c) => c.basis.startsWith("verify_")).length;
const summary = { zk_commit: zkCommit, zk_run: { ok: zkOk, ng: zkNg, total: recs.length }, extracted: cases.length, excluded: excluded.length, by_kind: kinds, basis_names_a_verify_script: withVerify, sections: index };
fs.writeFileSync(path.join(out, "index.json"), JSON.stringify(summary, null, 1) + "\n");
console.log(JSON.stringify({ zk_run: summary.zk_run, extracted: cases.length, excluded: excluded.length, by_kind: kinds, basis_names_a_verify_script: withVerify, sections: index.length }));
fs.rmSync(tmp, { recursive: true, force: true });
