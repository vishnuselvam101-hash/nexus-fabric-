#!/usr/bin/env python3
"""Round-trip check: transcribe vo-en.wav / vo-ja.wav with a local Whisper model
(faster-whisper, MIT; weights openai/whisper-small, MIT) and print per-segment text."""
import json, os, sys
import soundfile as sf
from scipy.signal import resample_poly
from faster_whisper import WhisperModel
HERE = os.path.dirname(os.path.abspath(__file__))
size = sys.argv[1] if len(sys.argv) > 1 else "small"
model = WhisperModel(size, device="cpu", compute_type="int8")
narr = json.load(open(os.path.join(HERE, "..", "script", "narration.json"), encoding="utf-8"))
out = {"model": "faster-whisper " + size}
for lang in ("en", "ja"):
    x, sr = sf.read(os.path.join(HERE, f"vo-{lang}.wav"), dtype="float32")
    x = resample_poly(x, 16000, sr).astype("float32")
    segs, _ = model.transcribe(x, language=lang, beam_size=5,
                               word_timestamps=True, condition_on_previous_text=False)
    res = [{"start": round(s.start, 2), "end": round(s.end, 2), "text": s.text.strip()} for s in segs]
    out[lang] = res
    print(f"--- {lang} (script)"); [print("  ", t) for t in narr[lang]["vo"]]
    print(f"--- {lang} (heard by ASR)"); [print("  %5.2f-%5.2f %s" % (r["start"], r["end"], r["text"])) for r in res]
json.dump(out, open(os.path.join(HERE, "asr-roundtrip.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
