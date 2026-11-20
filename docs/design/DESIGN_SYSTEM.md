# Design System (as implemented in `web/styles.css`)

19 November 2026 · Replaces the version of 18 November. One stylesheet, custom properties, no component framework (decision: `FRONTEND_ARCHITECTURE_AUDIT.md`). The tokens are shared with the product's launch film.

## Principles

1. **The engineering content is the interface.** Geometry, numbers and states come first; chrome is thin, dark and quiet.
2. **Nothing decorative.** No gradients, glass, glow or illustration. Shadows only on overlays (toast, popover, dialog). Colour means state.
3. **Nothing invented.** No placeholder data. A capability without a backend appears only as a dashed panel tagged **PLANNED · not implemented in this build**. No control approves or releases a result.
4. **The build status is always on screen**: amber notice in the top bar, in the status bar, on the sign-in screen and as a watermark on the pattern canvas.
5. **State is never colour alone.** Every status tag has a word and a glyph.
6. **Dense, but calm.** 13 px base, 28 px controls, 1 px rules, generous alignment; numbers in tabular monospace.

## Surface decision: dark workstation by default

The default is a graphite workstation. Reasons: the central object is line geometry, which reads best as light strokes on a dark field (as in CAD and CAM tools the users already work in); long sessions; visual continuity with the launch material. A **light theme** is kept as an explicit choice (top bar, `T`, or Account → Preferences) for bright shop-floor offices and for printing; the print stylesheet removes chrome. The chrome (top bar, navigation rail, status bar) is graphite in both themes so the product is recognisable and the build notice keeps one appearance.

## Colour tokens

| Token | Dark (default) | Light | Use |
|---|---|---|---|
| `--bg` | #0E1116 | #EEF1F4 | Application background, input wells |
| `--surface` / `--surface-2` / `--surface-3` | #171B21 / #1D2229 / #262C35 | #FFFFFF / #F5F7FA / #E7EBEF | Panels / raised, hover / pressed, selected |
| `--line` / `--line-2` | #2A313A / #3A434E | #D3D9DF / #AEB7C1 | Rules / stronger separators, overlay borders |
| `--field-line` | #6B7580 | #7D8792 | Borders of inputs, selects and buttons (≥ 3:1 against every surface) |
| `--text` / `--text-2` / `--text-3` | #F5F7FA / #C9CED4 / #8E969F | #0E1116 / #38404A / #59626D | Primary / secondary / tertiary text (white, titanium light, titanium) |
| `--accent` | #2F7DF6 | #2F7DF6 | Engineering blue: focus ring, selection, active navigation, progress |
| `--accent-fill` / `-h` | #2569DB / #1F5CC4 | same | Filled primary button with white text / hover |
| `--accent-text` | #7FB0FB | #1A56C2 | Links and blue text |
| `--warn` | #E8A33D | #855300 | Amber: review required, inconclusive, provisional, planned, the build notice |
| `--ok` | #4CB782 | #156C45 | Pass, discharged, intact |
| `--bad` | #F47067 | #B3261E | Fail, rejected, superseded, open obligation, error |
| `--*-soft`, `--*-line` | 11–16 % tints | 10–16 % tints | Tag and banner backgrounds, their borders |
| `--chrome…` | #0E1116 / #171B21 / #222831, line #2A313A, text #C9CED4 / #F5F7FA / #8E969F | same | Top bar, rail, status bar |
| `--pv-*` | canvas #0A0D11, grid #141920 / #212832, outer #5B9BF8, inner #E8A33D | canvas #FBFCFD, grid #EEF1F4 / #DCE1E7, outer #1A56C2, inner #9A6200 | Pattern viewer: `CUT_OUTER` blue, `CUT_INNER` amber |

### Measured contrast (WCAG 2.1, computed from the token values)

