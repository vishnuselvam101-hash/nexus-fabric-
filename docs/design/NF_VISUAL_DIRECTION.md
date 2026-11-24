# NEXUS FABRIC — visual direction (stage 1)

Status: stage 1, 24 Nov 2026. Replaces the Apple direction. The client looks like the VNX-DNA website in layout, type and
3D/motion feel, with NEXUS FABRIC's own steel and graphite palette and one accent. No DNA imagery or helix concept.
Concept: steel parts, flat patterns, unrolling, AMOEBA.

## Constraints
- No build step, no CDN, fully offline. Plain ES modules and CSS in `web/`.
- Fonts: IBM Plex Sans and IBM Plex Mono, latin subset, woff2, in `web/fonts/` (SIL OFL 1.1, text in `OFL-IBM-Plex.txt`).
  Japanese text uses the system stack (Hiragino, Yu Gothic, Noto Sans JP), as decided by the architect.
- 3D is our own WebGL shader (`web/js/lib/hero.js`). No three.js or GSAP is vendored in stage 1.

## Tokens (`web/app.css`, `:root` and `[data-theme]`)
- Neutrals: `--bg`, `--bg-raised`, `--bg-card`, `--bg-inset`, `--border`, `--border-strong`.
- Text: `--text-1`, `--text-2`, `--text-3`.
- One accent: `--accent` with `--accent-soft` and `--on-accent`. Status colours (`--ok`, `--warn`, `--bad`) are used for verdicts only.
- Spacing: `--sp-1` (4px) to `--sp-8` (64px). Radius: `--r-sm` 3, `--r-md` 6, `--r-lg` 10 px.

## Type scale
| Token | Size | Use |
|---|---|---|
| `--fs-xs` | 11px | mono labels, form labels (uppercase, tracking .04em) |
| `--fs-sm` | 13px | captions, table cells, hints |
| `--fs-md` | 15px | body (base) |
| `--fs-lg` | 18px | verdict badge label |
| `--fs-xl` | 24px | page and card titles |
| `--fs-2xl` | 34px | section titles |
| `--fs-display` | clamp(30px, 4.4vw, 52px) | hero titles only |

Weights: 400 body, 500 labels, 600 headings. Headings use -0.01em tracking. Mono is for identifiers, codes and labels.

## Palette and contrast (WCAG 2.2, measured 24 Nov 2026)
| Pair | Dark | Light |
|---|---|---|
| text-1 on bg | 16.0 : 1 | 16.8 : 1 |
| text-2 on bg | 9.0 : 1 | 7.1 : 1 |
| text-3 on card | 5.4 : 1 (`#8593A3`) | 5.6 : 1 (`#5C6976` on white) |
| text-3 on inset | 6.2 : 1 | 4.9 : 1 (on `#EBEFF3`) |
| accent on bg | 8.1 : 1 (`#35B6E8`) | 5.2 : 1 (`#076F94`) |
| on-accent on accent | 8.0 : 1 | 5.7 : 1 (white on `#076F94`) |
| ok / warn / bad on bg | 7.4 / 8.2 / 5.5 | 4.9 / 4.7 / 5.1 |

Every text pair is at least 4.5 : 1 (AA normal text). Changed in stage 1: dark text-3 (was 4.0 on card), light text-3 (was 3.9 on inset), light accent (was 4.25 on bg).

## 3D and motion rules
- One WebGL scene per page at most: the home drop zone. Canvas DPR capped at 1.5. Loop stops when the view is left or the tab is hidden.
- No WebGL: the stage shows a static CSS grid (`.drop-stage.no-gl`). Nothing depends on the scene for meaning.
- `prefers-reduced-motion: reduce`: one frame is drawn, nothing animates. The CSS transition/animation kill-switch stays in place.
- Motion is 140 ms (`--dur`) for hover and focus, 360 ms (`--dur-slow`) for panels. Easing `--ease`. No parallax on text. No autoplay sound or video.
- The scene never carries status: verdicts are always text plus colour plus icon.

## Do not
- No DNA, helix or biology imagery. No gradients used as decoration outside the hero. No stock photos.
- No second accent colour. No CDN fonts or scripts. No animation longer than 1.2 s without user action.
