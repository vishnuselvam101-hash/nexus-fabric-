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
  voice actor: limited emotional range, no direction possible. Lines 1–4 are at natural rate
  (speed 1.00–1.08, some pauses trimmed to 0.26–0.34 s).
- **English line 5 does NOT fit at a natural rate.** "From engineering drawings to manufacturing
  intelligence. NEXUS FABRIC. Engineering, reimagined." needs about 6.0 s of pure speech at the
  voice's normal pace; the slot is 4.55 s. It was synthesised at model speed 1.36 (native rate
  control, not time-stretching) so that it ends at 29.55 s. It will sound rushed, and the ASR
  round-trip heard the brand name there as "Next is fabric". Fix the script (for example
  "Drawings to manufacturing intelligence. NEXUS FABRIC. Engineering, reimagined.") or give the
  scene more time, then re-run.
- **Japanese: not verified as native quality, and probably not native quality.** Upstream grades
  `jf_alpha` C+ and the misaki Japanese front end used here supplies no pitch-accent marks, so
  wrong pitch accent on some words is likely (intelligible, but audibly synthetic to a native
  listener). Readings were checked: the phonemes generated from the kanji text match those from
  the supplied kana readings for all five lines. A native speaker must review before release;
  for a premium launch film a professional Japanese narrator is the safer choice.
- Both voices are 24 kHz models upsampled to 48 kHz: there is no speech content above 12 kHz.

## What was verified (see `checks.json`, `vo-timing.json`, `asr-roundtrip.json`)

- Each line placed at segment start + 0.25 s and ends before the next segment (margins 0.22–1.15 s).
- No silent segment, no clipping (per-line peak −3 dBFS).
- Whisper-small round trip, English: all five lines recovered word-for-word except
  "NEXUS FABRIC" → "Next is fabric" in the fast line 5.
- Whisper-small round trip, Japanese: all five lines recovered; the differences are homophone or
  near-homophone choices by the recogniser — 工数→コース, 核とする→拡充する, 制約→製薬, 再定義→最低技.
  工数 heard as コース and 核とする as 拡充する may also indicate slightly unclear vowel length
  or phrasing there; ask the native reviewer to check those two spots first.