| Pair | Dark | Light | Requirement |
|---|---|---|---|
| `--text` on surface | 16.1 | 18.9 | 4.5 |
| `--text-2` on surface | 10.9 | 10.5 | 4.5 |
| `--text-3` on surface / on `--surface-3` (worst case) | 5.8 / 4.7 | 6.2 / 5.2 | 4.5 |
| `--accent-text` on surface | 7.8 | 6.7 | 4.5 |
| `--warn` / `--ok` / `--bad` text on surface | 8.0 / 6.9 / 6.1 | 6.5 / 6.4 / 6.5 | 4.5 |
| Tag text on its own tint (worst: bad) | 5.1 | 5.0 | 4.5 |
| White on `--accent-fill` / hover | 5.1 / 6.2 | 5.1 / 6.2 | 4.5 |
| Chrome text / muted / group label on chrome | 11.9 / 6.3 / 5.0 | same | 4.5 |
| Focus ring `--accent` against surface | 4.4 | 3.9 | 3.0 (non-text) |
| `--field-line` against surface | 3.7 | 3.7 | 3.0 (non-text) |
| Canvas: outer / inner stroke, ruler text | 6.9 / 9.0 / 6.5 | 6.5 / 5.0 / 6.0 | 3.0 / 3.0 / 4.5 |

`#2F7DF6` itself is not used for text or for filled buttons with white text (4.4 and 3.9): hence `--accent-text` and `--accent-fill`. `--line` (1.3:1) is decorative separation only and never the sole boundary of a control.

## Type

System stack, no web fonts (offline, CSP): `-apple-system, "SF Pro Text", "Segoe UI", "Hiragino Sans", "Noto Sans JP", system-ui, sans-serif`; numbers, identifiers, hashes and coordinates in `ui-monospace, "SF Mono", Menlo, Consolas` with tabular figures.

| Token | Size / weight | Use |
|---|---|---|
| `--fs-4` | 20 / 600, −0.01em | Page title (`h1`) |
| `--fs-3` | 15 / 600 | Workspace title |
| `--fs-2` | 13 / 400–600 | Body, controls, panel titles (`h2` 600) |
| `--fs-1` | 12 | Tables, property lists, meta |
| `--fs-0` | 11 | Smallest size for anything read as text: captions, hints, status bar, rulers, graph labels, table heads; only uppercase tags and graph kind labels use 10.5 |

Line height 1.45. One `h1` per view; `h2` per panel; `h3` for labels inside panels.

## Space, radius, elevation, motion

Spacing scale `--s1…--s6`: 4 · 8 · 12 · 16 · 24 · 32 px. Control height 28 px (32 on sign-in); table rows ≈ 30 px; top bar 44 px; status bar 24 px; rail 232 px or 52 px collapsed.
Radii: `--r1` 2 px (tags, tooltips), `--r2` 4 px (controls, panels), `--r3` 6 px (dialog, popover). Nothing rounder.
Elevation: none on panels; `--shadow` only on toast, popover, dialog.
Motion: `--t-fast` 120 ms (hover, press, focus), `--t-med` 180 ms (overlay and toast entry, tooltip after 350 ms delay), easing `cubic-bezier(.2,0,0,1)`. No motion on layout or data. `prefers-reduced-motion: reduce` disables every transition and animation.

## States

| State | Rule |
|---|---|
| Hover | One step up in surface (`--surface-3`); links underline |
| Focus | 2 px `--accent` outline on every focusable element (`:focus-visible`), inside the canvas edge for the viewer |
| Selected / current | `--accent-soft` fill plus a 2 px accent bar (rail, curve row) or accent underline (tab) — and `aria-current` / `aria-selected` |
| Pressed toggle | `aria-pressed=true`: accent-soft fill, accent text |
| Disabled / busy | 50 % opacity, `not-allowed`; busy buttons show a spinner and a verb ("Computing and verifying…") |
| Invalid | `aria-invalid`, red border, message directly under the field (`role=alert`) |
| Empty | Icon, one-line title, one sentence, at most two actions |
| Loading | Skeleton only if a view takes longer than 200 ms; 2 px progress line under the top bar while any request runs |
| Error | Same shape as empty with a **Retry** action; request failures after an action become a toast and stay in the session log |
| Planned | Dashed border, amber dashed **PLANNED** tag, "not implemented in this build" |

