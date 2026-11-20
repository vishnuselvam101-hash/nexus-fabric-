# Interface round 3 — progress

Working file of the frontend lead for round 3 (English and Japanese; premium workstation design). Updated at every commit so that an interrupted session can be resumed from the repository alone. Branch `worktree-agent-a67576d456572b8c9`, based on `50b2c61`.

Order of value agreed with the coordinator: (1) a complete, tested English/Japanese switch; (2) the gates tab, the pattern canvas and the wrapping defects; (3) the remaining screens.

## Done

| Step | State |
|---|---|
| Read the design system, architecture audit, critic report R1, master goal, build guide, engine review B (RB-002, RB-003), the engine and server sources for every text they send | done |
| BEFORE screenshots of every screen and state, English, 1920×1080 and 1366×768 (138 files) with load metrics: `docs/design/screenshots/before-r3/` | done, committed |
| Capture tool for both languages and every state: `docs/design/tools/capture_r3.js`; PNG reduction with a stated worst error: `docs/design/tools/png_reduce.py` | done, committed |
| `web/i18n.js`: message catalogue (796 keys, English and Japanese, placeholders checked equal), Japanese labels for 230 identifiers the engine and server send, exact-match templates for engine and server prose (all 98 distinct engine texts of the demonstration data match; numbers pass through unchanged) | written and committed; **not yet embedded or imported**: the client in this commit is still the round-2 client |

Baseline measured on the unchanged client (`before-r3/metrics.json`): assets 190,308 bytes (html 666, app.js 135,518, viewer.js 16,143, css 37,981); DOMContentLoaded 54 ms; first view rendered 138 ms (dashboard) and 138 ms (heaviest job), medians of 9 cold loads on loopback.

## In hand (not yet committed as part of the client)

- `web/app.js`: rewritten so that every client string goes through the catalogue, views render from loaded data (a language switch re-renders without a request and keeps form input), gates as a verdict list with the full text one click away. In progress.
- `web/viewer.js`: crisp rulers with ticks and cursor marks, curve names and seam hatching from the engine's curve names, one number format, state save and restore. Written, waits for the new `app.js`.

## Next

1. Finish `app.js`, `styles.css`, `index.html`; embed `i18n.js` (one route in `crates/nf-server/src/main.rs`, as `viewer.js` was added).
2. Run the existing 108 browser checks in English; extend `tests/ui_smoke.js` for the language switch, persistence, `?lang=`, the Japanese DOM walk, overflow at three widths in Japanese, console errors in both languages.
3. Screen-by-screen design pass with screenshots at 1920, 1366, 1024 and 768 in both languages and the light theme.
4. `docs/design/LOCALISATION.md` (translated / glossed / left in English; glossary with confidence), `docs/design/UI_NEEDS_FROM_SERVER.md`, update `DESIGN_SYSTEM.md`.
5. AFTER screenshots in both languages into `docs/design/screenshots/after-r3/`; before/after asset sizes and first-view time.
6. Full verification: `cargo build --release -p nf-server`, `cargo test --workspace`, `tests/e2e_smoke.sh`, `tests/security_smoke.sh`, the browser suite.

## Known defects and open points

- None introduced yet: the client in the repository is still the round-2 client.
- The screenshots are reduced to a 256-colour palette (worst error 8 of 255 per channel, measured by `png_reduce.py`); they are evidence of layout and content, not colour references.
