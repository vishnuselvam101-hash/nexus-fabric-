// Freeze the reading validation of zumen-kirizu (ZK) as golden cases for the Rust port (crates/nf-core/src/reading.rs, lane PZ-2).
// usage: node extract_validate.mjs <zk_root> <out_file>
// ZK is read only: reader/schema.mjs is imported from <zk_root>, nothing is copied. For each ZK reading
// (design-demos/assets/kirizu/readings, byte-identical to tests/golden/zumen/readings) and for each malformed variant built
// below from a valid reading, it records what ZK's reader/research/review_read_validate.mjs computes: the errors of validate()
// on the reading as written and after normalizeReading() (full lists, sorted, ZK's own Japanese text), the param keys ZK does
// not know, and the params given that are not in the need/check list of the part's shape. A normalizeReading() that throws is
// recorded as such (the Rust port must not panic there).
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const [zk, out] = process.argv.slice(2);
if (!zk || !out) { console.error("usage: node extract_validate.mjs <zk_root> <out_file>"); process.exit(2); }
const { validate, normalizeReading, SHAPE_PARAMS, PARAM_LABEL } = await import(pathToFileURL(path.join(zk, "reader/schema.mjs")).href);
const zkCommit = spawnSync("git", ["-C", zk, "rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();
const dir = path.join(zk, "design-demos/assets/kirizu/readings");
const clone = (x) => JSON.parse(JSON.stringify(x));

// the four outputs of review_read_validate.mjs for one input
function judge(r) {
  const raw = validate(clone(r)).sort();
  let norm = null, normThrows = null;
  try { norm = validate(normalizeReading(clone(r))).sort(); } catch (e) { normThrows = String(e.message); }
  const unk = [], off = [];
  if (r && typeof r === "object" && Array.isArray(r.parts)) {
    for (const p of r.parts) {
      if (!p || typeof p !== "object") continue;
      const params = p.params && typeof p.params === "object" ? p.params : {};
      for (const k of Object.keys(params)) if (!(k in PARAM_LABEL)) unk.push(p.no + ":" + k);
      const s = SHAPE_PARAMS[p.shape];
      if (!s) { off.push(p.no + ": shape " + p.shape); continue; }
      for (const [k, q] of Object.entries(params)) if (q && ![...s.need, ...s.check].includes(k)) off.push(`${p.no}(${p.shape}):${k}`);
    }
  }
  return { raw_errors: raw, norm_errors: norm, norm_throws: normThrows, unknown_keys: unk, off_shape: off };
}

const cases = [];
for (const f of fs.readdirSync(dir).filter((x) => x.endsWith(".json")).sort()) {
  const r = JSON.parse(fs.readFileSync(path.join(dir, f), "utf8"));
  cases.push({ name: f, file: f, ...judge(r) });
}

// malformed variants of one valid reading (G2_tee_claude.json is valid after normalizeReading)
const base = normalizeReading(JSON.parse(fs.readFileSync(path.join(dir, "G2_tee_claude.json"), "utf8")));
if (validate(base).length) throw new Error("the base reading is not valid after normalizeReading");
const variants = [
  ["root_array", () => []],
  ["root_null", () => null],
  ["root_string", () => "reading"],
  ["page_missing", (r) => { delete r.page; }],
  ["page_kind_missing", (r) => { delete r.page.kind; }],
  ["page_kind_bad_enum", (r) => { r.page.kind = "blueprint"; }],
  ["page_qty_string", (r) => { r.page.qty = "1"; }],
  ["page_extra_key", (r) => { r.page.sheet = 2; }],
  ["root_extra_key", (r) => { r.extra = true; }],
  ["material_table_object", (r) => { r.material_table = {}; }],
  ["material_row_mass_missing", (r) => { delete r.material_table[0].mass; }],
  ["material_row_bbox_string_item", (r) => { r.material_table[0].bbox = ["1", 2, 3, 4]; }],
  ["material_row_bbox_null", (r) => { r.material_table[0].bbox = null; }],
  ["parts_object", (r) => { r.parts = { a: 1 }; }],
  ["part_shape_bad_enum", (r) => { r.parts[0].shape = "elbow"; }],
  ["part_no_number", (r) => { r.parts[0].no = 1; }],
  ["part_table_rows_number", (r) => { r.parts[0].table_rows = [1]; }],
  ["part_extra_key", (r) => { r.parts[0].weight = 3; }],
  ["param_value_string", (r) => { r.parts[0].params.D.value = "1016"; }],
  ["param_bbox_missing", (r) => { delete r.parts[0].params.D.bbox; }],
  ["param_source_bad_enum", (r) => { r.parts[0].params.D.source = "guess"; }],
  ["param_bbox_string_item", (r) => { r.parts[0].params.t.bbox = ["1", 2, 3, 4]; }],
  ["param_unknown_key", (r) => { r.parts[0].params.Dx = { value: 1, text: "1", bbox: null, source: "drawing" }; }],
  ["param_off_shape", (r) => { r.parts[0].params.rib_b = { value: 60, text: "b 60", bbox: null, source: "drawing" }; }],
  ["param_null_value_ok", (r) => { r.parts[0].params.L.value = null; }],
  ["params_missing_key", (r) => { delete r.parts[0].params.R; }],
  ["params_null", (r) => { r.parts[0].params = null; }],
  ["params_array", (r) => { r.parts[0].params = []; }],
  ["std_bad_item", (r) => { r.parts[0].std = { item: "valve", text: "JWWA G 118", bbox: null }; }],
  ["std_ok", (r) => { r.parts[0].std = { item: "tee", text: "JWWA G 118", bbox: [1, 2, 3, 4] }; }],
  ["dims_extra_key", (r) => { r.parts[0].dims = [{ label: "開先", text: "V 30°", bbox: null, depth: 2 }]; }],
  ["notes_string", (r) => { r.notes = "note"; }],
  ["unreadable_number_item", (r) => { r.unreadable = [3]; }],
  ["seam_bad_from", (r) => { r.seam = { angle: 45, from: "上", text: "45°", bbox: null }; }],
  ["seam_ok", (r) => { r.seam = { angle: 45, from: "地", text: "地から45°", bbox: [10, 10, 40, 30] }; }],
  ["seam_missing_text", (r) => { r.seam = { angle: 45, from: "天", bbox: null }; }],
  ["attach_missing", (r) => { delete r.parts[0].attach; }],
  ["notes_missing", (r) => { delete r.notes; }],
  ["seam_missing", (r) => { delete r.seam; }],
  ["part_null", (r) => { r.parts.push(null); }],
  ["part_table_rows_null", (r) => { r.parts[0].table_rows = null; }],
  ["unit_mass_bool", (r) => { r.page.unit_mass_kg = false; }],
];
for (const [name, edit] of variants) {
  let r = clone(base);
  const res = edit(r);
  if (res !== undefined) r = res;
  cases.push({ name, input: r, ...judge(r) });
}

const doc = {
  zk_commit: zkCommit,
  tool: "tests/golden/zumen/tools/extract_validate.mjs",
  source: "reader/schema.mjs validate() and normalizeReading(); reader/research/review_read_validate.mjs",
  note: "Errors are ZK's own text, sorted. norm_errors is null when normalizeReading() threw (norm_throws holds the message).",
  cases,
};
fs.writeFileSync(out, JSON.stringify(doc, null, 1) + "\n");
console.log(`${cases.length} cases (${cases.filter((c) => c.file).length} readings, ${variants.length} variants) from ZK ${zkCommit.slice(0, 7)} -> ${out}`);