Status vocabulary: `PASS` ✓ green · `INCONCLUSIVE` / `REVIEW REQUIRED` ▲ amber · `FAIL` / `REJECTED` / `OPEN` ✕/▲ red · `NOT IMPLEMENTED` / `SUPERSEDED` – neutral · `PLANNED` ◌ amber dashed.

## Effective job state (one rule, used by every tag, count, banner and watermark)

`superseded` > `no result` > the state the server stores. **NO RESULT** (red ✕) is a job whose input the engine refused: the server stores it as `REVIEW_REQUIRED` with an `outcome` and no pattern; the client never shows it as reviewable, never offers DXF, replay or correction on it, and announces it with an error notification. **SUPERSEDED** jobs carry a red frame and hatch on the canvas, the watermark "Superseded — do not use", strokes at 40 %, and "Open current job #N" as the primary action. Computed numbers use one display rule: three decimals with thousands separators and a unit; below 0.001 scientific; full precision in the hover title. Angles from the engine (radians) are shown in degrees with the radian value beside. Dates are ISO 8601 local time. Notifications: one toast at a time, bottom centre, 4 s (errors 8 s); the session log keeps all.

## Components

Application shell (top bar with breadcrumb, build notice, notifications, help, theme, account, sign-out; collapsible navigation rail in four groups; status bar with connection, engine, build status, open obligations) · page header · panel (header, count, tools, footer) · stat strip · segmented state bar · dense table (sticky head, client-side filter) · property list (`dl.kv`) · status tag · note / banner · planned panel · empty / loading / error states · tabs (ARIA tablist, arrow keys) · collapsible section (`details`) · form field with hint and inline error · buttons (default, primary, danger, ghost, icon) · tooltip (`data-tip`, shown on hover and focus) · toast and notification popover · modal dialog (native `<dialog>`) · resizable split (`role=separator`, drag, arrow keys, double-click to reset) · **pattern viewer** (grid, rulers, extent dimensions, scale bar, selection, hover snap, measurement) · **dependency graph** (drawn, layered, upstream amber / downstream blue) and its narrow-panel form, **dependency layers** (the same layers as wrapped record chips) · **parameter schematic** (to-scale elevation and end view drawn from the form values, dimension highlighted with the focused field) · difference table (before / after / change) · timeline.

Icons: 33 glyphs drawn for this product, 16 px grid, 1.5 px stroke, round caps and joins, `currentColor`, inline SVG, always `aria-hidden` next to a text label or inside a control with `aria-label`.

## Layout and breakpoints

Desktop-first. Fixed viewport grid: top bar · rail + main · status bar; `main` scrolls, or — in the workspace — its three panels scroll independently.

| Width | Behaviour |
|---|---|
| ≥ 1440 | Rail with labels; workspace panels ≈ 17 % · canvas · 27 % |
| < 1440 | Rail collapses to icons with tooltips (user can pin it open) |
| < 1500 | Two-column pages stack; side panels sit side by side below |
| < 1180 | Breadcrumb hidden; stat strip 4 × 2 |
| < 1100 | Workspace: canvas on top (≥ 360 px high), properties and results side by side below, page scrolls; short build notice in the top bar (full text stays in the status bar) |
| < 820 | Single column; role label and secondary status items hidden |
| < 600 | Top bar wraps onto a second line, so the build notice and the account chip stay whole and nothing runs past the right edge (BH-013) |

Verified by the browser test at 1366, 1024 and 768 px: no horizontal overflow, every visible control named and reachable. At 360 px the browser test checks the five main screens for horizontal scroll.

## Keyboard

`?` help · `G` then `D P W M A V S` go to a section · `N` new calculation · `/` focus list filter · `[` rail · `T` theme · viewer: `F` fit, `+` `−` zoom, arrows pan (viewer focused), `M` measure, `1…9` part, `Esc` clear · tabs and splitters: arrow keys. Shortcuts never fire while typing in a field.

## Rules for new work

