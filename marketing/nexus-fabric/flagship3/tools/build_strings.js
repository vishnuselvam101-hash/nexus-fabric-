// Writes NEW_STRINGS.md from script/onscreen.js (the only source of the film's own sentences).
// node tools/build_strings.js
const fs = require('fs'), path = require('path');
const HERE = path.resolve(__dirname, '..');
global.window = {};
require(path.join(HERE, 'script', 'onscreen.js'));
const OS = window.NF_ONSCREEN;
const cell = (v) => (Array.isArray(v) ? v.join(' / ') : String(v)).replace(/\|/g, '\\|');
const rows = Object.entries(OS).map(([k, v]) => `| ${k} | ${cell(v.en) || '—'} | ${cell(v.ja) || '—'} |`);
const md = `# On-screen text that is not in the narration — flagship film 3

Source of truth: \`script/onscreen.js\` (this file is written from it by \`tools/build_strings.js\`). The narration lines are in
\`script/SCRIPT.md\` and the voice files; the words that appear inside product screens are the application's own and are not listed.
Numbers, names, verdicts, DXF lines, graph labels and gate results are read from the product's output at build time and are not listed.

The Japanese wording has **not been reviewed by a native-speaking engineer**. Product words (state, gate results, quantity
names, tab names) are the application's own strings from \`web/i18n.js\`.

| Key | English | 日本語 |
|---|---|---|
${rows.join('\n')}

Product words the film shows on the product's own screens or restates from them (the application's own strings, web/i18n.js):
PASS, INCONCLUSIVE, NOT RUN, NOT IMPLEMENTED (gate and check results, with 合格 / 未確定 / 未実行 / 未実装); REVIEW REQUIRED (要レビュー);
open (未解決) and discharged (解消済み) (obligation states); Checks (検証項目); Records; Scrutiny; Changes; Graph; the quantity names
Developed width (展開幅), Length (長さ), Area (面積), Opening perimeter (開口の周長), Slant length (斜辺長); the Japanese check names
of the product (data/check-labels.js, 16 of the 26 check names; the other ten keep the machine name, as the product does in Japanese).
The sentence "{n} stored records checked, {f} failing." is the product's own (replay audit), in both languages.

Film-drawn symbols that are not words: the degree marks on the angle axis, the mark (the product's glyph, drawn from its geometry in
\`lib/gfx.js\`), the graph's typed relations (shown as the product returns them), the equation-free plots. The DXF stream
(scene 6) is the file's own text; its NOT-RELEASED file name and annotation are drawn in amber because the product's file carries them.
Amber is used only for REVIEW REQUIRED / NOT RELEASED and for open scrutiny obligations (the product's own state).
`;
fs.writeFileSync(path.join(HERE, 'NEW_STRINGS.md'), md);
console.log('wrote NEW_STRINGS.md with', rows.length, 'keys');
