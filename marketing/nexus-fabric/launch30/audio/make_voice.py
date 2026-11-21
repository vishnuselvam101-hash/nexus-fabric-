#!/usr/bin/env python3
"""NEXUS FABRIC launch30 - voice-over + subtitles.

Local neural TTS only (Kokoro-82M v1.0 via kokoro-onnx, Apache-2.0). Nothing is
sent to any speech API. Each narration line is synthesised on its own, placed at
segment start + 0.25 s, and the speaking rate is chosen natively by the model's
`speed` input (no post-hoc time stretching).

Outputs: vo-en.wav, vo-ja.wav (48 kHz mono, 30.000 s), vo-timing.json,
         ../subtitles/launch30-{en,ja}.{srt,vtt}
"""
import json, os, re, sys
import numpy as np
import soundfile as sf
from scipy.signal import resample_poly

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 48000
DUR = 30.0
LEAD = 0.25          # lead-in after segment start
TAIL = 0.20          # voice must end this long before the next segment
MODEL_DIR = os.environ.get("KOKORO_DIR", os.path.expanduser("~/.cache/nf-audio/kokoro"))
VOICES = {"en": ("af_heart", "en-us"), "ja": ("jf_alpha", "ja")}
SPEEDS = [1.0, 1.04, 1.08, 1.12, 1.16, 1.20]   # model-native rate, 1.20 = upper natural limit
# Kokoro's `speed` input is strongly non-linear (nominal 1.08 and 1.10 give identical audio,
# nominal 1.18 is ~1.09x real), so the lead's limits are enforced on the MEASURED rate:
# speech-only duration at speed 1.0 divided by speech-only duration of the take.
NOMINAL = [1.0, 1.04, 1.08, 1.12, 1.16, 1.18, 1.20, 1.22]
LAST_MAX_RATE = 1.08                            # final line: measured rate never above 1.08
PREV_MAX_RATE = 1.08                            # line before it may be tightened to make room
LAST_GAP = 0.30                                 # final line starts >= this long after the previous one
LAST_END = 29.70                                # final line must end by here
PAUSE_CAPS = [None, 0.34, 0.26, 0.22]                # optional tightening of inter-sentence pauses (s)

# Text normalisation for the synthesiser only (subtitles keep the script text).
EN_SAY = [("NEXUS FABRIC", "Nexus Fabric"), ("AMOEBA", "Amoeba")]
JA_SAY = [("NEXUS FABRIC", "ネクサス・ファブリック"), ("AMOEBA", "アメーバ"), ("AI", "エーアイ"),
          ("3D", "スリーディー"), ("CAD", "キャド")]


def load_engine():
    from kokoro_onnx import Kokoro
    return Kokoro(os.path.join(MODEL_DIR, "kokoro-v1.0.onnx"),
                  os.path.join(MODEL_DIR, "voices-v1.0.bin"))


_jag2p = None
def ja_phonemes(text):
    global _jag2p
    if _jag2p is None:
        from misaki import ja
        _jag2p = ja.JAG2P()
    out = _jag2p(text)
    return out[0] if isinstance(out, tuple) else out


def check_ja_readings(narr):
    """Kanji text must phonemise identically to the supplied kana reading."""
    bad = []
    for i, (t, r) in enumerate(zip(narr["ja"]["vo"], narr["ja"]["vo_reading"])):
        for k, v in JA_SAY:
            t = t.replace(k, v)
        # spacing and long-o spelling (oɯ vs oː) depend on tokenisation only
        a = re.sub(r"\s+", "", ja_phonemes(t)).replace("oɯ", "oː")
        b = re.sub(r"\s+", "", ja_phonemes(r)).replace("oɯ", "oː")
        if a != b:
            bad.append((i, a, b))
    return bad


def trim(x, sr, thr_db=-45.0, pad=0.02):
    env = np.abs(x)
    thr = 10 ** (thr_db / 20) * max(env.max(), 1e-9)
    idx = np.where(env > thr)[0]
    if len(idx) == 0:
        return x
    a = max(0, idx[0] - int(pad * sr)); b = min(len(x), idx[-1] + int(pad * sr))
    y = x[a:b].copy()
    f = int(0.008 * sr)
    y[:f] *= np.linspace(0, 1, f); y[-f:] *= np.linspace(1, 0, f)
    return y


