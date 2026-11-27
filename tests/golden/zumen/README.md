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
  It can only find marker words; it cannot prove that a number is not from a partner drawing.
- ZK's bend conventions (`amp` = outer swing radius, `allow` = 4 mm per piece, `seam` = 45 deg from the inner side) are documented in ZK's README_en.md
  as matching a partner factory's bend cut sheet. They are NOT taken as values here: L6a derives bend geometry from first principles
  (`verify_miterbend.py`, spec section 2). Cases that depend on them are tagged `convention_dependent` (756 of 1844; e.g. s09 at test_build.mjs:233-240
  and s14 at :656 and :693). Their values hold only under the same named rules (`bend_swing_radius`, `allow`, `seam`); the rules have no public source
  (see `/root/nf-logs/tasks/NFJ-1/FACTORY-RULES-PUBLIC.md`, rules 2 and 4).
- "Independent" here means: the expected number is a literal written into `test_build.mjs`, whose section comment names a `verify_*.py` (1800 of 1844 cases),
  or a closed form written in the test. It is ZK's claim, not proven by us. 8 of the 15 `verify_*.py` print numbers without a pass/fail counter
  (ZK docs/STATUS.md); the numbers frozen from them are only as good as ZK's review. Three shapes (reducing bend, cone branch, wye) were reviewed only by a Qwen session.

## Layout

- `sNN_L<line>.json` : one file per `//` section of `test_build.mjs` (`section_source` = file:line of the section comment). Each case has
  `id`, `name` (ZK's Japanese test name), `source` (`design-demos/assets/kirizu/test_build.mjs:LINE` of the assert call), `kind`, `want`, `tol`
  (ZK tolerance: length 0.1 mm, area/mass 0.05 % of want, per call), `zk_got` (the number ZK printed), `basis`, `origin` (section and nearest comment),
  `reading_hint` (reading files loaded since the previous assert: a hint, not a guarantee), `want_expr` (the expected-value expression as written in ZK),
  `convention_dependent` (present only on cases that depend on the bend conventions above).
- `inputs` : replay record. `calls` = every ZK engine call of the case (`build` / `Z.*`) with its exact arguments, `env` = the values the check reads
  (engine results are `{"$ref": call, "$path"}`, readings are `{"$reading": file}`), `helpers` = the helper functions the check uses, and
  `derived_from_engine_output` = scalars copied from ZK output (191 cases). Those scalars are a hidden ZK dependency: a real engine must recompute them
  (not fixed in this round).
- `got` : `expr` = JavaScript over `inputs` that yields the answer the engine must give; `path` = the engine result path when it is a plain result (672 cases).
- Replay: `node tools/adapter_zk.mjs /root/zumen-kirizu . --all` runs every case against ZK and judges it with ZK's tolerances.
  Result 1844 OK, 0 NG, 0 ERROR (`/root/nf-logs/tasks/NFJ-1/L5/build2-adapter.log`).
- `kind`: `num` (value within `tol`), `count` (exact count of checks of a level), `check_text` (a check of that level whose text contains the word; Japanese text, behavioural),
  `bool` (a condition ZK expects true), `refusal_kind` (builder stops with that check kind, no exception, no NaN).
- `excluded.json` : 50 asserts not taken (see below). `index.json` : counts per section and per kind.
- `readings/` : the 23 ZK reading JSON inputs. `zk_verify/` : the 15 `verify_*.py` and `verify_out.txt` as copied, for reference (not run here).
- `tools/extract_zk.mjs` (+ `inst_prelude.mjs`) : regenerate with `node tools/extract_zk.mjs /root/zumen-kirizu .` (Node 22+; ZK is read only: the test file is copied to a temp dir and instrumented there).
- `tools/check_no_partner.py` : `python3 -I tools/check_no_partner.py`.
- Lane L6a (separate commit): `verify_miterbend.py` here, `docs/shapes/spec_miterbend.md`, `tools/compare_zk_bend.py` (`basis` or `outer` swing argument)
  + `tools/zk_bend_dump.mjs` (heights, widths, masses) + `tools/zk_bend_curve.mjs` (cut-curve points).

## Result of the extraction (ZK commit f718c34)

1894 asserts run; 1844 extracted (1367 `num`, 29 `count`, 148 `check_text`, 214 `bool`, 86 `refusal_kind`); 50 excluded.
Numeric plus count values: 1396. The rest are behavioural expectations written by the ZK authors.

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
| 389 | valve by-pass DXF: line count uses the engine's hole point count | 1 |
| 445 | lateral: hole mark 0 position equals branch centre | 1 |
| 470 | lateral DXF line count uses the engine's hole length | 1 |
| 549 | values derived from engine output (sum, k0; reason in excluded.json) | 3 |
| 571 | pad G2 split in 2: mass equals the unsplit pad | 1 |
| 682 | values derived from engine output (reason in excluded.json) | 3 |
| 693 | reducing bend: swing equals ZK's own swing `a[0].k` | 9 |
| 814, 815 | cone branch written from the other mouth equals the same case | 4 |
| 823 | eccentric flat side and branch rotated together: same hole area | 3 |
| 911 | wye example 3: split mass equals unsplit | 3 |
| 934, 936 | wye main/branch swing equals the bend piece swing | 2 |
| 975 | wye branch mass equals engine summary | 1 |

Not covered at all (not asserts in `test_build.mjs`): the print-only verify scripts' own numbers beyond what `test_build.mjs` quotes
(verify_conebranch, g, g2, lateral, plate, tee, unfold, cone_split), `review_code_fuzz.mjs` fuzz cases, and the DXF writer text.

## Real-engine status (lane S1-L5b)

`crates/nf-core/tests/golden_zumen.rs` now runs `nf_core::shapes` / `nf_core::order` (not reachable from `job::run` yet) against sections s06-s09.
The real engine sees only a case key, never `want`, `zk_got` or `derived_from_engine_output`. Cases with derived inputs (191) are reported
separately and never counted as OK, NG or PENDING; a section that contains one cannot be listed in `PORTED_SECTIONS`.
`check_text` expectations are matched by a stable hold code (table in `eval_s08` / `eval_s09`), not by the Japanese text.
Not covered: all s09 bend cases (46, lane L6b) and tee-main cases (95, lane L7a), s09 0070/0072/0073/0074-0076/0171, s08 0009 (ZK DXF line count) and 0049.
