# NEXUS FABRIC launch film, notch format (30 s, EN and JA)

Built with the `apple-launch-film` skill: the notch at the top centre of the screen opens into one widget per scene and returns to rest between them. One HTML file, `film.html`, with `window.seek(seconds)`; every frame is computed from time.

**Status: draft for the owner's review. Not reviewed by the independent critics. Nobody has listened to the soundtrack. The Japanese captions were written by the lead, not by the localisation specialist.**

## Files
- `film.html` — the film. `?lang=en|ja`, `?reduced=1` (springs become short fades; also follows the system setting).
- `data.js` — real output of the development build for the reference job (header R150, branch r100 at 60°): request, engine, gate counts, the 16 checks with their measured differences, both patterns' vertex lists, the DXF file name, hash, size and an excerpt of its text. Its `source` field says how it was made. Raw files: `assets/job.json`, `assets/header.dxf`, `assets/branch.dxf`.
- `assets/app-workspace.png` — an unedited capture of the running application (1920×1080 at 2×) showing that job.
- `render.js` — `node render.js stills|burst|video [lang]`. Fonts are read from `../launch30/node_modules` (`npm install` there once; licences in `../launch30/FONTS.md`).
- `cues.json`, `audio/make_music.py`, `audio/make_bed.py` — sound: the original score and effects synthesised in code (no samples, no voice), effects timed to `cues.json`; `python audio/make_bed.py` writes `audio/bed.wav`.
- `final/NEXUS-FABRIC-notch-30s-{en,ja}.mp4` — rendered films with sound; `final/watch-*.png` — two frames per second.

## Scenes
| Time | The notch opens into | Caption | Status line |
|---|---|---|---|
| 0–3.2 | "hello", written in one line | none | none |
| 3.2–7.0 | a fictional drawing of the joint | One joint. One curve to find. | Fictional drawing made for this film |
| 7.0–10.8 | the real application, then its pattern canvas | Meet NEXUS FABRIC. | Actual software · development build; "AI drawing interpretation: planned" beside the positioning line |
| 10.8–14.6 | the governing equation with a live read-out | Every curve, from an equation. | Implemented · pattern mathematics |
| 14.6–18.4 | the two flat patterns, plotted from the product's vertices | Rolled steel, developed flat. | Actual product output · development build |
| 18.4–21.7 | the product's 16 checks and its gate summary | Checked, part against part. | Actual product output · 7 gates inconclusive · engineer approval not implemented |
| 21.7–25.0 | the DXF as written: real lines, name, hash | DXF written, re-read, compared. | Actual product output · not released |
| 25.0–30 | (notch at rest) mark, wordmark, line, four words | Engineering, Reimagined. | Product concept. Capabilities in development. Not validated for manufacturing. |

## Decisions taken while building (for the owner to confirm or change)
- **Menu bar has no words.** Menu titles and status items are grey placeholder lines and the left-most glyph is the NEXUS FABRIC mark. A named Mac menu bar would imply a native Mac application, which does not exist, and no Apple logo or trademark is used. The "hello" is the lead's own lettering.
- **Opening caption changed** from "Complex engineering drawings. Hours of engineering work." to "One joint. One curve to find.": the earlier reviewers found the first claim unsupported over a simple joint.
- **No voice-over.** Captions carry the film; planned capabilities are not voiced or shown as present. The only planned item named is AI drawing interpretation, marked as planned beside the owner's positioning line.
- **Wallpaper colours** are new (one hue per scene, as the skill's format asks); the Round 6 film's graphite-only palette is unchanged in `../launch30`.
- **Words on screen that are not product output or approved labels**, listed for approval: the six captions above; "Written / Re-read / Compared" (出力 / 再読込 / 照合); "engine", "marking side", "units", "outside · provisional" (エンジン, けがき面, 単位, 外面・暫定); "Implemented · pattern mathematics"; "Actual product output"; "7 gates inconclusive · engineer approval not implemented"; "not released".

## Checks run on the rendered files
1920×1080, 30 fps, 30.000 s, 900 frames, no decode errors, no black passages, audio and picture start together, about −16 LUFS with peak −1.5 dB, 0 console errors while rendering. Held picture, measured with a strict freeze detector: the finished "hello" (0.8 s), the complete check list (1.0 s) and the end card (2.3 s), all intended. The equation scene moves only a point and its read-out, so a looser detector reads 1.4 s of it as still.

## Known limits
- The checks shown are the product's own; an independent reviewer rated them weak because most invert the product's own construction. The film shows them as they are, with the inconclusive gates.
- The reference job declares equal contact and development radii (an idealised zero-thickness wall); a real set-on joint cannot yet be expressed correctly in the product (review finding RB-001, being fixed). The film shows the product's current output for that idealised case.
- The application has no Japanese interface; the Japanese version says so on screen.