def synth(engine, lang, text, speed):
    voice, code = VOICES[lang]
    if lang == "ja":
        for k, v in JA_SAY:
            text = text.replace(k, v)
        wav, sr = engine.create(ja_phonemes(text), voice=voice, speed=speed, is_phonemes=True)
    else:
        for k, v in EN_SAY:
            text = text.replace(k, v)
        wav, sr = engine.create(text, voice=voice, speed=speed, lang=code)
    wav = np.asarray(wav, dtype=np.float64).flatten()
    wav = resample_poly(wav, SR, sr)            # 24 kHz -> 48 kHz
    return trim(wav, SR)


def silence_gaps(x, sr, min_len=0.09, thr_db=-38.0):
    """Return [(start_s, end_s)] of internal pauses."""
    hop = int(0.005 * sr); win = int(0.02 * sr)
    n = max(1, (len(x) - win) // hop)
    rms = np.array([np.sqrt(np.mean(x[i * hop:i * hop + win] ** 2)) for i in range(n)])
    quiet = rms < 10 ** (thr_db / 20) * rms.max()
    gaps = []; i = 0
    while i < n:
        if quiet[i]:
            j = i
            while j < n and quiet[j]:
                j += 1
            a, b = i * hop / sr, (j * hop + win) / sr
            if b - a >= min_len and i > 0 and j < n:
                gaps.append((a, b))
            i = j
        else:
            i += 1
    return gaps


def tighten_pauses(x, sr, cap):
    """Shorten internal pauses longer than `cap` seconds (edit of silence only, no stretch)."""
    out = []; pos = 0; f = int(0.006 * sr)
    for a, b in silence_gaps(x, sr):
        if b - a > cap:
            cut = (b - a) - cap
            mid = (a + b) / 2
            i0 = int((mid - cut / 2) * sr); i1 = int((mid + cut / 2) * sr)
            seg = x[pos:i0].copy(); seg[-f:] *= np.linspace(1, 0, f)
            out.append(seg)
            x = x.copy(); x[i1:i1 + f] *= np.linspace(0, 1, f)
            pos = i1
    out.append(x[pos:])
    return np.concatenate(out)


# ---------- subtitle chunking ----------
def weight(s):
    return len(re.sub(r"[\s。、.,]", "", s)) + 1


def split_sentences(text, lang):
    if lang == "ja":
        return [s for s in re.findall(r"[^。]+。?", text) if s.strip()]
    return [s.strip() for s in re.findall(r"[^.]+\.?", text) if s.strip()]


def split_clauses(sentence, lang):
    if lang == "ja":
        return [s for s in re.findall(r"[^、]+、?", sentence) if s]
    parts = re.split(r"(?<=,)\s+|\s+(?=and\s)", sentence)
    return [p for p in parts if p]


def wrap2(text, lang, limit):
    """Wrap into <=2 lines of <=limit chars; returns list of lines or None."""
    if len(text) <= limit:
        return [text]
    best = None
    if lang == "ja":
        cands = [(i + 1, 0 if text[i] in "。、" else 1) for i in range(len(text) - 1)]
        cands = [(i, c) for i, c in cands if c == 0 or text[i] not in "。、ー"]
        join = lambda a, b: (a, b)
    else:
        cands = [(i, 0 if text[i - 1] in ".,;" else 1) for i, ch in enumerate(text) if ch == " "]
    for i, pen in cands:
        a, b = text[:i].strip(), text[i:].strip()
        if len(a) <= limit and len(b) <= limit:
            cost = pen * 100 + abs(len(a) - len(b))
            if best is None or cost < best[0]:
                best = (cost, [a, b])
    return best[1] if best else None


def assign_times(units, t0, t1, gaps):
    """Split [t0,t1] into len(units) spans; boundaries snap to pauses."""
    n = len(units)
    if n == 1:
        return [(t0, t1)]
    w = np.array([weight(u) for u in units], float)
    est = t0 + (t1 - t0) * np.cumsum(w)[:-1] / w.sum()
    inner = [(a, b) for a, b in gaps if a > t0 + 0.15 and b < t1 - 0.15]
    bounds = []
    used = set()
    for e in est:
        best = None
        for gi, (a, b) in enumerate(inner):
            if gi in used:
                continue
            d = abs((a + b) / 2 - e) - 0.5 * (b - a)   # longer pauses preferred
            if best is None or d < best[0]:
                best = (d, gi)
        if best is not None and best[0] < 0.6:
            used.add(best[1]); bounds.append(inner[best[1]])
        else:
            bounds.append((e, e))
    bounds.sort()
    spans = []; cur = t0
    for a, b in bounds:
        spans.append((cur, a)); cur = b
    spans.append((cur, t1))
    return spans


def make_cues(text, lang, t0, t1, gaps_abs):
    limit = 42 if lang == "en" else 20
    sents = split_sentences(text, lang)
    s_spans = assign_times(sents, t0, t1, gaps_abs)
    units = []   # (text, start, end, sentence_final)
    for s, (a, b) in zip(sents, s_spans):
        if wrap2(s, lang, limit) is not None:
            units.append((s, a, b))
        else:   # too long for 2 lines: split at clause boundaries
            cl = split_clauses(s, lang)
            groups = []; cur = ""
            for c in cl:
                sep = "" if lang == "ja" or not cur else " "
                if cur and wrap2(cur + sep + c, lang, limit) is None:
                    groups.append(cur); cur = c
                else:
                    cur = cur + sep + c
            groups.append(cur)
            for gtxt, (ga, gb) in zip(groups, assign_times(groups, a, b, gaps_abs)):
                units.append((gtxt, ga, gb))
    # merge short neighbouring sentences into two-line cues (one unit per line)
    cues = []; i = 0
    while i < len(units):
        txt, a, b = units[i]
        if (i + 1 < len(units) and len(txt) <= limit and len(units[i + 1][0]) <= limit
                and units[i + 1][2] - a <= 3.6 and (b - a) < 1.6):
            cues.append(([txt, units[i + 1][0]], a, units[i + 1][2])); i += 2
        else:
            cues.append((wrap2(txt, lang, limit), a, b)); i += 1
    return cues


def ts(t, sep):
    ms = int(round(t * 1000))
    return "%02d:%02d:%02d%s%03d" % (ms // 3600000, ms // 60000 % 60, ms // 1000 % 60, sep, ms % 1000)


def write_subs(cues, lang):
    d = os.path.join(HERE, "..", "subtitles"); os.makedirs(d, exist_ok=True)
    # small reading pad, never overlapping the next cue
    out = []
    for i, (lines, a, b) in enumerate(cues):
        a2 = max(0.0, a - 0.05)
        nxt = cues[i + 1][1] - 0.05 if i + 1 < len(cues) else DUR
        b2 = min(b + 0.35, nxt - 0.04, DUR)
        out.append((lines, a2, b2))
    with open(os.path.join(d, f"launch30-{lang}.srt"), "w", encoding="utf-8") as f:
        for i, (lines, a, b) in enumerate(out, 1):
            f.write(f"{i}\n{ts(a, ',')} --> {ts(b, ',')}\n" + "\n".join(lines) + "\n\n")
    with open(os.path.join(d, f"launch30-{lang}.vtt"), "w", encoding="utf-8") as f:
        f.write("WEBVTT\n\n")
        for i, (lines, a, b) in enumerate(out, 1):
            f.write(f"{i}\n{ts(a, '.')} --> {ts(b, '.')}\n" + "\n".join(lines) + "\n\n")
    return out


def main():
    narr = json.load(open(os.path.join(HERE, "..", "script", "narration.json"), encoding="utf-8"))
    segs = narr["segments"]
    bad = check_ja_readings(narr)
    if bad:
        print("WARNING: kanji/kana reading mismatch:", bad)
    engine = load_engine()
    timing = {"sample_rate": SR, "lead_in": LEAD, "reading_check_mismatches": len(bad)}
    for lang in ("en", "ja"):
        track = np.zeros(int(SR * DUR))
        info = []; all_cues = []
        lines = list(zip(narr[lang]["vo"], segs))
        cache = {}

        def speech_len(w):
            return len(w) / SR - sum(b - a for a, b in silence_gaps(w, SR))

        def take(text, speed):
            if (text, speed) not in cache:
                cache[(text, speed)] = synth(engine, lang, text, speed)
            return cache[(text, speed)]

        def rate(text, speed):
            return speech_len(take(text, 1.0)) / speech_len(take(text, speed))

        def variants(text, speeds, max_rate=None):
            """Yield (wav, speed, pause_cap), mildest first."""
            for speed in speeds:
                raw = take(text, speed)
                if max_rate is not None and rate(text, speed) > max_rate:
                    continue
                for cap in PAUSE_CAPS:
                    yield (raw if cap is None else tighten_pauses(raw, SR, cap)), speed, cap

        plan = []   # (wav, speed, cap, fits, start)
        last = len(lines) - 1
        for i, (text, (s0, s1)) in enumerate(lines[:last]):
            budget = (s1 - s0) - LEAD - TAIL
            for wav, speed, cap in variants(text, SPEEDS):
                fits = len(wav) / SR <= budget
                if fits:
                    break
            plan.append((wav, speed, cap, fits, s0 + LEAD))
        # Final line (lead decision): it may start before its segment, 0.3 s after the previous
        # line ends, and must end by LAST_END. The previous line may be tightened to <= 1.08.
        text, (s0, s1) = lines[last]
        ps0 = lines[last - 1][1][0]
        done = False
        for wav, speed, cap in variants(text, NOMINAL, LAST_MAX_RATE):
            for pw, pspeed, pcap in [plan[-1][:3]] + [v for v in variants(lines[last - 1][0], NOMINAL, PREV_MAX_RATE)
                                                      if len(v[0]) < len(plan[-1][0])]:
                earliest = ps0 + LEAD + len(pw) / SR + LAST_GAP
                if earliest + len(wav) / SR <= LAST_END:
                    done = True
                    break
            if done:
                break
        plan[-1] = (pw, pspeed, pcap, True, ps0 + LEAD)
        start = max(earliest, min(s0 + LEAD, LAST_END - len(wav) / SR))
        plan.append((wav, speed, cap, done, start))

        for i, ((text, (s0, s1)), (wav, speed, cap, fits, t0)) in enumerate(zip(lines, plan)):
            pk = np.abs(wav).max()
            wav = wav * (10 ** (-3 / 20) / pk)          # per-line peak -3 dBFS
            a = int(round(t0 * SR)); n = min(len(wav), len(track) - a)
            track[a:a + n] += wav[:n]
            t1 = t0 + len(wav) / SR
            gaps = [(t0 + ga, t0 + gb) for ga, gb in silence_gaps(wav, SR)]
            cues = make_cues(text, lang, t0, t1, gaps)
            all_cues += cues
            info.append({"index": i, "segment": [s0, s1], "start": round(t0, 3), "end": round(t1, 3),
                         "duration": round(t1 - t0, 3), "speed": round(speed, 3), "measured_rate": round(rate(text, speed), 3), "pause_cap": cap, "natural_rate": bool(rate(text, speed) <= 1.2), "fits": bool(fits),
                         "margin_to_next_segment": round(s1 - t1, 3), "text": text,
                         "pauses": [[round(x, 3), round(y, 3)] for x, y in gaps]})
            print(f"{lang} line {i}: {t0:.2f}-{t1:.2f}s nominal speed {speed:.2f} measured rate {rate(text, speed):.3f} pause_cap {cap} fits={fits}")
        sf.write(os.path.join(HERE, f"vo-{lang}.wav"), track, SR, subtype="PCM_24")
        subs = write_subs(all_cues, lang)
        timing[lang] = {"voice": VOICES[lang][0], "lines": info,
                        "cues": [{"start": round(a, 3), "end": round(b, 3), "lines": l} for l, a, b in subs]}
    json.dump(timing, open(os.path.join(HERE, "vo-timing.json"), "w", encoding="utf-8"),
              ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
