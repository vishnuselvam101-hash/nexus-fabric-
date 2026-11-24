#!/usr/bin/env python3
"""flagship2 narration: record both languages locally, lock the picture timing to the voice, write subtitles.

  python make_voice.py            record, place, write everything
  python make_voice.py --check    also run the speech-recognition round trip on the placed tracks (slow)

English  Kokoro-82M v1.0, voice af_heart (Apache-2.0), kokoro-onnx on CPU.
Japanese VOICEVOX CORE 0.16.3 (MIT) with the VOICEVOX Nemo voice 女声4 (style 10003): Open JTalk front end,
         accent phrases given explicitly in script/narration.json ("kana"). Credit required: "VOICEVOX Nemo".
Nothing is sent to any speech service.

Timing rule (the picture follows the voice): every line has one slot shared by both languages. A slot starts
where the previous slot ended plus that line's breathing gap; a slot is as long as the longer of the two
recordings. The film page reads the result from audio/timing.js, so both languages have identical picture timing.

Outputs: vo-en.wav vo-ja.wav (48 kHz mono, full film length) + .flac, vo-timing.json, timing.js,
         ../subtitles/flagship2-{en,ja}.{srt,vtt}
"""
import json, os, re, sys
import numpy as np, soundfile as sf
import nf_tts as T

HERE = os.path.dirname(os.path.abspath(__file__))
SR = T.SR
VOICE = {"en": {"engine": "kokoro", "voice": "af_heart", "speed": 0.88, "lang": "en-us"},
         "ja": {"engine": "voicevox", "voice": 10003, "speed": 1.05, "intonation": 1.0, "pitch": 0.0, "pause_scale": 0.62}}
LEAD = 0.60                                                   # first line starts here
GAPS = [0.90, 2.00, 1.30, 0.80, 0.55, 0.50, 0.65, 0.50]       # silence after lines 1..8 (the picture needs these breaths)
FILM = 50.0                                                   # film length; the tail after the last word takes what is left
MIN_TAIL = 2.0
LINE_PEAK_DB = -3.0; LINE_RMS_DB = -23.0                      # per-line level before the mix


def say(lang, ln):
    v = VOICE[lang]
    if v["engine"] == "kokoro":
        x = T.kokoro_say(ln.get("say", ln["text"]), v["voice"], v["speed"], v["lang"])
    else:
        x, _ = T.voicevox_say(kana=ln["kana"], style_id=v["voice"], speed=v["speed"], intonation=v["intonation"], pitch=v["pitch"], pause_scale=v["pause_scale"])
    x = T.trim(x)
    x = x * (10 ** (LINE_RMS_DB / 20) / (np.sqrt(np.mean(x ** 2)) + 1e-12))          # same loudness per line
    pk = np.abs(x).max(); lim = 10 ** (LINE_PEAK_DB / 20)
    return x * (lim / pk) if pk > lim else x


