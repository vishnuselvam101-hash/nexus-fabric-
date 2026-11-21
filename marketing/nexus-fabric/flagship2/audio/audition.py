#!/usr/bin/env python3
"""Objective voice audition for the flagship2 narration.

Nobody can listen in this build environment, so every candidate voice reads the whole script and is
measured: speech-recognition round trip (word / kana error and the recogniser's own confidence),
pace, pauses, clipping, pitch level and pitch movement. Results: audition/report-<lang>.json and
audition/REPORT-<lang>.md. Usage:  python audition.py en|ja [--keep-wav]
"""
import json, os, sys, time
import numpy as np, soundfile as sf
import nf_tts as T

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "audition"); os.makedirs(OUT, exist_ok=True)
NARR = json.load(open(os.path.join(HERE, "..", "script", "narration.json"), encoding="utf-8"))

EN_VOICES = ["af_heart", "af_bella", "af_nicole", "af_aoede", "af_kore", "af_sarah", "am_michael", "am_fenrir", "am_puck",
             "bf_emma", "bf_isabella", "bm_george", "bm_fable", "bm_lewis"]
JA_KOKORO = ["jf_alpha", "jf_gongitsune", "jm_kumo"]
KANA = {}
# VOICEVOX styles auditioned: the nine neutral "Nemo" voices and the calm narrator-like character styles whose terms allow
# commercial use with a credit line and no prior confirmation (see VOICES.md). Character-acting styles are left out.
JA_VOICEVOX = [10005, 10007, 10004, 10003, 10008, 10006, 10001, 10000, 10002, 16, 11, 52, 53, 99, 101, 100, 109, 107]


def squim():
    """torchaudio's reference-free estimates of STOI / PESQ / SI-SDR: a cleanliness check, not a naturalness score."""
    try:
        import torch, torchaudio
        from torchaudio.pipelines import SQUIM_OBJECTIVE
        m = SQUIM_OBJECTIVE.get_model()
        def f(x):
            y = torchaudio.functional.resample(torch.tensor(x, dtype=torch.float32)[None], T.SR, 16000)
            with torch.no_grad():
                st, pq, sd = m(y)
            return {"stoi_est": round(float(st), 3), "pesq_est": round(float(pq), 2), "sisdr_est_db": round(float(sd), 1)}
        return f
    except Exception as e:                                     # optional
        print("squim unavailable:", e); return lambda x: {}


