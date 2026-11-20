# flagship2 — progress log (kept current so an interruption loses little)

## Current instruction (coordinator, in force)
- Capture + pack is ONE re-runnable command per language, no hand edits:
  `node capture/capture.js --lang en|ja --server <nf-server binary>` (starts its own copy on a free port with a fresh
  data directory, captures, packs with `capture/pack.py`, removes the raw frames, stops only the server it started).
- Until the coordinator supplies the final binary, the stand-in is `scratchpad/flagship2/nf-server-r3`.
- Deliverables for now: DRAFT films only, `final/draft-en.mp4` and `final/draft-ja.mp4`. No `NEXUS-FABRIC-flagship-50s-*`
  files until the coordinator says so. Then stop and report.
- Graphics bar: typography in three sizes and two weights; one accent colour; no glows or gradients used as decoration;
  every frame composed on a 12-column grid (margins 120 px, gutters 24 px, columns 118 px); a still that looks like a
  generic template gets fixed before the report.
- Honesty rules and the treatment's wording stand (see TREATMENT.md).



## State (latest)
- Footage: `capture/footage/{en,ja}` from nf-server-final, LIGHT theme (`node capture/capture.js --lang <l> --server <binary> --theme light`).
  Frames 1134 (en) and 1136 (ja); all takes; job REVIEW REQUIRED; page language matches the film.
- Film: light look per the owner's style decision (film.html, film.js, lib/engine.js, lib/gfx.js). Checked with 2-second contact
  sheets for both languages and five full-size stills per language (3, 12, 20, 30, 47 s), each opened one by one.
- Music and mix unchanged (audio/bed-*.wav, audio/mix-*.wav). Cue times re-exported from the final film: anchors identical, largest cue shift 42 ms.
- Finals rendered and checked: final/NEXUS-FABRIC-flagship-50s-en.mp4 (23.7 MB) and -ja.mp4 (24.3 MB): 1920x1080, 30 fps, 1500 frames, h264 High, yuv420p, bt709; AAC 48 kHz stereo, 50.000 s; loudness -14.0 LUFS, LRA 3.2 / 3.1 LU, true peak -1.2 / -1.1 dBTP. Eight frames of each looked at. Drafts and raw frames deleted. Unused font subsets pruned (pixel-identical renders checked).
- Known deviations and weaknesses are listed in README.md.

## Latest instructions (coordinator, in force; supersede the earlier look rules)
- Final footage: nf-server-final, captured in the application's LIGHT theme (`capture/capture.js --theme light`), both languages.
- New look (owner's decision): bright and clean, Apple-style product-launch rhythm. Off-white wallpaper (#f5f5f7) with a very
  soft slow cool wash; one large product window (86-92 % of frame width, 18 px corners, soft wide shadow); near-black type
  (#1d1d1f); statement lines 96-132 px, one per scene, centred, 350 ms fade + 24 px rise; small print mid-grey, 22 px minimum;
  steel joint and plates solid and softly shaded; one accent blue for the cut; amber only where the product shows REVIEW
  REQUIRED / NOT RELEASED. Camera: ease-in-out glides, no tilt, no jump in scale between shots.
- Earlier seven review fixes still apply (equation 64+ px, plate labels 28+, DXF text 26+, small print 22+, end card centred).
- Music and mix unchanged. Same script, same voice timing, same shots, same honesty rules.
- Deliverables: final/NEXUS-FABRIC-flagship-50s-en.mp4 and -ja.mp4 (with the mix). Delete the drafts and raw frames at the end.

## Scope (from the first brief, still in force)
One film, 50.0 s, 1920x1080, 30 fps, English and Japanese with identical picture timing. Kokoro voice (English),
VOICEVOX Nemo (Japanese). Live capture: form, result, canvas, checks, records, gates, DXF takes of the real build.

## Done
1. Narration and timing locked: `script/narration.json`, `audio/vo-timing.json`, `audio/timing.js`, `audio/vo-{en,ja}.wav`.
2. Capture rewritten for the r3 build (ids and data attributes only; table `SEL` at the top of `capture/capture.js`).
   Interface language by `?lang=en|ja`, verified against `<html lang>` and `#lang-<lang>[aria-pressed]`.
   The interface check the film names is found by its machine name in the job JSON (`branch_cut_lies_on_header_opening`).
   Takes: form, result, canvas, checks, records, gates, dxf. Explore take removed (the film does not use it).
3. `capture/pack.py` takes `--frames <dir>`.
4. Film engine `lib/engine.js`, graphics `lib/gfx.js`, page `film.html` + `film.js`: NOT YET RUN OK (see next).

## Next (in order)
- Make `film.html?lang=en|ja` run with no errors; `window.seek(t)` deterministic.
- Apply the graphics bar; fix stills; `render.js`; review passes A and B.
- `audio/make_music.py`, `audio/make_mix.py`; loudness numbers.
- Five stills per language, then drafts `final/draft-en.mp4`, `final/draft-ja.mp4`; ffprobe, loudness, 8 frames each.
- Housekeeping: README.md, audio/VOICES.md, NEW_STRINGS.md, .gitignore, size checks. Then stop and report.

## Scratch (not in the repository)
- Stand-in server r3 on port 8893, started by `scratchpad/flagship2/server-r3.sh 8893 data-r3`. Admin / correct-horse-9,
  setup token `flagship2-setup-token-0123456789`. Stop it when done (only ours).
- Old server on 8870 (`nf-server`, pid 1255) and another worktree's server on 8882 are NOT ours: do not touch.

## Known defects / open questions
- The Japanese on-screen wording has not been reviewed by a native-speaking engineer.
