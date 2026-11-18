# Production and Quality-Control Report — Draft Stage

18 November 2026

## What was produced

| File | Verified properties |
|---|---|
| `drafts/flagship-draft-animatic-en.mp4` | H.264, 1280×720, 25 fps, **54.96 s**, 6.0 MB, no audio track |
| `drafts/flagship-draft-animatic-ja.mp4` | H.264, 1280×720, 25 fps, **54.96 s**, 5.7 MB, no audio track |
| `subtitles/flagship-{en,ja}.{srt,vtt}` | 8 cues each, matching the scene times |
| `drafts/source/` | Animatic page, recording script, capture script, seven real application screenshots |

How: real application screens were captured from the running development build; an HTML page plays the eight scenes on the storyboard timing; a browser recorded it; ffmpeg trimmed and encoded it. Durations were measured with ffprobe. Eight frames of the Japanese version were extracted and inspected.

## Checks on the draft

| Check | Result |
|---|---|
| Duration under 60 seconds | Pass (54.96 s both) |
| Same picture sequence in both languages | Pass (same page, same timing) |
| Captions present and in the right language; Japanese glyphs render | Pass on inspected frames |
| Real and concept scenes distinguishable | Pass: green "actual software" and amber "concept" labels; planned items noted on scenes 3, 5, 6 |
| No invented customers, figures, prices, certifications | Pass |
| Disclaimer on the end card | Pass |
| Legibility of application screens | **Fail for a final film**: full-window captures are too small; needs close crops |
| Scene 4 framing | Weak: the pattern is partly below the frame |
| Audio, synchronisation, music, loudness | Not applicable: silent |
| Japanese wording | **Unreviewed** by a native speaker |
| 4K | Not produced: draft is 720p |

## What could not be produced here, stated plainly

| Requested | Status | Reason |
|---|---|---|
| Photorealistic 3D steel assemblies, lighting, camera moves | Not produced | No 3D renderer is available in this environment |
| Professional voice-overs (English, Japanese) | Not produced | No speech recording or synthesis capability |
| Original music and sound effects | Not produced | No audio generation capability |
| Final 4K flagship films | Not produced | Depend on the three items above and on approval |
| 16 feature films, pricing film, how-to videos, social formats, thumbnails | Not produced | The workflow forbids rendering the campaign before approval; several depend on features that do not exist |

The draft is a timing and content animatic. It is not, and should not be presented as, the cinematic advertisement.

## What is needed to finish

1. Approval of script version (A with labels, or B) and of this storyboard.
2. Native review of the Japanese script.
3. A production route for 3D, voice and music (studio or licensed tools), using these documents as the brief.
4. A logo.
5. Management approval of the package structure before the pricing film.
6. Decision on films about unimplemented features: concept films now, or wait.