def run(lang, keep):
    lines = NARR[lang]["lines"]; sq = squim(); rows = []
    cands = []
    if lang == "en":
        for v in EN_VOICES:
            cands.append({"engine": "kokoro", "voice": v, "label": v, "say": lambda t, v=v: T.kokoro_say(t, v, 1.0, "en-gb" if v[0] == "b" else "en-us")})
    else:
        for v in JA_KOKORO:
            cands.append({"engine": "kokoro", "voice": v, "label": v, "say": lambda t, v=v: T.kokoro_say(t, v, 1.0, "ja")})
        syn, names = T.voicevox()
        for sid in JA_VOICEVOX:
            nm = names[sid]
            cands.append({"engine": "voicevox", "voice": sid, "label": f"VOICEVOX {nm} ({sid})", "say": lambda t, sid=sid: T.voicevox_say(kana=KANA[t], style_id=sid)[0]})
    if "--only-voicevox" in sys.argv:
        cands = [c for c in cands if c["engine"] == "voicevox"]
        rows = [r for r in json.load(open(os.path.join(OUT, f"report-{lang}.json"), encoding="utf-8")) if r["engine"] != "voicevox"]
    for c in cands:
        t0 = time.time(); per = []; allx = []
        for ln in lines:
            say = ln.get("say", ln["text"])
            if lang == "ja":
                say = ln["text"].replace("NEXUS FABRIC", "ネクサス・ファブリック").replace("DXF", "ディーエックスエフ").replace("「", "").replace("」", "")
                KANA[say] = ln.get("kana")
            try:
                x = T.trim(c["say"](say))
            except Exception as e:
                per = None; print("FAILED", c["label"], e); break
            m = T.measure(x)
            if lang == "en":
                m["units"] = len(T.norm_en(ln["text"]))
            else:
                m["units"] = len(T.ja_kana(ln["reading"]))                    # kana characters ~ morae
            m.update(sq(x)); per.append(m); allx.append(x)
        if per is None:
            continue
        # recognise all lines in one pass (0.9 s of silence between lines), then give each word to the line it falls in
        gap = np.zeros(int(0.9 * T.SR)); cat = np.concatenate([np.concatenate([x, gap]) for x in allx])
        _, lp, words = T.asr(cat, lang); t = 0.0
        for ln, x, m in zip(lines, allx, per):
            a, b = t - 0.45, t + len(x) / T.SR + 0.45; t += len(x) / T.SR + 0.9
            hyp = ("" if lang == "ja" else " ").join(w for w, ws, we in words if a <= (ws + we) / 2 < b)
            m["asr"] = hyp; m["asr_logprob"] = lp
            m["err"] = T.wer(ln["text"], hyp) if lang == "en" else T.cer_ja(ln["reading"], hyp)
        if keep:
            gap = np.zeros(int(0.5 * T.SR)); sf.write(os.path.join(OUT, f"{lang}-{c['label'].replace(' ', '_').replace('/', '-')}.flac"),
                                                     np.concatenate([np.concatenate([x, gap]) for x in allx]), T.SR, subtype="PCM_16")
        speech = sum(p["speech_s"] for p in per); units = sum(p["units"] for p in per)
        f0 = [p["f0_median_hz"] for p in per if p.get("f0_median_hz")]
        row = {"label": c["label"], "engine": c["engine"], "voice": c["voice"],
               "total_s": round(sum(p["duration_s"] for p in per), 2), "speech_s": round(speech, 2),
               "rate_units_per_s": round(units / speech, 2), "mean_err": round(float(np.mean([p["err"] for p in per])), 4),
               "lines_exact": int(sum(p["err"] == 0 for p in per)), "asr_logprob": round(float(np.mean([p["asr_logprob"] for p in per])), 3),
               "f0_median_hz": round(float(np.median(f0)), 1) if f0 else None,
               "f0_range_st": round(float(np.mean([p["f0_range_st"] for p in per if p.get("f0_range_st")])), 2),
               "f0_sd_st": round(float(np.mean([p["f0_sd_st"] for p in per if p.get("f0_sd_st")])), 2),
               "pauses_n": int(sum(len(p["pauses"]) for p in per)), "pause_mean_s": round(float(np.mean([b - a for p in per for a, b in p["pauses"]] or [0])), 3),
               "clipped": int(sum(p["clipped_samples"] for p in per)), "peak_dbfs": max(p["peak_dbfs"] for p in per),
               "pesq_est": round(float(np.mean([p.get("pesq_est", 0) for p in per])), 2), "stoi_est": round(float(np.mean([p.get("stoi_est", 0) for p in per])), 3),
               "seconds_to_run": round(time.time() - t0, 1), "lines": per}
        rows.append(row)
        print(f"{row['label']:<40} err {row['mean_err']:.3f} exact {row['lines_exact']}/{len(lines)} logp {row['asr_logprob']:.2f} rate {row['rate_units_per_s']:.2f} "
              f"f0 {row['f0_median_hz']} range {row['f0_range_st']} pesq {row['pesq_est']} total {row['total_s']}", flush=True)
        json.dump(rows, open(os.path.join(OUT, f"report-{lang}.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    unit = "words/s" if lang == "en" else "kana/s"
    with open(os.path.join(OUT, f"REPORT-{lang}.md"), "w", encoding="utf-8") as f:
        f.write(f"# Voice audition, {lang}: every candidate read the whole script at its default speed\n\n"
                f"Recogniser: faster-whisper `{os.environ.get('NF_WHISPER', 'large-v3-turbo')}`. Error = word error (en) or kana error (ja) against the script; "
                "confidence = the recogniser's mean log-probability (closer to 0 is clearer). PESQ/STOI are torchaudio SQUIM estimates of signal cleanliness, not naturalness. "
                "Nobody listened.\n\n"
                f"| Voice | Error | Lines exact | Confidence | Pace, {unit} | Median pitch, Hz | Pitch range, st | Pitch s.d., st | Pauses | PESQ est. | Clipped |\n|---|---|---|---|---|---|---|---|---|---|---|\n")
        for r in sorted(rows, key=lambda r: (r["mean_err"], -r["asr_logprob"])):
            f.write(f"| {r['label']} | {r['mean_err']:.3f} | {r['lines_exact']}/{len(lines)} | {r['asr_logprob']:.2f} | {r['rate_units_per_s']:.2f} | {r['f0_median_hz']} | {r['f0_range_st']} | {r['f0_sd_st']} | "
                    f"{r['pauses_n']} × {r['pause_mean_s']:.2f} s | {r['pesq_est']} | {r['clipped']} |\n")


if __name__ == "__main__":
    run(sys.argv[1], "--keep-wav" in sys.argv)
