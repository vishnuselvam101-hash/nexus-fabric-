#!/usr/bin/env python3
"""NEXUS FABRIC flagship film 3 - the narration on the bar grid (54.0 s, 120 BPM, bar = 2.0 s).

  python make_voice.py            synthesise, place, write audio, timing and subtitles
  python make_voice.py --check    also transcribe each placed line back with faster-whisper (slow)

Reads    script/narration.json  (locked text, slots, Japanese readings, and the proposed cut where 'use' is 'cut')
Writes   audio/vo-en.wav, audio/vo-ja.wav   54.000 s, 48 kHz, mono, 24-bit (the .flac copies are kept for git)
         audio/vo-timing.json, audio/timing.js, subtitles/film3-{en,ja}.{srt,vtt}

English  Kokoro-82M v1.0, voice af_heart (Apache-2.0), kokoro-onnx on CPU.
         Rate: Kokoro speed 1.08 for each line, raised for that line only as far as it needs (never above 1.18).
Japanese VOICEVOX CORE 0.16.3 (MIT), voice model Nemo, 女声4 (style 10003), Open JTalk accent given in kana.
         Rate: speedScale 1.12 for each line, raised as far as it needs (never above 1.22); pause scale 0.5,
         lowered to 0.3 only where the line still needs the room. Credit: 「音声：VOICEVOX Nemo」.
Nothing is sent to any speech service.

Placement rule (locked): every line sits inside its slot with at least 0.25 s of silence before and after.
The audible part of a line starts LEAD = 0.30 s after its slot starts, so the voice follows the bar line;
a line too long for that is moved earlier as far as the rule allows. Lines are measured without the trim
padding, so the 0.25 s is counted from the first and last audible sample.
"""
import json, os, re, sys
import numpy as np
import soundfile as sf
import nf_tts as T

HERE = os.path.dirname(os.path.abspath(__file__))
SUBS = os.path.join(HERE, "..", "subtitles")
SR = T.SR
FILM = 54.0
N = int(round(FILM * SR))
BPM = 120
BEAT = 60.0 / BPM                       # 0.5 s
BAR = 4 * BEAT                          # 2.0 s
RULE = 0.25                             # silence required before and after each line
LEAD = 0.30                             # audible start, this long after the slot starts
PAD = 0.03                              # silence kept around each clip (the trim pad)
SLACK = 0.05                            # spare time a line should keep inside its slot
EN_RATES = [1.08, 1.10, 1.12, 1.14, 1.16, 1.18]                         # Kokoro speed, never above 1.18
JA_RATES = [(1.12, 0.5), (1.16, 0.5), (1.22, 0.5), (1.22, 0.4), (1.22, 0.3)]   # (speedScale, pauseScale), never above 1.22
EN_BREATH, EN_BREATH_MIN = 0.22, 0.30  # last resort for English: sentence pauses of 0.30 s or more are shortened to 0.22 s
RMS_DB, PEAK_DB = -23.0, -3.0           # the same level for every line, as in film 2
JA_STYLE = 10003
VOICE = {"en": {"engine": "kokoro", "voice": "af_heart", "lang": "en-us", "rates": EN_RATES, "film2_speed": 0.88},
         "ja": {"engine": "voicevox", "voice": "女声4 (Nemo), style 10003", "style_id": JA_STYLE, "intonation": 1.0, "pitch": 0.0,
                "rates": JA_RATES, "film2_speed": 1.05, "film2_pause_scale": 0.62}}


def spoken(lang, ln):
    """the text and reading that are synthesised: the locked text, or the proposed cut where 'use' says so"""
    if lang == "ja" and ln.get("use") == "cut":
        return ln["cut"]["text"], ln["cut"]["kana"], "cut"
    if lang == "ja":
        return ln["text"], ln["kana"], "exact"
    return ln["text"], None, "exact"


def synth(lang, ln, text, kana, rate):
    if lang == "en":
        return T.kokoro_say(ln.get("say", text), VOICE["en"]["voice"], rate, VOICE["en"]["lang"])
    sp, ps = rate
    x, _ = T.voicevox_say(kana=kana, style_id=JA_STYLE, speed=sp, intonation=1.0, pitch=0.0, pause_scale=ps)
    return x


def audible_len(x):
    return len(T.trim(x, SR, pad=0.0)) / SR


