// Extract the INDEPENDENT expected values of zumen-kirizu (ZK) test_build.mjs into tests/golden/zumen/*.json.
// usage: node extract_zk.mjs <zk_root> <out_dir>
// ZK is read only. The test file is copied to a temp dir and instrumented there. Every call of an assert helper (eq, pct, has, count, ok, stopNeed,
// stopOk, noPieces, sf, keep) is rewritten to `__S(site, inputs-thunk, helper, ...args)`, so that when a result line is printed we also know the
// call site and the values of the identifiers its arguments use. Engine calls (build and Z.*) are recorded with their exact arguments, and
// engine results are replaced by {"$ref": call, "$path": accessor}. Each case therefore carries `inputs` (calls + env + helpers) and `got.expr`,
// a JavaScript expression over them: an adapter evaluates it against ANY engine that offers build() and Z.* (tools/adapter_zk.mjs does it for ZK).
// The classifier decides from the ORIGINAL text whether the expected value is a literal (independent) or computed by ZK's own engine (excluded).
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
const lineStart = []; { let o = 0; for (const l of lines) { lineStart.push(o); o += l.length + 1; } }
const lineOf = (off) => { let lo = 0, hi = lineStart.length - 1; while (lo < hi) { const m = (lo + hi + 1) >> 1; if (lineStart[m] <= off) lo = m; else hi = m - 1; } return lo + 1; };
const zkCommit = spawnSync("git", ["-C", zk, "rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();

// ---- helper definition lines (statements inside them are not user call sites) ----
const PRIMS = ["eq", "has", "count", "stopNeed", "stopOk", "noPieces", "ok", "pct", "sf", "keep"];
const VERDICT = new Set(["stopNeed", "stopOk", "sf", "keep"]);
const helperRe = new RegExp("^\\s*const (" + PRIMS.join("|") + ")\\s*=\\s*\\(");
const helperLines = new Set();
for (let i = 0; i < lines.length; i++) {
  if (!helperRe.test(lines[i])) continue;
  let bal = 0, j = i;
  for (; j < lines.length; j++) { for (const ch of lines[j]) { if (ch === "{" || ch === "(") bal++; else if (ch === "}" || ch === ")") bal--; } helperLines.add(j + 1); if (bal <= 0 && /[;]\s*$/.test(lines[j].replace(/\/\/.*$/, ""))) break; if (bal <= 0 && j > i) break; if (bal <= 0 && j === i) break; }
}

// ---- tiny JS scanner: skips comments, strings, templates, regex literals; finds helper call sites and splits arguments ----
function skipString(t, i) { const q = t[i]; i++; while (i < t.length && t[i] !== q) { if (t[i] === "\\") i++; i++; } return i + 1; }
function skipTemplate(t, i) { i++; while (i < t.length && t[i] !== "`") { if (t[i] === "\\") { i += 2; continue; } if (t[i] === "$" && t[i + 1] === "{") { let d = 1; i += 2; while (i < t.length && d > 0) { if (t[i] === "{") d++; else if (t[i] === "}") d--; else if (t[i] === '"' || t[i] === "'") { i = skipString(t, i); continue; } else if (t[i] === "`") { i = skipTemplate(t, i); continue; } i++; } continue; } i++; } return i + 1; }
function skipRegex(t, i) { i++; let cls = false; while (i < t.length) { if (t[i] === "\\") { i += 2; continue; } if (t[i] === "[") cls = true; else if (t[i] === "]") cls = false; else if (t[i] === "/" && !cls) return i + 1; i++; } return i; }
function regexAllowed(t, i) { let k = i - 1; while (k >= 0 && /\s/.test(t[k])) k--; return k < 0 || "(,=:[!&|?{};+-*%<>~^".includes(t[k]) || /\breturn$/.test(t.slice(Math.max(0, k - 6), k + 1)); }
// step over one lexical unit at i; returns the new index
function stepLex(t, i) {
  const c = t[i];
  if (c === "/" && t[i + 1] === "/") { while (i < t.length && t[i] !== "\n") i++; return i; }
  if (c === "/" && t[i + 1] === "*") { const e = t.indexOf("*/", i + 2); return e < 0 ? t.length : e + 2; }
  if (c === '"' || c === "'") return skipString(t, i);
  if (c === "`") return skipTemplate(t, i);
  if (c === "/" && regexAllowed(t, i)) return skipRegex(t, i);
  return i + 1;
}
function parseArgs(t, open) { // t[open] === "("; returns {args, end}
  const args = []; let depth = 1, cur = "", i = open + 1;
  while (i < t.length) {
    const c = t[i];
    if ("([{".includes(c)) { depth++; cur += c; i++; continue; }
    if (")]}".includes(c)) { depth--; if (depth === 0) { args.push(cur.trim()); return { args, end: i }; } cur += c; i++; continue; }
    if (c === "," && depth === 1) { args.push(cur.trim()); cur = ""; i++; continue; }
    const j = stepLex(t, i); cur += t.slice(i, j); i = j;
  }
  return null;
}
function splitTop(t) { const parts = []; let depth = 0, cur = "", i = 0; while (i < t.length) { const c = t[i]; if ("([{".includes(c)) depth++; else if (")]}".includes(c)) depth--; if (c === "," && depth === 0) { parts.push(cur.trim()); cur = ""; i++; continue; } const j = stepLex(t, i); cur += t.slice(i, j); i = j; } parts.push(cur.trim()); return parts; }

// ---- identifiers of an expression (names it reads from the surrounding scope) ----
const KEYWORDS = new Set("true false null undefined NaN Infinity this new typeof void delete in of instanceof return if else for while do const let var function async await class try catch finally throw switch case break continue default yield Math Object Number Array JSON String Set Map Boolean Date Error RegExp Symbol import fileURLToPath parseFloat parseInt isNaN isFinite console globalThis".split(" "));
function stripLits(t) { // blank string/template text, keep ${...} code, drop comments and regex literals
  let o = "", i = 0;
  while (i < t.length) {
    const c = t[i];
    if (c === '"' || c === "'") { i = skipString(t, i); o += '""'; continue; }
    if (c === "`") { const j = skipTemplate(t, i); const body = t.slice(i + 1, j - 1); let k = 0; while (k < body.length) { if (body[k] === "\\") { k += 2; continue; } if (body[k] === "$" && body[k + 1] === "{") { let d = 1, m = k + 2; while (m < body.length && d > 0) { if (body[m] === "{") d++; else if (body[m] === "}") d--; m++; } o += " (" + stripLits(body.slice(k + 2, m - 1)) + ") "; k = m; continue; } k++; } o += '""'; i = j; continue; }
    const j = stepLex(t, i);
    if (j - i > 1 && (c === "/")) { o += " "; i = j; continue; }
    o += t.slice(i, j); i = j;
  }
  return o;
}
function identsOf(text) {
  let s = stripLits(text);
  const bound = new Set();
  for (const m of s.matchAll(/\(([^()]*)\)\s*=>/g)) for (const x of m[1].replace(/=[^,]*/g, "").matchAll(/[A-Za-z_$][\w$]*/g)) bound.add(x[0]); // defaults (o = opt) are not parameters
  for (const m of s.matchAll(/(?<![\w$.])([A-Za-z_$][\w$]*)\s*=>/g)) bound.add(m[1]);
  for (const m of s.matchAll(/\b(?:const|let|var)\s+(\[[^\]]*\]|\{[^}]*\}|[A-Za-z_$][\w$]*)/g)) for (const x of m[1].matchAll(/[A-Za-z_$][\w$]*/g)) bound.add(x[0]);
  s = s.replace(/\.\.\./g, " ").replace(/\?\.\s*[A-Za-z_$][\w$]*|\.\s*[A-Za-z_$][\w$]*/g, " ").replace(/([{,]\s*)([A-Za-z_$][\w$]*)\s*:(?!:)/g, "$1 ");
  const ids = new Set();
  for (const m of s.matchAll(/(?<![\w$])([A-Za-z_$][\w$]*)/g)) if (!KEYWORDS.has(m[1]) && !bound.has(m[1])) ids.add(m[1]);
  return ids;
}
// the declaration that supplies `name` at line `ln` (1-based): {line, rhs} of the nearest preceding declaration, or null
function declOf(name, ln) {
  for (let i = ln - 1; i >= 0; i--) {
    const L = lines[i];
    let m = L.match(new RegExp("(?:const|let|var)\\s+" + name + "\\s*=\\s*(.*)$")), first = true;
    if (!m) { m = L.match(new RegExp("[,;]\\s*(?:const\\s+)?" + name + "\\s*=\\s*(?![=>])(.*)$")); first = false; }
    if (!m) continue;
    if (!first && !/(?:const|let|var)\s/.test(L.slice(0, m.index + 1))) continue; // a later declarator needs a declaration keyword before it
    let t = m[1];
    const open = (x) => { let d = 0; for (const ch of stripLits(x)) { if ("([{".includes(ch)) d++; else if (")]}".includes(ch)) d--; } return d; };
    for (let j = i + 1; j < lines.length && !(/;\s*(\/\/.*)?$/.test(t) && open(t) <= 0); j++) t += " " + lines[j].replace(/(^|\s)\/\/ .*$/, "");
    t = t.replace(/;\s*(\/\/.*)?$/, "");
    return { line: i + 1, rhs: splitTop(t)[0] };
  }
  return null;
}
function paramLine(id, ln) { // nearest line within 14 above that binds id as an arrow/for parameter
  const pats = [new RegExp("\\(\\[[^\\]]*\\b" + id + "\\b[^\\]]*\\](?:\\s*,\\s*\\w+)*\\)\\s*=>"), new RegExp("\\((?:[^()]*,\\s*)?" + id + "(?:\\s*,[^()]*)?\\)\\s*=>"), new RegExp("for \\((?:const|let) \\[?[^)]*\\b" + id + "\\b"), new RegExp("(?<![\\w$.])" + id + "\\s*=>")];
  for (let i = ln - 1; i >= Math.max(0, ln - 15); i--) if (pats.some((r) => r.test(lines[i]))) return i + 1;
  return 0;
}
// transitive set of scope names that an expression needs (declarations followed through their right-hand sides)
function closure(text, ln) {
  const seen = new Set(), work = [...identsOf(text)];
  let k = 0;
  while (work.length && k++ < 400) {
    const id = work.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    const d = declOf(id, ln);
    if (d) for (const x of identsOf(d.rhs)) if (!seen.has(x)) work.push(x);
  }
  return seen;
}

// ---- literal-ness of an expected-value expression (does ZK's own engine feed it?) ----
const ENGINE = /\bZs\.|\b(build|pc|load|readE|readF|readR|readK|rd|unf|netKg|kgOf|brKg|brU|coneU|coneKg|mnI|tm|cyl|bend|area|perim|per|marks|sf|genAng|leftEdge|rightEdge|sum|bb|holesOf|teeY|fitStock|redBendModel|cbModel|summary|unfold)\(/;
const ANALYTIC_LINES = new Set([595, 833]); // closed-form expectations written out in the test from public formulas (checked by hand)
function literalExpr(expr, line, depth = 0) {
  if (ANALYTIC_LINES.has(line)) return { lit: true };
  if (depth > 3) return { lit: false, why: "deep" };
  if (/^[-+*/().\d\se,\s]*$/.test(expr)) return { lit: true };
  if (ENGINE.test(expr)) return { lit: false, why: "engine call in expression" };
  const stripped = expr.replace(/Math\.(PI|abs|sqrt|tan|sin|cos|atan|asin|acos|hypot|min|max)\b/g, "M").replace(/\d+\.\d+/g, "0");
  for (const id of identsOf(stripped)) {
    if (id === "M") continue;
    const d = declOf(id, line), pl = paramLine(id, line);
    if (!d || pl > d.line) continue; // loop/table parameter: a literal from the table in the test
    const rr = literalExpr(d.rhs.trim(), d.line, depth + 1);
    if (!rr.lit) return { lit: false, why: `${id} derived from the engine (${d.rhs.slice(0, 60)})` };
  }
  return { lit: true };
}

// ---- call sites ----
const sites = [];
{
  const NAMES = new Set(PRIMS);
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/[A-Za-z_$]/.test(c) && !/[\w$.]/.test(src[i - 1] || " ")) {
      let j = i; while (j < src.length && /[\w$]/.test(src[j])) j++;
      const id = src.slice(i, j);
      if (NAMES.has(id) && src[j] === "(" && !helperLines.has(lineOf(i))) {
        const pa = parseArgs(src, j);
        if (pa) sites.push({ id: sites.length, fn: id, off: i, nameEnd: j, line: lineOf(i), args: pa.args });
      }
      i = j; continue;
    }
    if (/[\w$]/.test(c)) { while (i < src.length && /[\w$]/.test(src[i])) i++; continue; }
    i = stepLex(src, i);
  }
}
// direct console.log result lines that are not produced by a helper (an `ok` computed on the line before)
const INLINE = [];
for (const ln of [60, 77, 150, 193]) {
  const L = lines[ln - 1], at = lineStart[ln - 1] + L.indexOf("console.log(");
  let expr = null;
  if (ln === 60) expr = L.match(/const ok = (.*?); if \(!ok\)/)[1];
  if (ln === 77) expr = L.match(/const noSeam = (.*?); if \(!noSeam\)/)[1];
  if (ln === 150) expr = "fromTbl(s, l) === want";
  if (ln === 193) expr = lines[191].match(/const noTeeQty = (.*);\s*$/)[1];
  const s = { id: sites.length, fn: "inline", off: at, line: ln, args: ["", expr], inline: true };
  sites.push(s); INLINE.push(s);
}
for (const s of sites) {
  const gotSide = s.inline ? [s.args[1]] : s.fn === "noPieces" || s.fn === "ok" ? [s.args[1]] : VERDICT.has(s.fn) ? [`${s.fn}(${s.args.join(", ")})`] : s.fn === "count" || s.fn === "has" ? s.args.slice(1) : [s.args[1]];
  s.capture = [...closure(gotSide.join(" , "), s.line)].sort();
  if (s.fn === "ok" && s.args.length > 1) s.gotExprBase = s.args[1];
}

