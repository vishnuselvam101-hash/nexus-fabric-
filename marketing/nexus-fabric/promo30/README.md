# NEXUS FABRIC promo, built on real application screens (30 s, EN and JA)

Made by applying the "SaaS promo" recipe from the motion-graphics guide by D. Caudullo: open with a short hook, show each real screen and move in on the part that matters with one short line, bold words between the screens on a steady beat, end on the logo, one line and a call to action. The guide's renderer (HyperFrames) was not installed; this uses the project's existing equivalent (Chromium frames joined by FFmpeg).

**Status: draft for the owner's review. Not reviewed by the independent critics. Nobody has listened to the soundtrack. The Japanese text was written by the lead, not by the localisation specialist.**

## Files
- `film.html` — the film; `window.seek(seconds)`, `?lang=en|ja`.
- `screens/*.png` — five unedited captures of the running development build (engine 0.3.0), 1920×1080 at 2×: new calculation with its parameter schematic, the job with the header pattern and gates, the checks tab, the records tab, the branch pattern. `screens/rects.json` holds the measured positions of the highlighted elements. Nothing on a screen is redrawn; the camera only moves, and a blue frame marks the element a caption refers to.
- Caption counts (9 numbers, 16 checks, 39 records) are read from the same job's data in `../notch30/data.js`.
- `render.js`, `cues.json`, `audio/make_music.py`, `audio/make_bed.py` — as in `../notch30`; `NF_LUFS=-14 python audio/make_bed.py` writes `audio/bed.wav` (the guide's loudness target for social video).
- `final/NEXUS-FABRIC-promo-30s-{en,ja}.mp4`, `final/watch-*.png` (two frames per second).

## The cut
| Time | Picture | Words |
|---|---|---|
| 0–3 | three word slams | ENTER THE NUMBERS. / GET THE FLAT PATTERN. / SEE EVERY CHECK. |
| 3–7.5 | new-calculation screen, in on the form and its to-scale schematic | Nine numbers in. One joint. |
| 7.5–8.5 | slam | DEVELOPED. |
| 8.5–14.5 | the job: the opening on the header plate, the curve table, the branch pattern | The flat plate, with its opening. / Every curve, measured. / Both parts of the joint. |
| 14.5–15.5 | slam | CHECKED. |
| 15.5–20.5 | checks tab, then the gate summary | 16 checks, line by line. / Every gate shown, pass or not. |
| 20.5–21.5 | slam | ON RECORD. |
| 21.5–25 | records tab with the replay result, then the DXF control | 39 records. Replay matches. / DXF out, marked not released. |
| 25–30 | end card | NEXUS FABRIC · Engineering, Reimagined. · Request a demonstration · disclaimer |

## Decisions and limits
- Only implemented capabilities are shown, so no "planned" marks are needed; the owner's positioning line (the engineering platform with automated drawing reading, powered by AMOEBA) is **not** in this cut because nothing on these screens is automated drawing reading.
- Every screen carries "Actual software · development build"; the application's own "not validated" banner and "NOT RELEASED · REVIEW REQUIRED" watermark stay visible where the camera passes them.
- The job shown is the idealised zero-thickness reference case; the gate summary in frame (5 pass, 7 inconclusive, 1 not implemented) is the product's own.
- While the camera is moving, panel text at the frame edge is cut, as in any move across a screen; each resting frame was set on panel boundaries.
- Checks on the rendered files: 1920×1080, 30 fps, 30.000 s, 900 frames, no decode errors, no black passages, about −14 LUFS with peak −1.2 dB, 0 console errors. Held picture of a second or more: only the end card.
- A 9:16 version was not made.
