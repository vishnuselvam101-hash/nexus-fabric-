# NEXUS FABRIC — visual direction (stage 1)

Status: stage 1, 24 Nov 2026, with the founder's palette and theme answers applied. Replaces the Apple direction.
The client looks like the VNX-DNA website in layout, type and 3D/motion feel, with NEXUS FABRIC's own graphite and steel
palette and one molten-orange accent. No DNA imagery or helix concept. Concept: steel parts, flat patterns, unrolling, AMOEBA.

## Constraints
- No build step, no CDN, fully offline. Plain ES modules and CSS in `web/`.
- Fonts: IBM Plex Sans and IBM Plex Mono, latin subset, woff2, in `web/fonts/` (SIL OFL 1.1; `OFL-IBM-Plex.txt`, `NOTICE.txt`).
  Japanese text uses the system stack (Hiragino, Yu Gothic, Noto Sans JP).
- 3D is our own WebGL shader (`web/js/lib/hero.js`). No three.js or GSAP in stage 1.

## Tokens (`web/app.css`, `:root` and `[data-theme]`)
- Neutrals: `--bg`, `--bg-raised`, `--bg-card`, `--bg-inset`, `--border`, `--border-strong`.
- Text: `--text-1`, `--text-2`, `--text-3`.
- One accent: `--accent` with `--accent-soft` and `--on-accent`. Dark accent `#FF6A1A` on graphite `#0E1013`; light accent `#B53F05` (the same hue, darkened so it passes AA as text on white).
- Status colours (`--ok`, `--warn`, `--bad`) carry verdicts only, always with text and an icon.
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

Weights: 400 body, 500 labels, 600 headings (IBM Plex has no 650/800; those map to 600). Mono is for identifiers, codes and labels.

## Palette and contrast (WCAG 2.2, measured 24 Nov 2026)
| Pair | Dark | Light |
|---|---|---|
| text-1 on bg / card | 16.0 / 14.1 | 16.8 / 18.5 |
| text-2 on bg / card | 9.9 / 8.7 | 8.5 / 9.3 |
| text-3 on bg / card | 6.6 / 5.8 | 5.3 / 5.9 |
| accent on bg / card | 6.7 / 5.8 | 5.2 / 5.7 |
| on-accent on accent | 6.7 (`#0E1013` on `#FF6A1A`) | 5.7 (white on `#B53F05`) |
| ok / warn / bad on bg | 7.5 / 8.7 / 6.8 | 5.6 / 5.4 / 6.0 |

Every text pair is at least 4.5 : 1 (AA normal text).

## Theme and language
- Theme follows the OS (`prefers-color-scheme`) when nothing is stored; the manual toggle stays and overrides it.
- Language follows the browser (ja* gives JA, else EN) when nothing is stored or in the URL; the EN/JA switch stays.

## 3D and motion rules
- 3D is used on the home drop zone only in stage 1. Stage 2 adds the living AMOEBA. Work screens stay flat.
- Canvas DPR capped at 1.5. The loop stops when the view is left or the tab is hidden.
- Weak GPU: if most frames in a 120-frame window take over 50 ms, the scene is dropped and the static grid shows.
- No WebGL: the stage shows the static CSS grid (`.drop-stage.no-gl`). Nothing depends on the scene for meaning.
- `prefers-reduced-motion: reduce`: one frame is drawn, nothing animates. The CSS transition/animation kill-switch stays.
- Motion: 140 ms (`--dur`) for hover and focus, 360 ms (`--dur-slow`) for panels, easing `--ease`. No parallax on text, no autoplay sound.

## Do not
- No DNA, helix or biology imagery. No gradients as decoration outside the hero. No stock photos.
- No second accent colour. No CDN fonts or scripts. No animation longer than 1.2 s without user action.
