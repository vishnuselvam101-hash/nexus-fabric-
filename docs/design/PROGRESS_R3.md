# Interface round 3 — progress

Working file of the frontend lead for round 3 (English and Japanese; premium workstation design). Branch
`worktree-agent-a67576d456572b8c9`, on top of `f52d8d3` (the round-3 client, before this pass). Updated at the
commit that ends this pass, so that an interrupted session can be resumed from the repository alone.

## Done

| Step | State |
|---|---|
| Read the design system, the architecture audit, critic report R1, the engine and server text sources | done |
| BEFORE screenshots, English, 1920 and 1366 (138 files): `docs/design/screenshots/before-r3/` | done (earlier commit) |
| Message catalogue, English and Japanese, 803 keys each, placeholders equal (`web/i18n.js`) | done |
| Client on the catalogue: every string through `t`/`h`; the engine and server text through exact templates | done (`f52d8d3`) |
| `#/` and an empty hash open the dashboard (they showed "page does not exist") | done; browser check added |
| Japanese checks in `tests/ui_smoke.js`, section 14b: switch without reload, `<html lang>`, persistence, `?lang=`, the empty route, 32 Japanese states walked for English words, `missing()` empty, overflow at 1366/1024/768, console errors | done |
| Defects the Japanese walk found, fixed: identifiers now monospace (sketch symbols, curve names, quantity keys, part classes, record kinds, gate IDs); engine templates with bare identifiers marked; user names in breadcrumbs wrapped; engine sentences with `eps_num`, `binary64`, `theta`, `phi`, `CAM` and similar marked | done |
| Design pass (the coordinator's ten points, see below), five critique rounds on the job view, four on the dashboard, three on the new-calculation form and the gates tab | done, partly (see below) |
| `docs/design/screenshots/after-r3/`: 12 files (sign-in, dashboard, new calculation, job pattern, job gates, job records; English and Japanese; 1920×1080; dark), reduced with `png_reduce.py`, 1.1 MB, worst palette error 6/255 | done |
| `docs/design/LOCALISATION.md` (under 120 lines) | done |
| `docs/design/DESIGN_SYSTEM.md`: a Round 3 section describing what changed | done |
| `docs/design/tools/i18n_check.mjs`: key parity, placeholder parity, bare English in Japanese; no browser, no temporary files | done, passes |

## The ten points

1. **`#/` opens the dashboard.** Fixed. The router took the default only for a missing segment; an empty one slipped through.
2. **Canvas text and the watermark in Japanese.** Fixed for this sandbox. Canvas text uses the page's sans stack with the
   Japanese faces after the Latin one; the stamp "未リリース · 要レビュー" renders with its long-vowel mark. Partly: only
   IPA Pゴシック and WenQuanYi are installed here, so the Hiragino, Yu Gothic and Noto faces of macOS, Windows and common Linux
   setups were not seen.
3. **The canvas as the hero.** Fixed: the plate has its own fill and an outline of its cut; every cut is one colour, weighted
   (2.25 px, 3.5 px selected); overall dimensions are thin lines with filled terminators, figures centred above their line,
   thousands separators; the grid and the rulers are quieter; a sheet frame; the title block in the bottom right holds only
   part, job, engine, marking side and status; the stamp is in the top band. Widths: at 1920 the sheet is about 920 px against
   panels of 300 and 460; at 1366 about 640 px against 260 and 400.
4. **Hierarchy.** Fixed: three sizes and two weights everywhere (CSS audited: no other size or weight); the job number and the
   state are the headline; the description, author and date are the second line; at most three actions, one primary.
5. **Noise.** Fixed: the correction link is quiet (secondary tone, accent on hover or focus; still in the DOM and clickable); the
   identifier under each label is small, monospace, one tone.
6. **Numbers.** Partly: tabular figures and right alignment in tables, read-outs, the request list and the change table; a unit
   sits in a fixed 34 px column in those places. Dashboard counts have no units; some older read-outs still print plain numbers.
7. **Colour discipline.** Fixed: amber only for review required, not released, inconclusive and open obligations; red only for
   refused, failed, rejected, no result, mismatch, errors and a broken chain; green only for a passed check, a matching replay or
   an intact chain. Superseded is blocked but neither refused nor failed, so its banner and stamp are grey. No gradients, glows,
   purple or decorative shadows. Icons kept on controls, the brand mark and the search field only.
8. **Light theme.** Fixed: a paper sheet on a grey desk; screenshots taken at 1920 and 1366 (scratch set, not committed).
9. **Empty and error states.** Partly: the states that say what is missing (no jobs, no projects, no pattern, job or page not
   found, unreachable server, failed view, no dependency edges, no DXF) were rewritten in both languages with a next step; other
   empty states keep their wording. No illustrations.
10. **Motion.** Fixed: the only motion is the fade of a tab panel (140 ms, ease-out); reduced motion removes it. Hover transitions,
    tooltip fades, pulsing placeholders, spinners and the sliding progress bar are gone. Nothing bounces.

## Critique rounds (what the critic attacked, then what changed)

- **Job view, five rounds.** R1 (English, dark, 1920): the title block sat under the plate, the stamp covered the vertical
  dimension, the results tabs were cut at "Records", the request units were clipped. Fixed: block and stamp given their own bands,
  wider results pane. R2: the block covered the horizontal dimension; the middle dot in "reducer · Eccentric reducer". Fixed:
  bottom margin, family on its own line. R3 (English 1366, Japanese 1920): the build notice was cut mid-sentence at 1366; identifiers
  broke onto their own lines in Japanese labels; the status bar cut the notice at 1024. Fixed: short notice below 1500 px, identifiers
  inline, engine id hidden at 1180 px and below. R4 (light 1920, gates 1366): the drawing had no sheet border. Fixed: a frame.
  R5 (records, superseded and corrected jobs): summary labels wrapped word by word; superseded shown in red; request units not in the
  unit column. Fixed: label column share, neutral superseded, unit column.
- **Dashboard, four rounds** (English dark, Japanese dark, light, Japanese 1366): "No result" now red; the page no longer stops at 1560 px;
  identifiers in the parts column inline.
- **New calculation, three rounds** (English dark, English light, Japanese 1366): the Japanese view titles were clipped; the schematic now
  draws in a viewBox and scales to its cell; the input column narrowed so that "展開半径 r_dev" fits on one line.
- **Gates tab, three rounds**: it shares the job layout; the fixes above apply.

Claims removed from the text (the brief forbids them): the sign-in subtitle said "Verified flat patterns"; the toast for a computed job
said "computed and verified"; "independent verification" is now "independent checks"; "Immutable records" is "Stored records"; the
Japanese equivalents changed to match. The module header claimed the Japanese terms were "approved"; it now says they have not been
reviewed.

## Open points and known limits

- The Japanese terminology has not been reviewed by a native-speaking fabrication engineer.
- Japanese glyphs in the screenshots come from IPA Pゴシック (the only Japanese UI face installed in the build sandbox).
- The screenshot set is at 1920×1080 and dark, as the brief asks; the light and 1366 captures were reviewed but are not committed.
- `docs/design/UI_NEEDS_FROM_SERVER.md` (listed in the round-3 plan) was not written; no server change was needed for this pass.
- Gate vocabulary and engine text are unchanged; the engine's own wording stays English where it has no reviewed template.
- `tests/security_r6.sh` is an audit whose checks are expected to fail while its findings are open; it was not run as a gate.

## Verification record

Run on the committed tree: the release build of the workspace, and the debug binary the two shell suites build for themselves.

| Check | Result |
|---|---|
| `cargo build --release` (workspace) | exit 0 |
| `cargo test --workspace` | exit 0; 98 tests in the crate test binaries, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `bash tests/e2e_smoke.sh` | 89 passed, 0 failed |
| `bash tests/security_smoke.sh` | 66 passed, 0 failed |
| `tests/ui_smoke.js` on a fresh server, run 1 / 2 / 3 | 151 passed, 0 failed, each run |
| `node docs/design/tools/i18n_check.mjs` | 803 keys in each language; no problem |

The browser suite's 151 checks are the 108 English checks of round 2, unchanged, plus the Japanese section (14b). The check
"obligation: waived with a reason" passed in all three runs. No check was weakened, skipped or deleted: `tests/ui_smoke.js`
has 88 added lines and no removed line.

Servers this pass started were all stopped. Not started by this pass and left running: an `nf-server` on port 8882 from this
worktree's binary (data directory under the shared scratchpad), and two servers of another session.