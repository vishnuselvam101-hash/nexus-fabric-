# Design System (as implemented in `web/styles.css`)

18 November 2026 · Small on purpose: one stylesheet, no component framework. It records the rules the interface follows so that additions stay consistent.

## Principles

1. **The engineering content is the interface.** Geometry, numbers and states come first; chrome is minimal.
2. **Nothing decorative.** No gradients, shadows-as-ornament, glow, glass, illustration or animation. Colour means state.
3. **Nothing invented.** No placeholder data, sample charts or "coming soon" tiles; absent features are stated in one dashed panel per page.
4. **Warnings are sentences, not icons.** A blocked or uncertain result says why in a full-width bar.
5. **State is never colour alone.** Every status has a word.

## Tokens

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` / `--panel` | #f3f5f7 / #fff | #0d1319 / #151d26 | Page / surfaces |
| `--ink` / `--muted` | #16202b / #5d6b7a | #e6edf3 / #93a1b0 | Text / secondary |
| `--line` | #d7dde4 | #283441 | Rules and borders |
| `--accent` | #0b5cad | #5aa9f5 | Actions, links, computed outer geometry |
| `--ok` `--warn` `--bad` (+ `…bg`) | green / amber / red | lighter tints | PASS · review, inconclusive, openings · fail, rejected, open obligation |
| `--side` | #101b27 | #0a0f14 | Navigation |

Type: system sans-serif stack with Japanese fallbacks; 15 px body, 22 px page title, 16 px section title, 12 px uppercase table headings; monospace with tabular figures for every number that is compared and for hashes. Spacing in steps of 4 px (4/8/12/16/20). Radius 3–4 px. One breakpoint at 860 px (navigation moves above content).

## Components

Shell (navigation, top bar with the permanent build notice) · card · stat tile (count + label only) · table (sticky header, numeric columns right-aligned monospace) · status tag (`b-ok`, `b-warn`, `b-bad`, `b-na`) · alert bars (`err`, `info`) · not-implemented panel (`ni`) · form grid · primary and secondary button · pattern drawing (SVG; outer profile in accent, openings in amber) · inline flash message.

## Rules for new work

Use existing tokens; add a token only with a stated meaning · every input has an associated label · every state-changing control gives inline feedback, never a browser dialog · every list has an empty state in words · every destructive or recorded decision asks for its reason in place · escape all output · no third-party scripts, fonts or images (offline factories; content security policy).

## Open

Icon set (none used) · data-visualisation rules beyond the pattern drawing · 2D/3D workspace controls · measured contrast ratios · Japanese typography review.