def shorten_pauses(x, target, min_len):
    """cut the middle out of each internal silence of at least min_len that is longer than target"""
    y = x.copy()
    for p, q in sorted(T.pauses(x, SR, min_len=min_len), reverse=True):
        excess = (q - p) - target
        if excess <= 0:
            continue
        i0 = int(round((p + (q - p) / 2 - excess / 2) * SR)); n = int(round(excess * SR))
        y = np.concatenate([y[:i0], y[i0 + n:]])
    return y


def choose(lang, ln, text, kana, slot):
    """the first (slowest) rate that leaves SLACK inside the slot. If English still overruns at the fastest rate,
    its sentence pauses are shortened (the words and the rate do not change). Otherwise the line is flagged OVERRUN.
    -> (rate, clip, audible length, status, cap, tried rates, edit note)"""
    cap = (slot[1] - slot[0]) - 2 * RULE
    tried, last = [], None
    for r in VOICE[lang]["rates"]:
        x = synth(lang, ln, text, kana, r)
        a = audible_len(x)
        tried.append([list(r) if isinstance(r, tuple) else r, round(a, 3)])
        last = (r, x, a)
        if a <= cap - SLACK:
            return r, x, a, "fits", cap, tried, ""
    r, x, a = last
    note = ""
    if lang == "en" and a > cap - SLACK:
        x = shorten_pauses(x, EN_BREATH, EN_BREATH_MIN)
        a = audible_len(x)
        note = "sentence pauses of %.2f s or more shortened to %.2f s" % (EN_BREATH_MIN, EN_BREATH)
        tried.append(["breath %.2f" % EN_BREATH, round(a, 3)])
    status = "fits" if a <= cap - SLACK else ("fits, tight" if a <= cap else "OVERRUN")
    return r, x, a, status, cap, tried, note


def place(slot, a):
    s, e = slot
    start = s + LEAD
    if start + a > e - RULE:
        start = e - RULE - a
    return round(start, 4)


def mora_count(kana):
    syn, _ = T.voicevox()
    aq = syn.create_audio_query_from_kana(kana, JA_STYLE)
    return T.aq_summary(aq)[1]


def cer_ja(ref, hyp):
    r, h = T.ja_kana(ref), T.ja_kana(hyp)
    return round(T.lev(r, h) / max(len(r), 1), 4)