Use existing tokens; add one only with a stated meaning and a measured contrast · every input has a visible label; every icon-only control has `aria-label` and `data-tip` · every state-changing control gives inline feedback, never a browser dialog · every list has an empty state in words · every recorded decision asks for its reason in place · escape all output (`esc()`) · no third-party scripts, fonts or images · never add a control that sets a result to APPROVED: approval has no backend, and when it does it must be designed with the engineers who carry the responsibility.

## Open

Japanese typography and vocabulary review · touch gestures in the viewer · high-contrast (forced-colors) pass · contrast was computed from token values, not yet audited with assistive technology or real users.

## Round 3

What changed in round 3 (`web/`). Where a section above describes round 2 and differs, this section is the current state.

- **Language.** English and Japanese from one catalogue (`web/i18n.js`). The switch is in the top bar and on the sign-in
  screen; it redraws the screen without a request or a reload. `?lang=` and the saved choice are honoured, and
  `<html lang>` follows. See `LOCALISATION.md`.
- **Route.** The empty route `#/` opens the dashboard. It used to say that the page does not exist.
- **Type.** Three sizes: `--t-s` 11 px, `--t-m` 13 px, `--t-l` 20 px (Japanese 12, 14 and 20 px). Two weights: 400 and 600.
  The five sizes and the 500 and 700 weights are gone.
- **Colour.** One accent, `--signal`, for the cut, the focus and the primary action. Amber only for review required, not
  released, inconclusive and open obligations. Red only for refused, failed, rejected, no result, a replay mismatch, an
  error message and a broken audit chain. Green only for a passed check, a matching replay or an intact audit chain. A superseded job is blocked but neither refused nor failed: its
  banner and stamp are grey. There are no gradients, glows, shadows or purple.
- **Tokens.** Renamed: `--desk`, `--sheet`, `--sheet-2`, `--plate`, `--rule`, `--rule-2`, `--ink`, `--ink-2`, `--ink-3`,
  `--signal`, `--amber`, `--red`, `--green`, and for the drawing `--cv`, `--cut`, `--mark`, `--dim`, `--frame`. One radius
  of 3 px (tags and chips 2 px). Spacing on a 4 px grid.
- **Light theme.** A paper sheet on a grey desk: the chrome is white, the drawing is on warm paper, and the title block
  and the stamp are printed-looking boxes. Status colours are darkened for contrast on white.
- **Icons.** Kept on controls and on the brand mark. Removed from status tags, notes, banners, empty states and drawing
  legends, because they were decoration.
- **The sheet.** The plate has its own fill and a border made of its cut. Every cut is one colour; the weight carries the
  hierarchy (2.25 px, 3.5 px when selected). The grid and the rulers are quieter than the drawing, and a thin sheet frame
  is drawn inside the canvas. Overall dimensions have thin lines, filled terminators and figures centred above their line
  (the vertical figure to the left of its line), with thousands separators. Seams are hatched. The status stamp sits in
  the top band; the title block sits in the bottom right corner, filled only from data the job already shows (part, job,
  engine, marking side, status); the scale bar sits top left.
- **Job header.** The job number and its state are the headline. The second line carries the family, the project, the
  author and the date. At most three actions, one of them primary.
- **Gates.** A verdict list: identifier, name, verdict in a fixed column, the reason in at most two lines, the engine's
  full statement one click away.
- **Numbers.** Right-aligned, tabular figures. A unit sits in a fixed 34 px column in the secondary tone (`.q`), in every
  table, list and read-out that shows a unit.
- **Input rows.** The correction link is quiet: secondary tone, accent on hover or focus. It stays in the DOM and clickable.
- **Motion.** The only motion is the fade of a tab panel (140 ms, ease-out). Reduced motion removes it. There are no hover
  transitions, tooltip fades, pulsing placeholders or spinners; a busy state is shown by its words.
- **Empty and error states.** Rewritten, in both languages, to say what is missing and what to do next where the client
  knows it. No illustrations.
- **Wording.** The sign-in subtitle and the toast for a computed job no longer say "verified". The Japanese equivalents
  were changed to match.