// ---- instrument a copy (same line numbers) ----
let inst = src;
for (const s of [...sites].sort((a, b) => b.off - a.off)) {
  const thunk = `() => ({${s.capture.map((n) => `${JSON.stringify(n)}: () => ${n}`).join(", ")}})`;
  if (s.inline) inst = inst.slice(0, s.off) + `__S0(${s.id}, ${thunk}); ` + inst.slice(s.off);
  else inst = inst.slice(0, s.off) + `__S(${s.id}, ${thunk}, ${s.fn}, ` + inst.slice(s.nameEnd + 1);
}
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "zkx-"));
const wrapPath = path.join(tmp, "zk_wrap.mjs");
fs.writeFileSync(wrapPath, fs.readFileSync(path.join(here, "zk_wrap.mjs"), "utf8").replace("ZK_BUILD_URL", pathToFileURL(path.join(zk, "reader/build.mjs")).href));
inst = inst
  .replace("const here = path.dirname(fileURLToPath(import.meta.url));", `const here = ${JSON.stringify(tbDir)};`)
  .replaceAll('"../../../reader/build.mjs"', JSON.stringify(pathToFileURL(wrapPath).href))
  .replace("const eq = (name, got, want, tol = 0.1) => {", "const eq = (name, got, want, tol = 0.1) => { globalThis.__pend = { k: \"eq\", got, want, tol };")
  .replace("const has = (name, r, level, word) => {", "const has = (name, r, level, word) => { globalThis.__pend = { k: \"has\", level, word };")
  .replace("const count = (name, r, level, want) => {", "const count = (name, r, level, want) => { globalThis.__pend = { k: \"count\", want };")
  .replace(/const ok = \(name, cond(, info = "")?\) => \{/g, (m) => m + ' globalThis.__pend = { k: "ok", got: !!cond };');
const instPath = path.join(tmp, "test_build_inst.mjs"), recsPath = path.join(tmp, "recs.json");
fs.writeFileSync(instPath, inst);
const r = spawnSync("nice", ["-n", "10", "node", "--import", pathToFileURL(path.join(here, "inst_prelude.mjs")).href, instPath], { env: { ...process.env, ZK_RECS: recsPath, ZK_READINGS: path.join(tbDir, "readings") }, encoding: "utf8", maxBuffer: 1 << 28 });
if (!fs.existsSync(recsPath)) { console.error("instrumented run failed", r.stderr); process.exit(1); }
const { recs, calls } = JSON.parse(fs.readFileSync(recsPath, "utf8"));
const zkOk = recs.filter((x) => x.line.startsWith("OK ")).length, zkNg = recs.length - zkOk;

// ---- helpers for case construction ----
const sectionStarts = [];
lines.forEach((l, i) => { if (/^\/\/ /.test(l)) sectionStarts.push({ line: i + 1, text: l.slice(3).trim() }); });
const sectionAt = (ln) => { let s = null; for (const x of sectionStarts) if (x.line <= ln) s = x; return s; };
const commentAt = (ln) => { for (let i = ln - 1; i >= 0; i--) { const m = lines[i].match(/^\s*\/\/ (.*)$/); if (m) return m[1].trim(); if (i < ln - 40) break; } return null; };
const verifyOf = (t) => (t && (t.match(/verify_\w+\.py/g) || [])[0]) || null;
const logName = (line) => line.replace(/^(OK|NG) /, "").replace(/\s{2}.*$/, "").trim();
const callById = new Map(calls.map((c) => [c.id, c]));
const refsOf = (v, acc = new Set()) => { if (Array.isArray(v)) v.forEach((x) => refsOf(x, acc)); else if (v && typeof v === "object") { if (v.$ref) acc.add(v.$ref); else Object.values(v).forEach((x) => refsOf(x, acc)); } return acc; };
const mapRefs = (v, m) => Array.isArray(v) ? v.map((x) => mapRefs(x, m)) : v && typeof v === "object" ? (v.$ref ? { $ref: m.get(v.$ref), $path: v.$path } : Object.fromEntries(Object.entries(v).map(([k, x]) => [k, mapRefs(x, m)]))) : v;
function buildInputs(env, capture, line) {
  const helpers = {}, vars = {};
  for (const [k, v] of Object.entries(env || {})) { if (v && typeof v === "object" && v.$fn) helpers[k] = v.$fn; else vars[k] = k === "here" ? { $builtin: "here" } : v; }
  const need = new Set(), work = [...refsOf(vars)];
  while (work.length) { const id = work.pop(); if (need.has(id)) continue; need.add(id); for (const x of refsOf(callById.get(id).args)) work.push(x); }
  const order = [...need].sort((a, b) => a - b), m = new Map(order.map((id, i) => [id, i + 1]));
  const derived = [];
  for (const [k, v] of Object.entries(vars)) { const d = declOf(k, line); if (d && ENGINE.test(d.rhs) && !(v && typeof v === "object" && v.$ref) && !(v && typeof v === "object" && (v.$engine || v.$builtin))) derived.push(k); }
  return { calls: order.map((id) => ({ id: m.get(id), fn: callById.get(id).fn, args: mapRefs(callById.get(id).args, m) })), env: mapRefs(vars, m), helpers, ...(derived.length ? { derived_from_engine_output: derived.sort() } : {}) };
}
// bend-family inputs depend on ZK's bend conventions (amp / allow / seam): tag them
function conventions(inputs, readingFiles) {
  const txt = JSON.stringify(inputs.calls) + JSON.stringify(inputs.env), tags = new Set();
  let bendy = /"shape":"(bend|miter_bend|wye)"|"joints"|miter_bend|"wye"/.test(txt);
  for (const f of readingFiles) { try { const rd = fs.readFileSync(path.join(tbDir, "readings", f), "utf8"); if (/"shape":\s*"(miter_bend|wye|bend)"/.test(rd)) bendy = true; } catch {} }
  for (const k of ["amp", "allow", "seam"]) if (new RegExp('"' + k + '"').test(txt)) tags.add(k);
  if (bendy) { tags.add("amp"); tags.add("allow"); tags.add("seam"); }
  return [...tags].sort();
}
function gotPath(expr, inputs) {
  const chain = "((?:\\.[A-Za-z_$][\\w$]*|\\[\\d+\\])*)";
  let m = expr.match(new RegExp("^([A-Za-z_$][\\w$]*)" + chain + "$"));
  if (m && inputs.env[m[1]] && inputs.env[m[1]].$ref) return { call: inputs.env[m[1]].$ref, path: inputs.env[m[1]].$path + m[2] };
  m = expr.match(new RegExp('^pc\\(([A-Za-z_$][\\w$]*),\\s*"([^"]*)"(?:,\\s*"([^"]*)")?\\)' + chain + "$"));
  if (m && inputs.env[m[1]] && inputs.env[m[1]].$ref) return { call: inputs.env[m[1]].$ref, path: inputs.env[m[1]].$path + ".pieces", find: { part: m[2], ...(m[3] ? { label: m[3] } : {}) }, then: m[4] };
  return null;
}

const cases = [], excluded = [];
const bump = (o, k) => (o[k] = (o[k] || 0) + 1);
for (const rec of recs) {
  const ok = rec.line.startsWith("OK ");
  const site = rec.sid == null ? null : sites[rec.sid];
  const ln = site ? site.line : 0;
  const name = logName(rec.line);
  const sec = sectionAt(ln), near = commentAt(ln);
  const vs = verifyOf(near) || verifyOf(sec && sec.text);
  const base = { name, source: `${TB_REL}:${ln}`, section_line: sec ? sec.line : 0, origin: { section_comment: sec ? sec.text : null, nearest_comment: near }, basis: vs ? `${vs} (named in the test comment)` : "literal in test_build.mjs; origin not named in the comment", reading_hint: rec.readings, zk_status: ok ? "OK" : "NG" };
  if (!site) { excluded.push({ ...base, reason: "unclassified: no call site" }); continue; }
  const P = rec.pend || { k: "inline" }, a = site.args;
  let c = null, gotExpr = null, lit = { lit: true };
  if (site.inline) { c = { kind: "bool", want: true, zk_got: ok, want_expr: "true" }; gotExpr = a[1]; }
  else if (VERDICT.has(site.fn)) {
    const callText = `${site.fn}(${a.join(", ")})`;
    gotExpr = `__v(() => ${callText})[${JSON.stringify(name)}]`;
    if (site.fn === "stopNeed" || site.fn === "stopOk") c = { kind: "refusal_kind", want: { refusal_kind: a[2], no_exception: true, no_nan: true }, zk_got: ok, want_expr: "true" };
    else c = { kind: "bool", want: true, zk_got: ok, want_expr: "true" };
  } else if (P.k === "eq" && ["eq", "pct", "noPieces"].includes(site.fn)) {
    const wexpr = site.fn === "noPieces" ? "0" : a[2];
    gotExpr = site.fn === "noPieces" ? `((r) => r ? r.pieces.length : -1)(${a[1]})` : a[1];
    c = { kind: "num", want: P.want, tol: P.tol, zk_got: P.got, want_expr: wexpr };
    lit = literalExpr(wexpr, ln);
  } else if (P.k === "has") {
    gotExpr = `(${a[1]}).checks.some((c) => c.level === ${a[2]} && (c.title + c.text).includes(${a[3]}))`;
    c = { kind: "check_text", want: { level: P.level, contains: P.word }, zk_got: ok, want_expr: `${a[2]}, ${a[3]}` };
  } else if (P.k === "count") {
    const m = rec.line.match(/\s(-?\d+)（期待 (-?\d+)）/);
    gotExpr = `(${a[1]}).checks.filter((c) => c.level === ${a[2]}).length`;
    c = { kind: "count", want: P.want, level: /^"[^"]*"$/.test(a[2]) ? JSON.parse(a[2]) : null, zk_got: m ? +m[1] : null, want_expr: a[3] };
    lit = literalExpr(a[3], ln);
  } else if (P.k === "ok") {
    gotExpr = a[1];
    c = { kind: "bool", want: true, zk_got: ok, want_expr: "true" };
  }
  if (!c) { excluded.push({ ...base, reason: "unclassified" }); continue; }
  if (!lit.lit) { excluded.push({ ...base, kind: c.kind, want_expr: c.want_expr, zk_want: c.want, reason: "self-referential: " + lit.why }); continue; }
  const inputs = buildInputs(rec.env, site.capture, ln);
  const conv = conventions(inputs, rec.readings);
  const got = { expr: gotExpr, ...(gotPath(gotExpr, inputs) ? { path: gotPath(gotExpr, inputs) } : {}) };
  cases.push({ ...base, ...c, ...(conv.length ? { convention_dependent: conv } : {}), inputs, got });
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
  fs.writeFileSync(path.join(out, file), JSON.stringify({ zk_commit: zkCommit, section: title, section_source: `${TB_REL}:${sl}`, cases: arr }) + "\n");
  index.push({ file, id, section: title, source_line: sl, cases: arr.length, convention_dependent: arr.filter((c) => c.convention_dependent).length });
});
excluded.forEach((e) => delete e.section_line);
fs.writeFileSync(path.join(out, "excluded.json"), JSON.stringify({ zk_commit: zkCommit, note: "asserts whose expected value is computed by ZK's own engine, or not parseable; not golden", excluded }, null, 1) + "\n");
const kinds = {}; cases.forEach((c) => bump(kinds, c.kind));
const withVerify = cases.filter((c) => c.basis.startsWith("verify_")).length;
const summary = { zk_commit: zkCommit, zk_run: { ok: zkOk, ng: zkNg, total: recs.length }, extracted: cases.length, excluded: excluded.length, by_kind: kinds, basis_names_a_verify_script: withVerify, convention_dependent: cases.filter((c) => c.convention_dependent).length, with_derived_inputs: cases.filter((c) => c.inputs.derived_from_engine_output).length, with_got_path: cases.filter((c) => c.got.path).length, sections: index };
fs.writeFileSync(path.join(out, "index.json"), JSON.stringify(summary, null, 1) + "\n");
console.log(JSON.stringify({ zk_run: summary.zk_run, extracted: cases.length, excluded: excluded.length, by_kind: kinds, basis_names_a_verify_script: withVerify, convention_dependent: summary.convention_dependent, with_derived_inputs: summary.with_derived_inputs, with_got_path: summary.with_got_path, sections: index.length, sites: sites.length }));
if (!process.env.ZK_KEEP) fs.rmSync(tmp, { recursive: true, force: true }); else console.error("kept", tmp);
