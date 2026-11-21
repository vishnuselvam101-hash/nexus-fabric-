# launch30 audio

Voice-over (local neural TTS), original synthesised score, sound design, mixes and subtitles for
the 30-second NEXUS FABRIC film. Everything is 48 kHz, exactly 30.000 s. Voice details and the
honest quality assessment are in `VOICES.md`.

## Rebuild

```sh
python3 -m venv ~/.venvs/nf-audio
~/.venvs/nf-audio/bin/pip install numpy scipy soundfile kokoro-onnx "misaki[ja]" faster-whisper
mkdir -p ~/.cache/nf-audio/kokoro && cd ~/.cache/nf-audio/kokoro
curl -LO https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx
curl -LO https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin

cd marketing/nexus-fabric/launch30/audio
~/.venvs/nf-audio/bin/python make_voice.py   # vo-en.wav vo-ja.wav vo-timing.json ../subtitles/launch30-{en,ja}.{srt,vtt}
~/.venvs/nf-audio/bin/python make_music.py   # music.wav sfx.wav sfx-cues-used.json (reads ../cues.json if present)
~/.venvs/nf-audio/bin/python make_mix.py     # mix-en.wav mix-ja.wav music-sfx-only.wav checks.json (exit 1 if a check fails)
~/.venvs/nf-audio/bin/python verify_asr.py   # optional: Whisper round-trip -> asr-roundtrip.json
```

### After the film's `../cues.json` changes

One command rebuilds the sound effects (from the current `cues.json`), both mixes, the
voiceless bed, the FLAC copies and `checks.json`; the voice stems are not touched:

```sh
cd marketing/nexus-fabric/launch30/audio && ~/.venvs/nf-audio/bin/python make_music.py && ~/.venvs/nf-audio/bin/python make_mix.py
```

It prints `ALL PASS` and exits 0 when every check holds. `sfx-cues-used.json` lists the sound
placed for each cue. Cue kinds understood: `scene_start`, `transition`, `draw_on`, `reveal`,
`camera_move`, `sweep`, `unroll`, `impact_soft`, `brand_reveal` (others are ignored); time ranges
and tick lists are read from the `note` text ("12.2-15.4 s", "at 22.0, 22.2, 22.4 s",
"five soft steps 0.18 s apart").

A venv is needed because the system setuptools cannot build `unidic-lite`. `KOKORO_DIR` overrides
the model folder. Music and sfx are deterministic (seeded); the voice is deterministic on the same
onnxruntime build.

## Files

| File | What |
|---|---|
| `vo-en.wav`, `vo-ja.wav` | mono voice stems (WAV + FLAC); lines 1–4 at segment start + 0.25 s, line 5 see below |
| `music.wav` | stereo score, synthesised in `make_music.py` (no samples); D minor tension, dominant build from 18 s, D(add9) resolution at 25 s, silent from 29.9 s; 1–4 kHz dipped for the voice |
| `sfx.wav` | stereo whooshes, ticks, soft glints, movement beds and the brand impact, placed from `../cues.json` |
| `mix-en.wav`, `mix-ja.wav` | voice + music + sfx; music ducked 9 dB under the voice (90 ms attack, 450 ms release, 60 ms look-ahead); −16 LUFS, true peak ≤ −1 dBTP |
| `music-sfx-only.wav` | un-ducked bed for voiceless cuts, also −16 LUFS |
| `vo-timing.json` | placed start/end, speed and pauses per line, and the subtitle cues |
| `checks.json` | durations, clipping, per-line fit, loudness (ffmpeg `ebur128=peak=true`) |
| `asr-roundtrip.json` | what Whisper-small heard in each voice stem |

Subtitle cues follow the measured pauses in the placed voice, max 2 lines, 42 characters per line
(EN) / 20 (JA). Re-run `make_voice.py` after any script change; it rewrites them.

Line 5 in both languages may start before 25.0 s (0.3 s after line 4) and ends by 29.70 s; the
English line 4 is tightened to make room. `make_mix.py` also writes FLAC copies of `vo-en`,
`vo-ja` and `music-sfx-only`. See `VOICES.md`.