def ts(t, sep):
    ms = int(round(t * 1000)); return "%02d:%02d:%02d%s%03d" % (ms // 3600000, ms // 60000 % 60, ms // 1000 % 60, sep, ms % 1000)


def cues_for(lang, text, t0, x):
    """Subtitle cues from the recording itself: sentences and clauses are given the stretches between the pauses
    measured in the audio. Max two lines; 42 characters a line (en), 18 (ja)."""
    gaps = [(t0 + a, t0 + b) for a, b in T.pauses(x, SR, min_len=0.12)]
    t1 = t0 + len(x) / SR
    if lang == "ja":
        parts = [p for p in re.findall(r"[^。、]+[。、]?", text) if p.strip()]
    else:
        parts = [p.strip() for p in re.findall(r"[^.,:;—]+[.,:;—]?", text) if p.strip()]
    w = np.array([len(re.sub(r"[\s。、.,:;—「」]", "", p)) + 1 for p in parts], float)
    est = t0 + (t1 - t0) * np.cumsum(w)[:-1] / w.sum()
    bounds = []; used = set()
    for e in est:                                              # snap each boundary to the nearest measured pause
        best = None
        for gi, (a, b) in enumerate(gaps):
            if gi in used: continue
            d = abs((a + b) / 2 - e)
            if best is None or d < best[0]: best = (d, gi)
        if best and best[0] < 0.75: used.add(best[1]); bounds.append(gaps[best[1]])
        else: bounds.append(None)
    # merge parts that have no pause between them, then group into cues
    groups = [[parts[0]]]; spans = [[t0, None]]
    for p, b in zip(parts[1:], bounds):
        if b is None: groups[-1].append(p)
        else: spans[-1][1] = b[0]; groups.append([p]); spans.append([b[1], None])
    spans[-1][1] = t1
    limit = 42 if lang == "en" else 18; join = (lambda g: "".join(g)) if lang == "ja" else (lambda g: " ".join(g))
    cues = []
    for g, (a, b) in zip(groups, spans):
        s = join(g)
        if cues and len(cues[-1][0]) == 1 and len(cues[-1][0][0]) <= limit and len(s) <= limit and a - cues[-1][2] < 0.5 and b - cues[-1][1] < 4.2:
            cues[-1] = (cues[-1][0] + [s], cues[-1][1], b)     # short neighbours share a two-line cue
        else:
            cues.append(([s], a, b))
    out = []
    for lines, a, b in cues:                                   # wrap a long single line in two
        if len(lines) == 1 and len(lines[0]) > limit:
            s = lines[0]; cand = [i for i, ch in enumerate(s) if (ch == " " if lang == "en" else ch in "、。")] or [len(s) // 2]
            i = min(cand, key=lambda i: abs(i - len(s) / 2)); lines = [s[:i + (lang == "ja")].strip(), s[i + (lang == "ja"):].strip()]
        out.append(([l for l in lines if l], a, b))
    return out


def write_subs(lang, cues, dur):
    d = os.path.join(HERE, "..", "subtitles"); os.makedirs(d, exist_ok=True); out = []
    for i, (lines, a, b) in enumerate(cues):
        nxt = cues[i + 1][1] if i + 1 < len(cues) else dur
        out.append((lines, max(0.0, a - 0.06), min(b + 0.40, nxt - 0.08, dur - 0.05)))
    with open(os.path.join(d, f"flagship2-{lang}.srt"), "w", encoding="utf-8") as f:
        for i, (lines, a, b) in enumerate(out, 1): f.write(f"{i}\n{ts(a, ',')} --> {ts(b, ',')}\n" + "\n".join(lines) + "\n\n")
    with open(os.path.join(d, f"flagship2-{lang}.vtt"), "w", encoding="utf-8") as f:
        f.write("WEBVTT\n\n")
        for i, (lines, a, b) in enumerate(out, 1): f.write(f"{i}\n{ts(a, '.')} --> {ts(b, '.')}\n" + "\n".join(lines) + "\n\n")
    return [{"start": round(a, 3), "end": round(b, 3), "lines": l} for l, a, b in out]


def main():
    N = json.load(open(os.path.join(HERE, "..", "script", "narration.json"), encoding="utf-8"))
    rec = {lang: [say(lang, ln) for ln in N[lang]["lines"]] for lang in ("en", "ja")}
    n = len(N["en"]["lines"]); assert n == len(N["ja"]["lines"]) == len(GAPS) + 1
    slots = []; t = LEAD
    for i in range(n):
        d = max(len(rec["en"][i]), len(rec["ja"][i])) / SR
        slots.append({"id": N["en"]["lines"][i]["id"], "start": round(t, 3), "end": round(t + d, 3), "en": round(len(rec["en"][i]) / SR, 3), "ja": round(len(rec["ja"][i]) / SR, 3)})
        t += d + (GAPS[i] if i < n - 1 else 0)
    dur = FILM; TAIL = round(dur - t, 3)
    print("lines en:", " ".join(f"{len(x) / SR:.2f}" for x in rec["en"]), "| ja:", " ".join(f"{len(x) / SR:.2f}" for x in rec["ja"]), "| last word ends", round(t, 2))
    if TAIL < MIN_TAIL: raise SystemExit(f"narration does not fit: last word ends at {t:.2f} s, tail would be {TAIL:.2f} s (< {MIN_TAIL})")
    timing = {"sample_rate": SR, "fps": 30, "duration": round(float(dur), 4), "frames": int(round(dur * 30)), "lead": LEAD, "gaps": GAPS, "tail": TAIL, "slots": slots, "voice": VOICE}
    for lang in ("en", "ja"):
        track = np.zeros(int(round(dur * SR))); info = []; cues = []
        for i, (ln, x, s) in enumerate(zip(N[lang]["lines"], rec[lang], slots)):
            a = int(round(s["start"] * SR)); track[a:a + len(x)] += x
            m = T.measure(x)
            units = len(T.norm_en(ln["text"])) if lang == "en" else len(T.ja_kana(ln["reading"]))
            info.append({"id": ln["id"], "text": ln["text"], "start": s["start"], "end": round(s["start"] + len(x) / SR, 3), "duration": m["duration_s"], "speech": m["speech_s"],
                         "units": units, "rate": round(units / m["speech_s"], 2), "pauses": [[round(s["start"] + p, 3), round(s["start"] + q, 3)] for p, q in m["pauses"]],
                         "peak_dbfs": m["peak_dbfs"], "f0_median_hz": m.get("f0_median_hz"), "clipped": m["clipped_samples"]})
            cues += cues_for(lang, ln["text"], s["start"], x)
        sf.write(os.path.join(HERE, f"vo-{lang}.wav"), track, SR, subtype="PCM_24"); sf.write(os.path.join(HERE, f"vo-{lang}.flac"), track, SR, subtype="PCM_24")
        timing[lang] = {"lines": info, "cues": write_subs(lang, cues, float(dur)), "speech_total": round(sum(i["duration"] for i in info), 2),
                        "rate_unit": "words/s" if lang == "en" else "kana/s", "mean_rate": round(float(np.mean([i["rate"] for i in info])), 2)}
        print(f"{lang}: " + " ".join(f"{i['duration']:.2f}" for i in info) + f" | total {timing[lang]['speech_total']} s, mean rate {timing[lang]['mean_rate']} {timing[lang]['rate_unit']}")
    if "--check" in sys.argv:
        for lang in ("en", "ja"):
            x, _ = sf.read(os.path.join(HERE, f"vo-{lang}.wav")); _, lp, words = T.asr(x, lang)
            for ln, s in zip(timing[lang]["lines"], slots):
                hyp = ("" if lang == "ja" else " ").join(w for w, a, b in words if s["start"] - 0.3 <= (a + b) / 2 < s["end"] + 0.3)
                ref = N[lang]["lines"][timing[lang]["lines"].index(ln)]
                ln["asr"] = hyp; ln["asr_error"] = T.wer(ref["text"], hyp) if lang == "en" else T.cer_ja(ref["reading"], hyp)
            timing[lang]["asr_logprob"] = lp
            print(lang, "round trip:", [l["asr_error"] for l in timing[lang]["lines"]], "confidence", lp)
    else:                                                      # keep an earlier round-trip result if the audio did not change
        try:
            old = json.load(open(os.path.join(HERE, "vo-timing.json"), encoding="utf-8"))
            for lang in ("en", "ja"):
                for a, b in zip(timing[lang]["lines"], old[lang]["lines"]):
                    if a["text"] == b["text"] and abs(a["duration"] - b["duration"]) < 1e-3 and "asr" in b: a["asr"] = b["asr"]; a["asr_error"] = b["asr_error"]
        except Exception: pass
    json.dump(timing, open(os.path.join(HERE, "vo-timing.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    open(os.path.join(HERE, "timing.js"), "w", encoding="utf-8").write("window.NF_TIMING = " + json.dumps({k: timing[k] for k in ("fps", "duration", "frames", "slots")}) + ";\n")
    print("slots:", " ".join(f"{s['id']} {s['start']:.2f}-{s['end']:.2f}" for s in slots), "| film", timing["duration"], "s,", timing["frames"], "frames")


if __name__ == "__main__":
    main()
