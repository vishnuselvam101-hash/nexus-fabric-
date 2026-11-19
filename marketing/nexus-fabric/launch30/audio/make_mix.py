#!/usr/bin/env python3
"""NEXUS FABRIC launch30 - final mixes + automated checks.

mix-en.wav / mix-ja.wav : voice + music + sfx, music side-chain ducked under the voice,
                          -16 LUFS integrated, true peak <= -1 dBTP (measured with ffmpeg ebur128)
music-sfx-only.wav      : un-ducked bed, same loudness target, for voiceless cuts
checks.json             : every automated check and measured number
"""
import json, os, re, subprocess, sys
import numpy as np
import soundfile as sf
from scipy.signal import butter, sosfilt, sosfiltfilt, resample_poly
from scipy.ndimage import maximum_filter1d, minimum_filter1d, uniform_filter1d

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 48000
N = SR * 30
TARGET_LUFS = -16.0
TP_CEIL = -1.0
DUCK_DB = -9.0        # music under voice
SFX_DUCK_DB = -3.0


def rd(name):
    x, sr = sf.read(os.path.join(HERE, name), dtype="float64")
    assert sr == SR, name
    return x


def measure(path):
    """Integrated loudness / LRA / true peak via ffmpeg's EBU R128 meter."""
    r = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", path, "-af", "ebur128=peak=true",
                        "-f", "null", "-"], capture_output=True, text=True)
    s = r.stderr[r.stderr.rindex("Summary:"):]
    g = lambda pat: float(re.search(pat, s).group(1))
    return {"I_LUFS": g(r"I:\s+(-?[\d.]+) LUFS"), "LRA_LU": g(r"LRA:\s+(-?[\d.]+) LU"),
            "true_peak_dBTP": g(r"Peak:\s+(-?[\d.]+) dBFS")}


def envelope_gain(vo, depth_db, attack=0.09, release=0.45, lookahead=0.06):
    """Smooth side-chain gain curve (linear), 1.0 = no ducking."""
    env = np.sqrt(np.maximum(uniform_filter1d(vo ** 2, int(0.03 * SR)), 0))
    act = (env > 10 ** (-42 / 20)).astype(float)
    act = maximum_filter1d(act, int(0.30 * SR))             # hold through word gaps
    act = np.roll(act, -int(lookahead * SR)); act[-int(lookahead * SR):] = 0
    out = np.empty_like(act); y = 0.0
    ca = np.exp(-1 / (attack * SR)); cr = np.exp(-1 / (release * SR))
    for i in range(len(act)):                               # one-pole attack/release
        c = ca if act[i] > y else cr
        y = c * y + (1 - c) * act[i]
        out[i] = y
    return 10 ** (depth_db * out / 20)


def voice_chain(vo):
    v = sosfilt(butter(2, 85, "high", fs=SR, output="sos"), vo)
    # gentle 2.5:1 compression above -20 dBFS (RMS detector), then small presence shelf
    env = np.sqrt(np.maximum(uniform_filter1d(v ** 2, int(0.02 * SR)), 0)) + 1e-9
    over = np.maximum(20 * np.log10(env) + 20.0, 0)
    gr = uniform_filter1d(-over * (1 - 1 / 2.5), int(0.05 * SR))
    v = v * 10 ** (gr / 20)
    pres = sosfilt(butter(1, [2500, 6000], "band", fs=SR, output="sos"), v)
    return v + 0.18 * pres


def limit(x, ceil_db):
    """Look-ahead peak limiter working on the 4x oversampled peak estimate."""
    up = np.abs(resample_poly(x, 4, 1, axis=0)).max(axis=1)
    pk = up.reshape(-1, 4).max(axis=1)
    c = 10 ** (ceil_db / 20)
    g = np.minimum(1.0, c / np.maximum(pk, 1e-9))
    la = int(0.004 * SR)
    g = minimum_filter1d(g, 2 * la + 1)
    g = uniform_filter1d(g, la)
    return x * g[:, None], float(20 * np.log10(g.min()))


def master(x, path):
    """Gain to target loudness, limit, verify with ffmpeg; iterate until on target."""
    gain_db = 0.0; ceil = TP_CEIL - 0.3
    tmp = path
    for it in range(8):
        y, gr = limit(x * 10 ** (gain_db / 20), ceil)
        sf.write(tmp, y, SR, subtype="PCM_24")
        m = measure(tmp)
        err = TARGET_LUFS - m["I_LUFS"]
        if abs(err) <= 0.05 and m["true_peak_dBTP"] <= TP_CEIL:
            break
        if m["true_peak_dBTP"] > TP_CEIL:
            ceil -= (m["true_peak_dBTP"] - TP_CEIL) + 0.1
        gain_db += err
    m["limiter_max_gain_reduction_dB"] = round(gr, 2)
    m["master_gain_dB"] = round(gain_db, 2)
    return y, m


