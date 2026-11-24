# Voices and their terms — flagship film 3

Both narrations were synthesised on this machine by `audio/make_voice.py` and `audio/nf_tts.py` (the placement and the
settings of every line are in `audio/vo-timing.json`). No text or audio was sent to an external service; both engines
ran locally, from model files already on the machine. The music and effects in `audio/make_music.py` are synthesised
in code from sines, filtered noise and envelopes; no sample library, no recording and no downloaded sound is used.

| Language | Engine | Voice | Settings (film 3, from `audio/vo-timing.json`) | Credit shown on screen |
|---|---|---|---|---|
| English | Kokoro-82M v1.0 through `kokoro-onnx` 0.6.1 | `af_heart` | `lang en-us`; speed 1.08 for lines 1 to 7 and 10, 1.16 for line 9, 1.18 for line 8; in line 8 the pause between its two sentences is shortened from 0.37 s to 0.22 s (no word changed) | none required by the terms read below; none shown |
| Japanese | VOICEVOX CORE 0.16.3 with the VOICEVOX voice model `n0.vvm` | style 10003, 女声4 of "Nemo" (speaker "Nemo" = VOICEVOX Nemo) | speedScale 1.12 for lines 1, 2 and 10; 1.16 for line 4; 1.22 for lines 3 and 5 to 9 (the maximum). Pause scale 0.5, lowered to 0.4 (line 5) or 0.3 (lines 3, 6 to 9). Accent notation per line in `script/narration.json` | 「音声：VOICEVOX Nemo」 on the Japanese end card (see `audio/CREDIT_NEEDED.txt`) |

## English: Kokoro

Terms as found in the installed files (read locally; nothing downloaded):

- `~/.venvs/nf-flagship2/lib/python3.13/site-packages/kokoro_onnx-0.6.1.dist-info/licenses/LICENSE`
  begins: "MIT License — Copyright (c) 2025 github.com/thewh1teagle" (the wrapper that runs the model).
- `~/.venvs/nf-flagship2/lib/python3.13/site-packages/kokoro_onnx-0.6.1.dist-info/METADATA` states:
  "TTS with onnx runtime based on [Kokoro-TTS](https://huggingface.co/spaces/hexgrad/Kokoro-TTS)" and
  "- kokoro model: Apache 2.0".
- The model files `~/.cache/nf-audio/kokoro/kokoro-v1.0.onnx` and `voices-v1.0.bin` have **no licence file shipped
  with them** in that folder. The Apache 2.0 statement above is the package's own description of the model. The
  model card on Hugging Face (hexgrad/Kokoro-82M) was not fetched, so its full text was not checked here.
- Credit: none is required by the terms above as quoted, and none is shown. Apache 2.0 asks that the licence notice
  travel with redistributed copies of the model; the film does not redistribute the model.

## Japanese: VOICEVOX Nemo

Terms as found in the installed files:

- `~/.cache/nf-audio/voicevox/vvm-README.md` and `~/.cache/nf-audio/voicevox/vvm-TERMS.txt` (the VOICEVOX 音声モデル
  利用規約 and the voice-library terms). The model `n0.vvm` lists "女声4 · ノーマル · 10003" (vvm-README.md, the Nemo
  table). The voice-library section (vvm-README.md lines 351–356; vvm-TERMS.txt lines 379–385) reads:
  「VOICEVOX Nemo の音声ライブラリを用いて生成した音声は、「VOICEVOX Nemo」とクレジットを記載すれば、商用・非商用で利用可能です。」
  (Audio generated with the VOICEVOX Nemo voice library can be used commercially and non-commercially if the credit
  「VOICEVOX Nemo」 is given.) The same file links https://voicevox.hiroshiba.jp/nemo/term/ for the Nemo terms; that
  page was not fetched.
- The general terms (vvm-TERMS.txt, 許諾内容) say: 「1. 商用・非商用問わず利用することができます」,
  「2. アプリケーションに組み込んで再配布することができます」, and 「3. 作成された音声を利用する際は、各音声ライブラリの規約に従ってください」.
  They also say: 「ご利用の際は VOICEVOX を利用したことがわかるクレジット表記が必要です。」
- The engine: `~/.venvs/nf-flagship2/lib/python3.13/site-packages/voicevox_core-0.16.3.dist-info/licenses/LICENSE`
  is the MIT licence, "Copyright (c) 2021 Hiroshiba Kazuyuki" (METADATA: License: MIT).
- Credit shown: 「音声：VOICEVOX Nemo」 at the bottom right of the Japanese end card, for the whole time the card is up
  (`audio/CREDIT_NEEDED.txt`).

## Checks and limits

- Speech round trip: every placed line was transcribed back with faster-whisper (large-v3-turbo, weights already on
  this machine) and compared with its script text; the results are in `audio/vo-timing.json` (`asr`, `asr_error`).
  The round trip is a measurement, not a listening test; the recogniser chooses homophones without context.
- No licence text was found for the Kokoro model weights other than the package's metadata line.
- The Nemo terms page on the VOICEVOX site was not opened (no downloads were allowed).
- Nothing here is legal advice; the lines above are quotations, not an interpretation.
