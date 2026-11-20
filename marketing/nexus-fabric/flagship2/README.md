# NEXUS FABRIC — flagship film 2 (50 s, English and Japanese)

Status: **final files rendered** — `final/NEXUS-FABRIC-flagship-50s-en.mp4` and `final/NEXUS-FABRIC-flagship-50s-ja.mp4`
(picture and mix). The footage is from the application's final build (`nf-server-final`), captured in its light theme.
Not a release: the Japanese wording has not been reviewed by a native-speaking engineer (see weaknesses).

The film is a code-rendered motion piece. `film.html?lang=en|ja` draws every frame from time (`window.seek(t)`); the
product screens are real captured frames, cropped and moved, never redrawn. `render.js` photographs the page frame by
frame into H.264 and muxes the mix. The narration, score and mix are made locally in code; no text or audio was sent to
any service.

## Look (the owner's style decision)

Bright and clean, in the product-launch manner: off-white wallpaper (`#f5f5f7`) with a very soft, slow, cool wash; one
large product window (87 % of the frame width, 18 px corners, a soft wide shadow) on the wallpaper; statement lines
at 96–132 px, centred, one per scene, entering with a 350 ms fade and a 24 px rise and leaving before the next; secondary
lines at 32 px under the window; small print at 22 px, mid-grey; near-black type (`#1d1d1f`, Inter 600 and 400;
Noto Sans JP 700 and 400 for Japanese); one accent blue (`#2f6bff`) for the cut and the live values. Steel is solid, softly
shaded light metal with a soft contact shadow on the wallpaper; plates are filled sheets. Amber appears only where the
product's own file carries NOT RELEASED (the DXF file name and its annotation). Motion: critically damped springs
(no overshoot) and ease-in-out glides; the camera has no tilt and always covers the window, so the scale never jumps
between shots. The scene timing keeps the voice slots (`audio/vo-timing.json`); each statement is held at least 1.2 s.
The motion principles were taken from the `apple-launch-film` skill (retargetable springs, short text, no claims); its
notch and menu-bar concept, and any Apple logo, font, product image or wording, are not used.

## Files

| Path | What it is |
|---|---|
| `film.html`, `film.js`, `lib/engine.js`, `lib/gfx.js` | the film: timeline, camera, footage player, drawn graphics |
| `render.js` | renders frames (`video`), stills (`stills`) or the sound cues (`cues`) |
| `capture/capture.js` | one command per language: starts a copy of the application, drives the takes in the light theme, packs them, removes the raw frames, stops the copy it started |
| `capture/pack.py` | packs the frames into lossless WebP atlases and `footage.js` (the job JSON and the DXF text are inside) |
| `capture/footage/<lang>/` | the packed footage (about 6 MB per language) |
| `script/narration.json` | every spoken line and every on-screen sentence, both languages (source of `narration.js`) |
| `tools/build_script.py` | writes `script/narration.js` from the JSON |
| `audio/vo-{en,ja}.flac` | the narrations on the film timeline (50.0 s, mono, 48 kHz; the WAV copies are rebuilt from these by `make_mix.py`) |
| `audio/vo-timing.json`, `audio/timing.js` | the locked voice slots the picture is cut to |
| `audio/cues-{en,ja}.json` | the sound cues exported from the film (`render.js cues`) |
| `audio/make_music.py` | the score and the effects, synthesised in code, into `audio/bed-<lang>.wav` |
| `audio/make_mix.py` | voice over bed, levels, mastering; writes `audio/mix-<lang>.wav` and `audio/mix-report.json` |
| `audio/make_voice.py`, `audio/nf_tts.py`, `audio/audition.py` | how the narrations were made and auditioned (the engines are needed) |
| `audio/VOICES.md` | the voices, and the licence and credit text quoted from the installed files |
| `NEW_STRINGS.md` | every on-screen sentence the film sets, both languages, with its size and place |
| `TREATMENT.md` | the idea, the narration, the shot list and the honesty rules this film follows (the look is superseded by the section above) |
| `final/NEXUS-FABRIC-flagship-50s-{en,ja}.mp4` | the finals (picture and mix) |
| `data/` | an earlier build's product output; the film does not read it (ignored by the repository root's `.gitignore`) |
| `fonts/` | Inter (Latin, Greek), Noto Sans JP (the 27 subsets the Japanese film uses, 400 and 700), as woff2; the stylesheet lists only these |

## How to rebuild

Requirements: Node with the global `playwright` package and a Chromium it can launch; Python with numpy, scipy,
soundfile and Pillow (the venv `~/.venvs/nf-flagship2` has them); ffmpeg and ffprobe. Paths are from the flagship2 folder.

1. Footage, one command per language (starts its own copy of the application on a free port with a fresh data
   directory, drives the takes, packs them, removes the raw frames, stops the copy it started):
   ```
   node capture/capture.js --lang en --server <path to the nf-server binary> --theme light
   node capture/capture.js --lang ja --server <path to the nf-server binary> --theme light
   ```
   The interface language is set by `?lang=`, and the theme by the application's own toggle (`#theme`); the script
   refuses to continue if either is not as asked. The selectors are ids and data attributes only (table `SEL` at the top).
2. Script and cues:
   ```
   python3 tools/build_script.py
   node render.js cues en audio/cues-en.json
   node render.js cues ja audio/cues-ja.json
   ```
3. Sound (the music and the mix of this film are unchanged from the earlier build):
   ```
   ~/.venvs/nf-flagship2/bin/python audio/make_music.py
   NF_SCRATCH=<a scratch folder> ~/.venvs/nf-flagship2/bin/python audio/make_mix.py
   ```