def main():
    music = rd("music.wav"); sfx = rd("sfx.wav")
    timing = json.load(open(os.path.join(HERE, "vo-timing.json"), encoding="utf-8"))
    report = {"targets": {"I_LUFS": TARGET_LUFS, "true_peak_max_dBTP": TP_CEIL}, "files": {}, "vo_lines": {}}
    ok = True

    for lang in ("en", "ja"):
        vo = rd(f"vo-{lang}.wav")
        v = voice_chain(vo)
        duck = envelope_gain(vo, DUCK_DB)
        sduck = envelope_gain(vo, SFX_DUCK_DB)
        # level the voice against the bed: voice RMS (active) ~ 11 dB above ducked bed
        act = np.abs(vo) > 10 ** (-42 / 20)
        v_rms = np.sqrt((v[act] ** 2).mean())
        bed = music * 0.9 * duck[:, None] + sfx * 0.8 * sduck[:, None]
        v = v * (10 ** (-20 / 20) / v_rms)
        mix = bed + np.stack([v, v], axis=1)
        _, m = master(mix, os.path.join(HERE, f"mix-{lang}.wav"))
        m["music_duck_dB"] = DUCK_DB
        report["files"][f"mix-{lang}.wav"] = m

    _, m = master(music * 0.9 + sfx * 0.8, os.path.join(HERE, "music-sfx-only.wav"))
    report["files"]["music-sfx-only.wav"] = m

    # ---------------- automated checks ----------------
    for name in ("vo-en.wav", "vo-ja.wav", "music.wav", "sfx.wav", "mix-en.wav", "mix-ja.wav", "music-sfx-only.wav"):
        info = sf.info(os.path.join(HERE, name))
        x = rd(name)
        e = report["files"].setdefault(name, measure(os.path.join(HERE, name)))
        e.update({"sample_rate": info.samplerate, "channels": info.channels, "frames": info.frames,
                  "duration_s": round(info.frames / info.samplerate, 6),
                  "sample_peak_dBFS": round(float(20 * np.log10(np.abs(x).max())), 2),
                  "clipped_samples": int((np.abs(x) >= 0.999).sum()),
                  "ends_silent": bool(np.abs(x[-int(0.05 * SR):]).max() < 10 ** (-60 / 20))})
        good = info.samplerate == SR and info.frames == N and e["clipped_samples"] == 0
        if name.startswith(("mix", "music-sfx")):
            good = good and abs(e["I_LUFS"] - TARGET_LUFS) <= 0.5 and e["true_peak_dBTP"] <= TP_CEIL
        e["pass"] = bool(good); ok = ok and good

    for lang in ("en", "ja"):
        vo = rd(f"vo-{lang}.wav"); rows = []
        L = timing[lang]["lines"]
        for k, ln in enumerate(L):
            s0, s1 = ln["segment"]
            final = k == len(L) - 1
            # Lead decision: the final line may start early (>= 0.3 s after the previous line)
            # and must end by 29.7 s. Every other line stays inside its own segment.
            w0 = L[k - 1]["end"] if k else 0.0
            w1 = 29.7 if final else s1
            i0 = int((w0 + 0.01) * SR) if final else int(s0 * SR)
            i1 = int(s1 * SR) if final else int(min(s1, L[k + 1]["start"] - 0.01) * SR)
            seg = vo[i0:i1]
            idx = np.where(np.abs(seg) > 10 ** (-50 / 20))[0]
            rms = 20 * np.log10(np.sqrt((seg ** 2).mean()) + 1e-12)
            first = i0 / SR + idx[0] / SR if len(idx) else None
            last = i0 / SR + idx[-1] / SR if len(idx) else None
            gap_prev = round(ln["start"] - w0, 3) if k else None
            inside = bool(len(idx) and last <= w1 + 0.005 and (first >= s0 or (final and gap_prev >= 0.3)))
            row = {"line": ln["index"] + 1, "segment": ln["segment"], "first_sound_s": round(first, 3) if first else None,
                   "last_sound_s": round(last, 3) if last else None, "gap_after_previous_line_s": gap_prev,
                   "segment_rms_dBFS": round(float(rms), 1), "nominal_speed": ln["speed"],
                   "measured_rate": ln["measured_rate"], "pause_cap_s": ln["pause_cap"],
                   "starts_before_segment": bool(first is not None and first < s0),
                   "silent": bool(rms < -45), "inside_window": inside}
            row["pass"] = (not row["silent"]) and inside and ln["fits"]
            ok = ok and row["pass"]
            rows.append(row)
        report["vo_lines"][lang] = rows

    # lossless FLAC copies (24-bit) of the stems the edit needs
    report["flac"] = {}
    for name in ("vo-en", "vo-ja", "music-sfx-only"):
        x = rd(name + ".wav")
        sf.write(os.path.join(HERE, name + ".flac"), x, SR, format="FLAC", subtype="PCM_24")
        y, sr2 = sf.read(os.path.join(HERE, name + ".flac"), dtype="float64")
        same = sr2 == SR and y.shape == x.shape and float(np.abs(y - x).max()) < 2 ** -22
        report["flac"][name + ".flac"] = {"frames": len(y), "identical_to_wav": bool(same)}
        ok = ok and same

    report["all_pass"] = bool(ok)
    json.dump(report, open(os.path.join(HERE, "checks.json"), "w"), indent=1)
    for k, e in report["files"].items():
        print(f"{k:20s} {e['duration_s']:.3f}s  I={e['I_LUFS']:6.1f} LUFS  TP={e['true_peak_dBTP']:5.1f} dBTP  "
              f"peak={e['sample_peak_dBFS']:6.2f} dBFS  clipped={e['clipped_samples']}  pass={e['pass']}")
    for lang, rows in report["vo_lines"].items():
        for r in rows:
            print(f"vo-{lang} line {r['line']}: {r['first_sound_s']}-{r['last_sound_s']} s  segment {r['segment']}  "
                  f"rate {r['measured_rate']}  gap_prev {r['gap_after_previous_line_s']}  pass={r['pass']}")
    print("ALL PASS" if ok else "CHECKS FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
