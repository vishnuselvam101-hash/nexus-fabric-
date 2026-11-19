# Voices — NEXUS FABRIC launch30

All speech was synthesised **locally** in the build container. No text or audio was sent to any
external speech API. Nobody has listened to these files: the build agent cannot hear. Everything
below marked "verified" was verified by measurement or by speech recognition, not by ear.

| Track | Engine | Model / voice | Version | Licence |
|---|---|---|---|---|
| `vo-en.wav` | Kokoro (StyleTTS2-derived neural TTS), run with `kokoro-onnx` on onnxruntime CPU | Kokoro-82M, voice `af_heart` (US English, female) | model v1.0 (`kokoro-v1.0.onnx`, `voices-v1.0.bin`), kokoro-onnx 0.6.1, onnxruntime 1.32.0 | Weights and voices: Apache-2.0 (hexgrad/Kokoro-82M). kokoro-onnx: MIT |
| `vo-ja.wav` | same | Kokoro-82M, voice `jf_alpha` (Japanese, female) | same; text-to-phoneme by misaki 0.7.4 (`JAG2P`, fugashi 1.5.2 + unidic-lite 1.0.8) | Weights and voices: Apache-2.0. misaki: Apache-2.0. fugashi: MIT/BSD. UniDic: BSD-style |

Model files (sha256): `kokoro-v1.0.onnx` 7d5df8ec…36a6c5, `voices-v1.0.bin` bca610b8…1fbf7d,
from the kokoro-onnx GitHub release `model-files-v1.0`.

Licence notes
- Apache-2.0 permits commercial use of the model and its output. `jf_alpha` and `af_heart` carry no
  CC-BY attribution flag in the upstream voice table (some other Japanese Kokoro voices do).
- English text-to-phoneme inside kokoro-onnx uses espeak-ng (GPL-3.0) through `phonemizer` /
  `espeakng-loader` as an unmodified tool. It produces phoneme strings only; no espeak audio is in
  the files. Have legal confirm this is acceptable if the build pipeline itself is redistributed.
- Round-trip checker: faster-whisper 1.2.1 (MIT) with OpenAI Whisper `small` weights (MIT). Not in the audio.

Engines considered: Piper has no Japanese voice, so one engine for both languages was preferred;
MeloTTS and the PyTorch `kokoro` package need a ~2 GB torch install and were not needed. No
espeak-class robotic voice was used.

## Quality — blunt assessment

- **English**: neural, clear, the best-graded Kokoro voice. It is a good synthetic voice, not a
  voice actor: limited emotional range, no direction possible. Measured rates are 1.00–1.07x,
  with some pauses trimmed to 0.22–0.34 s.
- **English line 5** (lead decision): the text was shortened to "From drawings to manufacturing
  intelligence. NEXUS FABRIC. Engineering, reimagined." and the line starts early, 0.33 s after
  line 4 ends: 24.62–29.70 s. Measured speech rate 1.07x (nominal Kokoro speed 1.12), pauses capped
  at 0.22 s. Line 4 was tightened to a measured 1.05x (nominal 1.18) and ends at 24.29 s.
  Kokoro's speed input is non-linear, so limits are enforced on the measured rate
  (speech-only duration at speed 1.0 / speech-only duration of the take); both numbers are in
  `vo-timing.json`. The pacing across lines 4–5 is continuous and tight; it has not been heard.
- **Japanese line 5** also starts early (24.88–29.70 s) at speed 1.00 with no pause trimming.
- **Japanese: not verified as native quality, and probably not native quality.** Upstream grades
  `jf_alpha` C+ and the misaki Japanese front end used here supplies no pitch-accent marks, so
  wrong pitch accent on some words is likely (intelligible, but audibly synthetic to a native
  listener). Readings were checked: the phonemes generated from the kanji text match those from
  the supplied kana readings for all five lines. A native speaker must review before release;
  for a premium launch film a professional Japanese narrator is the safer choice.
- Both voices are 24 kHz models upsampled to 48 kHz: there is no speech content above 12 kHz.

## What was verified (see `checks.json`, `vo-timing.json`, `asr-roundtrip.json`)

- Lines 1–4 start at segment start + 0.25 s and end inside their segment; line 5 starts at least
  0.3 s after line 4 and ends by 29.70 s.
- No silent segment, no clipping (per-line peak −3 dBFS).
- Whisper-small round trip, English: all five lines recovered word-for-word, including
  "Nexus Fabric" in line 5.
- Whisper-small round trip, Japanese: all five lines recovered; the differences are homophone or
  near-homophone choices by the recogniser — 工数→コース, 核とする→拡充する, 制約→製薬, 知恵へ→地位へ, 再定義→最低儀.
  工数 heard as コース and 核とする as 拡充する may also indicate slightly unclear vowel length
  or phrasing there; ask the native reviewer to check those two spots first.