4. Picture and mix:
   ```
   node render.js video en final/NEXUS-FABRIC-flagship-50s-en.mp4 --audio audio/mix-en.wav
   node render.js video ja final/NEXUS-FABRIC-flagship-50s-ja.mp4 --audio audio/mix-ja.wav
   ```
   Frame f shows film time f/30 s (1500 frames). libx264, crf 15, preset slow, yuv420p, bt709 matrix and tags; AAC
   stereo 48 kHz.
5. Review: `node render.js stills en <dir> 3,12,20,30,47` (full-size PNG at the given times).

The narrations are locked; rebuilding them needs the Kokoro and VOICEVOX engines and models (`audio/VOICES.md`).

## What is real footage, what is illustration

Real footage (frames photographed from the running application in its light theme; every value on those screens is the
application's): the form (3), the result (4), the canvas (5), the checks (7 after the fold), the records (8), the gates (9)
and the DXF download (10). The numbers over them come from the same job: the form values, the schematic (read from the
application's SVG at capture time), the vertices of the patterns (721 vertices of the saddle cut, read from the job's API
result), the check rows, the gate summary (5 pass, 7 inconclusive, 1 not implemented), the record count and the closure
hash, and the DXF text (downloaded from the API and parsed in the page). The pointer is drawn by the film at the recorded
positions of the real pointer events, because headless browsers do not paint one.

Illustration made for the film, labelled on screen as "Visualisation made for this film · not a product view": the joint
and the line of light (shot 1), the plates and their fold (shot 7), the curve's plotting in shot 6 and the mark morph in
shot 10–11. The equation's curve is plotted from the application's vertices at the same place.

The mark (the lockup in shots 11–12) is drawn from coordinates typed in `lib/gfx.js`. **They were not extracted from the
application's SVG** and must be checked against it before release.

## Measured figures (the finals, after the AAC encode)

| | NEXUS-FABRIC-flagship-50s-en.mp4 | NEXUS-FABRIC-flagship-50s-ja.mp4 |
|---|---|---|
| video | H.264 High, 1920×1080, 30 fps, 1500 frames, yuv420p, bt709 (matrix, primaries, transfer), limited range | the same |
| audio | AAC LC, 48 kHz, stereo | the same |
| duration | 50.000 s (video 1500 frames; audio 50.000 s) | 50.000 s |
| size | 23.7 MB | 24.3 MB |
| integrated loudness | −14.0 LUFS | −14.0 LUFS |
| loudness range | 3.2 LU | 3.1 LU |
| true peak | −1.2 dBTP | −1.1 dBTP |

The mix (`audio/mix-report.json`, measured on the stems before the encode; unchanged): the voice is 12.5 LU above the bed
(the bed measured without the side-chain duck, the stricter case); the bed is ducked 4 dB under the voice while a word
sounds. The master gain is +8.5 dB (en) and +9.1 dB (ja); the limiter's largest reduction is 5.1 dB (en) and 1.3 dB (ja),
at the loudest words. Both mixes are 50.0 s, 48 kHz, 24-bit, stereo, with no clipped samples.

The sound cues of the new picture were compared with the cues the bed was made from: every anchor is identical, and the
largest shift of any cue is 42 ms (about one frame). The bed was not rebuilt.

## Known weaknesses (stated plainly)

1. **The Japanese on-screen wording has not been reviewed by a native-speaking engineer.**
2. **The joint fills about half the frame width (about 1030 of 1920 px), not the 70 % asked for.** The statement is centred
   above it, and the joint's proportions and the height available leave no room for a wider joint in that position.
3. **Shot 9 (gates): the development-build banner and the REVIEW REQUIRED state tag are out of view for most of the shot**;
   the window shows the gate list. The state tag comes into view at the end of the shot (about 32.5 s).
4. **Statement lines cannot sit over a product scene.** The window takes 87 % of the frame, so product scenes carry a
   32 px line and a 22 px label under the window. Statements (96–132 px) appear only on the wallpaper scenes (the joint,
   the fold, the DXF, the end card). The 48–56 px caption size of the earlier look no longer appears.
5. **The fold (shot 7) is mid-roll at about 20 s.** The plates roll between about 19.4 s and 21.6 s; a still in that window
   shows a sheet half rolled. The tilted window of shot 2 (the pull-back) was removed by the look rules.
6. **The interface check is shown as the product's own Checks tab**, after the fold; the film no longer draws a check panel.
7. **The English master limiter works hard:** up to 5.1 dB of reduction on the loudest words (true peak −1.2 dBTP,
   loudness −14.0 LUFS, range 3.2 LU). The reduction is on peaks only; the effect was not listened to in this pass.
8. **The mark is hand-entered** (see above).
9. **The camera keys are tuned to the final build's layout.** A different layout needs the zoom and focus values in
   `film.js` (`CAM`) checked again. The window coverage is enforced in code (`clampCam`), so no black edge appears.
10. **The sound cues are the earlier ones** (within 42 ms of the picture); the music and the mix were kept as instructed.
11. **Footage packing was not verified bit-for-bit.** `capture/pack.py --verify` needs the raw frames, which are deleted
    after packing. The atlases are lossless WebP by design.
12. **The fonts were pruned to the subsets the film loads** (measured over the whole timeline, both languages). A new
    text, or a glyph outside those subsets, would fall back to a system font: re-run the survey after changing text.

## Folder size

Git keeps about 69 MB (the limit is 80 MB): the finals 48 MB, the footage about 13 MB, the fonts about 1.3 MB, the voices
(FLAC) about 6 MB, the rest small. The whole folder on disk is about 141 MB (the limit is 400 MB); the ignored parts are
the bed and mix WAVs and the voice WAVs (all rebuildable). `.gitignore` lists them.
