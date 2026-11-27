# Golden values from zumen-kirizu (NFJ-1 lane S1-L5a)

Expected values for the ported cut-pattern shapes, taken from the independent expectations of zumen-kirizu (ZK),
not from values that ZK's own `shared.js` / `build.mjs` computed.

- ZK source commit: **f718c34** (`/root/zumen-kirizu`, "Add status of review fixes and standard table step"). ZK baseline at that commit:
  `node test_build.mjs` prints 1894 OK, 0 NG (re-run by the extractor, see `index.json` -> `zk_run`).
- Files here are versioned by that commit; the Rust runner (`crates/nf-core/tests/golden_zumen.rs`) refuses files from another commit.

## Provenance note

- ZK is the founder's own work (licence and ownership of the code, the tests and the JIS table numbers inside NF are still open: DESIGN D7).
- No partner-factory drawing, image or value was opened for this lane. ZK's `readings/*.json` are hand-made or taken from public
  calculation examples and public standards (each says so in `_note`; `G_tee_claude.json` and `G2_tee_claude.json` are readings of generated public test images).
- `tools/check_no_partner.py` greps the golden folder for partner markers and checks every reading has its "hand made, not a partner drawing" note.
  It can only find marker words; it cannot prove that a number is not from a partner drawing. The three bend conventions (`amp`, `allow`, `seam`) are
  documented in ZK's README_en.md as matching a partner factory's bend cut sheet: they are NOT taken here (L6a derives bend values from first principles).
- "Independent" here means: the expected number is a literal written into `test_build.mjs`, whose section comment names a `verify_*.py` (1816 of 1860 cases),
  or a closed form written in the test. It is ZK's claim, not proven by us. 8 of the 15 `verify_*.py` print numbers without a pass/fail counter
  (ZK docs/STATUS.md); the numbers frozen from them are only as good as ZK's review. Three shapes (reducing bend, cone branch, wye) were reviewed only by a Qwen session.

## Layout

- `sNN_L<line>.json` : one file per `//` section of `test_build.mjs` (`section_source` = file:line of the section comment). Each case has
  `id`, `name` (ZK's Japanese test name), `source` (`design-demos/assets/kirizu/test_build.mjs:LINE` of the assert call), `kind`, `want`, `tol`
  (ZK tolerance: length 0.1 mm, area/mass 0.05 % of want, per call), `zk_got` (the number ZK printed), `basis`, `origin` (section and nearest comment),
  `reading_hint` (reading files loaded since the previous assert: a hint, not a guarantee), `want_expr` (the expected-value expression as written in ZK).
- `kind`: `num` (value within `tol`), `count` (exact count of checks of a level), `check_text` (a check of that level whose text contains the word; Japanese text, behavioural),
  `bool` (a condition ZK expects true), `refusal_kind` (builder stops with that check kind, no exception, no NaN).
- `excluded.json` : 34 asserts not taken (see below). `index.json` : counts per section and per kind.
- `readings/` : the 23 ZK reading JSON inputs. `zk_verify/` : the 15 `verify_*.py` and `verify_out.txt` as copied, for reference (not run here).
- `tools/extract_zk.mjs` (+ `inst_prelude.mjs`) : regenerate with `node tools/extract_zk.mjs /root/zumen-kirizu .` (Node 22+; ZK is read only: the test file is copied to a temp dir and instrumented there).
- `tools/check_no_partner.py` : `python3 -I tools/check_no_partner.py`.
- Lane L6a (separate commit): `verify_miterbend.py` here, `docs/shapes/spec_miterbend.md`, `tools/compare_zk_bend.py` + `tools/zk_bend_dump.mjs` (black-box comparison with ZK bend).

## Result of the extraction (ZK commit f718c34)

1894 asserts run; 1860 extracted (1383 `num`, 29 `count`, 148 `check_text`, 214 `bool`, 86 `refusal_kind`); 34 excluded.
Numeric plus count values: 1412. The rest are behavioural expectations written by the ZK authors.

Excluded (the expected value is computed by ZK's own engine, i.e. a relation between two ZK outputs, not an independent number).
They are useful as invariants for the port, but are not golden values:

| test_build.mjs line | group | n |
|---|---|---|
| 85 | tee main hole centre for e=-100 equals e=100 | 1 |
| 111 | eccentric cone, 3-split mass equals 2-split mass | 1 |
| 174 | rib corner A equals the tee formula `teeY` | 1 |
| 238 | mitered bend bands: right edge equals neighbour's left edge | 10 |
| 252 | big bend: sum of band masses equals unsplit piece mass | 3 |
| 286 | tee main split in 3: mass equals unsplit | 2 |
| 445 | lateral: hole mark 0 position equals branch centre | 1 |
| 470 | lateral DXF line count uses the engine's hole length | 1 |
| 571 | pad G2 split in 2: mass equals the unsplit pad | 1 |
| 814, 815 | cone branch written from the other mouth equals the same case | 4 |
| 823 | eccentric flat side and branch rotated together: same hole area | 3 |
| 911 | wye example 3: split mass equals unsplit | 3 |
| 934, 936 | wye main/branch swing equals the bend piece swing | 2 |
| 975 | wye branch mass equals engine summary | 1 |

Not covered at all (not asserts in `test_build.mjs`): the print-only verify scripts' own numbers beyond what `test_build.mjs` quotes
(verify_conebranch, g, g2, lateral, plate, tee, unfold, cone_split), `review_code_fuzz.mjs` fuzz cases, and the DXF writer text.
