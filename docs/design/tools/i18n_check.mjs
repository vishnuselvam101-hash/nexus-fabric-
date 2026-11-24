// Checks the message catalogue web/i18n.js without a browser. Run: node docs/design/tools/i18n_check.mjs
// 1. every key exists in English and in Japanese, and the placeholders of each key are the same in both;
// 2. no Japanese engine or server template carries a bare identifier (an ASCII word of three or more letters
//    outside backquotes: those are shown in monospace by the client, so they are never read as English);
// 3. no Japanese catalogue string carries a bare English-looking word (same rule, same allow-list as tests/ui_smoke.js).
// The module is loaded from a data: URL, so nothing is written to disk. Exit status 1 if 1 or 2 or 3 fails.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const src = fs.readFileSync(path.join(here, '..', '..', '..', 'web', 'i18n.js'), 'utf8');
const { CATALOGUE, PROSE_TABLES } = await import('data:text/javascript;base64,' + Buffer.from(src).toString('base64'));

// the English the Japanese text may keep: the product, its module, file formats, the hash name, the unit, and the DXF file-name prefix NOT-RELEASED
const ALLOW = new Set(['NEXUS', 'FABRIC', 'AMOEBA', 'DXF', 'PDF', 'PNG', 'JPEG', 'TIFF', 'STEP', 'SHA', 'rad', 'eps_num', 'eps_rep']);
const bare = (text) => [...String(text).replace(/`[^`]*`/g, ' ').replace(/NOT-RELEASED/g, ' ').replace(/\{[a-z_0-9]+\}/g, ' ').matchAll(/[A-Za-z][A-Za-z0-9_]{2,}/g)]
  .map((m) => m[0]).filter((w) => !ALLOW.has(w));
const placeholders = (v) => [...new Set(String(Array.isArray(v) ? v.join(' ') : v).match(/\{\w+\}/g) || [])].sort().join(' ');

let failures = 0;
const en = Object.keys(CATALOGUE.en), ja = Object.keys(CATALOGUE.ja);
const missingJa = en.filter((k) => !(k in CATALOGUE.ja)), missingEn = ja.filter((k) => !(k in CATALOGUE.en));
const mismatch = en.filter((k) => k in CATALOGUE.ja && placeholders(CATALOGUE.en[k]) !== placeholders(CATALOGUE.ja[k]));
console.log(`catalogue: ${en.length} English keys, ${ja.length} Japanese keys`);
console.log(`1. keys missing in Japanese: ${missingJa.length}; missing in English: ${missingEn.length}; placeholder mismatches: ${mismatch.length}`);
for (const k of [...missingJa, ...missingEn, ...mismatch].slice(0, 20)) console.log(`   ${k}`);
failures += missingJa.length + missingEn.length + mismatch.length;

let templ = 0;
for (const [table, rows] of Object.entries(PROSE_TABLES)) for (const row of rows) {
  const b = bare(row[1]); if (b.length) { templ++; console.log(`2. ${table}: [${b.join(' ')}] ${row[1].slice(0, 80)}`); }
}
console.log(`2. Japanese engine and server templates with bare identifiers: ${templ}`);
failures += templ;

let cat = 0;
for (const [k, v] of Object.entries(CATALOGUE.ja)) { const b = bare(Array.isArray(v) ? v.join(' ') : v); if (b.length) { cat++; console.log(`3. ${k}: [${b.join(' ')}]`); } }
console.log(`3. Japanese catalogue strings with bare English words: ${cat}`);
failures += cat;

console.log(failures ? `FAILED: ${failures} problem(s)` : 'ok: the catalogue is complete and its Japanese carries no bare English identifiers');
process.exit(failures ? 1 : 0);