def ts(t, sep):
    ms = int(round(t * 1000))
    return "%02d:%02d:%02d%s%03d" % (ms // 3600000, ms // 60000 % 60, ms // 1000 % 60, sep, ms % 1000)


def cues_for(lang, text, t0, x):
    """Subtitle cues from the recording: the sentences and clauses of the text are spread over the stretches
    between the pauses measured in the audio. Max two lines; 42 characters a line (en), 18 (ja)."""
    gaps = [(t0 + a, t0 + b) for a, b in T.pauses(x, SR, min_len=0.12)]
    t1 = t0 + len(x) / SR
    if lang == "ja":
        parts = [p for p in re.findall(r"[^。、]+[。、]?", text) if p.strip()]
    else:
        parts = [p.strip() for p in re.findall(r"[^.,:;—]+[.,:;—]?", text) if p.strip()]
    w = np.array([len(re.sub(r"[\s。、.,:;—「」]", "", p)) + 1 for p in parts], float)
    est = t0 + (t1 - t0) * np.cumsum(w)[:-1] / w.sum()
    bounds, used = [], set()
    for e in est:
        best = None
        for gi, (a, b) in enumerate(gaps):
            if gi in used:
                continue
            d = abs((a + b) / 2 - e)
            if best is None or d < best[0]:
                best = (d, gi)
        if best and best[0] < 0.75:
            used.add(best[1]); bounds.append(gaps[best[1]])
        else:
            bounds.append(None)
    groups = [[parts[0]]]; spans = [[t0, None]]
    for p, b in zip(parts[1:], bounds):
        if b is None:
            groups[-1].append(p)
        else:
            spans[-1][1] = b[0]; groups.append([p]); spans.append([b[1], None])
    spans[-1][1] = t1
    limit = 42 if lang == "en" else 18
    join = (lambda g: "".join(g)) if lang == "ja" else (lambda g: " ".join(g))
    cues = []
    for g, (a, b) in zip(groups, spans):
        s = join(g)
        if cues and len(cues[-1][0]) == 1 and len(cues[-1][0][0]) <= limit and len(s) <= limit and a - cues[-1][2] < 0.5 and b - cues[-1][1] < 4.2:
            cues[-1] = (cues[-1][0] + [s], cues[-1][1], b)
        else:
            cues.append(([s], a, b))
    out = []
    for lines, a, b in cues:
        if len(lines) == 1 and len(lines[0]) > limit:
            s = lines[0]
            cand = [i for i, ch in enumerate(s) if (ch == " " if lang == "en" else ch in "、。")] or [len(s) // 2]
            i = min(cand, key=lambda i: abs(i - len(s) / 2))
            lines = [s[:i + (lang == "ja")].strip(), s[i + (lang == "ja"):].strip()]
        out.append(([l for l in lines if l], a, b))
    return out


def write_subs(lang, cues, dur):
    os.makedirs(SUBS, exist_ok=True); out = []
    for i, (lines, a, b) in enumerate(cues):
        nxt = cues[i + 1][1] if i + 1 < len(cues) else dur
        out.append((lines, max(0.0, a - 0.06), min(b + 0.40, nxt - 0.08, dur - 0.05)))
    with open(os.path.join(SUBS, f"film3-{lang}.srt"), "w", encoding="utf-8") as f:
        for i, (lines, a, b) in enumerate(out, 1):
            f.write(f"{i}\n{ts(a, ',')} --> {ts(b, ',')}\n" + "\n".join(lines) + "\n\n")
    with open(os.path.join(SUBS, f"film3-{lang}.vtt"), "w", encoding="utf-8") as f:
        f.write("WEBVTT\n\n")
        for i, (lines, a, b) in enumerate(out, 1):
            f.write(f"{i}\n{ts(a, '.')} --> {ts(b, '.')}\n" + "\n".join(lines) + "\n\n")
    return [{"start": round(a, 3), "end": round(b, 3), "lines": l} for l, a, b in out]


def main():
    check = "--check" in sys.argv
    ja_only = "--ja-only" in sys.argv
    NR = json.load(open(os.path.join(HERE, "..", "script", "narration.json"), encoding="utf-8"))
    en_slots = [tuple(ln["slot"]) for ln in NR["en"]["lines"]]
    ja_slots = [tuple(ln["slot"]) for ln in NR["ja"]["lines"]]
    assert en_slots == ja_slots and len(en_slots) == 10, "both languages must share the ten slots"
    assert all(abs(t / BAR - round(t / BAR)) < 1e-9 for s in en_slots for t in s), "slot boundaries must sit on bar lines"
    assert en_slots[-1][1] == FILM and en_slots[0][0] == 0.0
    old = {}
    try:
        old = json.load(open(os.path.join(HERE, "vo-timing.json"), encoding="utf-8"))
    except Exception:
        pass
    timing = {"sample_rate": SR, "fps": 30, "duration": FILM, "frames": int(round(FILM * 30)), "bpm": BPM, "beat": BEAT, "bar": BAR,
              "rule_s": RULE, "lead_s": LEAD, "trim_pad_s": PAD, "slack_s": SLACK, "voice": VOICE}
    langs = ("ja",) if ja_only else ("en", "ja")
    if ja_only:                    # rebuild the Japanese narration only; the English audio, subtitles and timing are kept as they are
        if "en" not in old:
            raise SystemExit("--ja-only needs the existing vo-timing.json (for the English timing)")
        timing["en"] = old["en"]
    clips = {}
    for lang in langs:
        track = np.zeros(N); info = []; cues = []; clips[lang] = []
        for k, ln in enumerate(NR[lang]["lines"]):
            slot = en_slots[k]
            text, kana, use = spoken(lang, ln)
            r, x, a, status, cap, tried, note = choose(lang, ln, text, kana, slot)
            if status == "OVERRUN":
                raise SystemExit(f"{lang} {ln['id']}: {a:.3f} s of speech does not fit {cap:.3f} s (rate {r}); see the proposed cut")
            clip = T.trim(x, SR, pad=PAD)
            vo = clip[int(PAD * SR): len(clip) - int(PAD * SR)]
            clip = clip * (10 ** (RMS_DB / 20) / (np.sqrt(np.mean(vo ** 2)) + 1e-12))
            pk = np.abs(clip).max(); lim = 10 ** (PEAK_DB / 20)
            if pk > lim:
                clip = clip * (lim / pk)
            start = place(slot, a)
            i0 = int(round((start - PAD) * SR))
            track[i0:i0 + len(clip)] += clip
            clips[lang].append(clip)
            rec = {"id": ln["id"], "slot": list(slot), "text": ln["text"], "spoken": text, "use": use, "fit": status,
                   "cap_s": round(cap, 3), "tried": tried, "edit": note, "start": start, "end": round(start + a, 3), "duration": round(a, 3),
                   "slack_before": round(start - slot[0], 3), "slack_after": round(slot[1] - (start + a), 3),
                   "peak_dbfs": round(20 * np.log10(np.abs(clip).max() + 1e-12), 2), "rms_dbfs": round(10 * np.log10(np.mean(vo ** 2) + 1e-12), 2),
                   "pauses": [[round(start - PAD + p, 3), round(start - PAD + q, 3)] for p, q in T.pauses(clip, SR)]}
            if lang == "en":
                rec.update({"speed": r, "units": len(T.norm_en(text)), "unit": "words"})
            else:
                sp, ps = r
                n = mora_count(kana)
                rec.update({"speed": sp, "pause_scale": ps, "kana": kana, "units": n, "unit": "mora"})
            rec["rate"] = round(rec["units"] / a, 2)
            rec["rate_unit"] = "words/s" if lang == "en" else "mora/s"
            if lang == "ja" and use == "cut":
                rec["locked_text"] = ln["text"]
            info.append(rec)
            cues += cues_for(lang, text, start - PAD, clip)
            print(f"{lang} {ln['id']:<3} slot {slot[0]:>5.1f}-{slot[1]:<5.1f} speech {a:5.2f} s  cap {cap:5.2f}  rate {r}  {status}", flush=True)
        if not check:                                                  # keep an earlier round trip if the audio is unchanged
            for rec, o in zip(info, old.get(lang, {}).get("lines", [])):
                if rec["spoken"] == o.get("spoken") and abs(rec["duration"] - o.get("duration", -1)) < 1e-3 and "asr" in o:
                    rec["asr"] = o["asr"]; rec["asr_error"] = o["asr_error"]; rec["asr_logprob"] = o.get("asr_logprob")
        sf.write(os.path.join(HERE, f"vo-{lang}.wav"), track, SR, subtype="PCM_24")
        sf.write(os.path.join(HERE, f"vo-{lang}.flac"), track, SR, subtype="PCM_24")
        units = sum(r["units"] for r in info); tot = sum(r["duration"] for r in info)
        timing[lang] = {"lines": info, "cues": write_subs(lang, cues, FILM), "speech_total": round(tot, 3),
                        "rate_unit": "words/s" if lang == "en" else "mora/s", "mean_rate": round(units / tot, 2),
                        "film_length_s": FILM, "samples": int(len(track))}
        print(f"{lang}: written, {len(track)} samples = {len(track) / SR:.3f} s, speech {tot:.2f} s", flush=True)
    if check:
        for lang in langs:
            for rec, clip in zip(timing[lang]["lines"], clips[lang]):
                hyp, lp, _ = T.asr(clip, lang)
                rec["asr"] = hyp; rec["asr_logprob"] = lp
                rec["asr_error"] = T.wer(rec["spoken"], hyp) if lang == "en" else cer_ja(rec["spoken"], hyp)
                print(f"{lang} {rec['id']:<3} error {rec['asr_error']:.3f}  | {rec['spoken']}  ->  {hyp}", flush=True)
    timing["slots"] = []
    for k, sl in enumerate(en_slots):
        e, j = timing["en"]["lines"][k], timing["ja"]["lines"][k]
        timing["slots"].append({"id": NR["en"]["lines"][k]["id"], "start": sl[0], "end": sl[1], "en": e["duration"], "ja": j["duration"],
                                "en_start": e["start"], "en_end": e["end"], "ja_start": j["start"], "ja_end": j["end"]})
    json.dump(timing, open(os.path.join(HERE, "vo-timing.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    js = {"fps": 30, "duration": FILM, "frames": timing["frames"], "bpm": BPM, "beat": BEAT, "bar": BAR, "bars": int(round(FILM / BAR)),
          "beats": int(round(FILM / BEAT)), "slots": timing["slots"]}
    open(os.path.join(HERE, "timing.js"), "w", encoding="utf-8").write("window.NF_TIMING = " + json.dumps(js, ensure_ascii=False) + ";\n")
    for s in timing["slots"]:
        print(f"{s['id']:<4} slot {s['start']:>5.1f}-{s['end']:<5.1f}  en {s['en_start']:6.3f}-{s['en_end']:6.3f}  ja {s['ja_start']:6.3f}-{s['ja_end']:6.3f}")


if __name__ == "__main__":
    main()
