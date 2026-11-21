#!/usr/bin/env python3
"""NEXUS FABRIC flagship film 2 - the final sound of each language: the voice over the bed.

Reads    audio/vo-<lang>.wav    the narration on the film timeline (50.0 s, mono, from make_voice.py)
         audio/bed-<lang>.wav   score and effects (50.0 s, stereo, from make_music.py)
Writes   audio/mix-<lang>.wav   50.0 s, 48 kHz, 24-bit, stereo: voice over bed, mastered
         audio/mix-report.json  every measured number

Levels, all measured with ffmpeg's EBU R128 meter (ebur128, true peak):
  - the bed is ducked under the voice (side-chain, 4 dB while a word is sounding);
  - the voice is at least 12 LU above the bed: the bed is set so that the voice's integrated loudness minus the
    bed's integrated loudness (bed without the duck, the stricter case) is 12.5 LU;
  - the master is set to -14 LUFS integrated, true peak at or below -1 dBTP (look-ahead limiter, verified by ffmpeg).
Temporary stems are written to $NF_SCRATCH (or the system temporary folder), never into the repository.
"""
import json, os, re, subprocess, tempfile
import numpy as np
import soundfile as sf
from scipy.ndimage import maximum_filter1d, minimum_filter1d, uniform_filter1d
from scipy.signal import butter, resample_poly, sosfilt

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 48000
N = SR * 50
TARGET_LUFS = -14.0
TP_CEIL = -1.0
MARGIN_LU = 12.0
MARGIN_AIM = 12.5
DUCK_DB = -4.0
SCR = os.environ.get("NF_SCRATCH") or tempfile.gettempdir()


def rd(name):
    if name.startswith("vo-") and not os.path.exists(os.path.join(HERE, name)):
        name = name[:-4] + ".flac"                               # the voice is kept as lossless FLAC in the repository
    x, sr = sf.read(os.path.join(HERE, name), dtype="float64")
    assert sr == SR, (name, sr)
    return x


def measure(path):
    """integrated loudness, loudness range and true peak from ffmpeg's EBU R128 summary"""
    r = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", path, "-af", "ebur128=peak=true", "-f", "null", "-"],
                       capture_output=True, text=True)
    s = r.stderr[r.stderr.rindex("Summary:"):]
    I = float(re.search(r"I:\s+(-?[\d.]+) LUFS", s).group(1))
    LRA = float(re.search(r"LRA:\s+(-?[\d.]+) LU", s).group(1))
    TP = float(re.search(r"True peak:\s*\n\s*Peak:\s+(-?[\d.]+) dBFS", s).group(1))
    return {"I_LUFS": I, "LRA_LU": LRA, "true_peak_dBTP": TP}


def write_tmp(name, x):
    os.makedirs(SCR, exist_ok=True)
    p = os.path.join(SCR, name)
    sf.write(p, x, SR, subtype="PCM_24")
    return p


def envelope_gain(vo, depth_db, attack=0.09, release=0.45, lookahead=0.06):
    """side-chain gain curve (linear): 1.0 with no voice, 10^(depth/20) while a word sounds"""
    env = np.sqrt(np.maximum(uniform_filter1d(vo ** 2, int(0.03 * SR)), 0))
    act = (env > 10 ** (-42 / 20)).astype(float)
    act = maximum_filter1d(act, int(0.30 * SR))
    act = np.roll(act, -int(lookahead * SR)); act[-int(lookahead * SR):] = 0
    out = np.empty_like(act); y = 0.0
    ca = np.exp(-1 / (attack * SR)); cr = np.exp(-1 / (release * SR))
    for i in range(len(act)):
        c = ca if act[i] > y else cr
        y = c * y + (1 - c) * act[i]
        out[i] = y
    return 10 ** (depth_db * out / 20)


def voice_chain(vo):
    """high-pass, a 3.5:1 compressor above -24 dBFS (20 ms RMS detector), a small presence band"""
    v = sosfilt(butter(2, 85, "high", fs=SR, output="sos"), vo)
    env = np.sqrt(np.maximum(uniform_filter1d(v ** 2, int(0.02 * SR)), 0)) + 1e-9
    over = np.maximum(20 * np.log10(env) + 24.0, 0)
    gr = uniform_filter1d(-over * (1 - 1 / 3.5), int(0.05 * SR))
    v = v * 10 ** (gr / 20)
    pres = sosfilt(butter(1, [2500, 6000], "band", fs=SR, output="sos"), v)
    return v + 0.18 * pres


def limit(x, ceil_db):
    """look-ahead peak limiter on a 4x oversampled peak estimate; returns the limited signal and the largest reduction"""
    up = np.abs(resample_poly(x, 4, 1, axis=0)).max(axis=1)
    pk = up.reshape(-1, 4).max(axis=1)
    c = 10 ** (ceil_db / 20)
    g = np.minimum(1.0, c / np.maximum(pk, 1e-9))
    la = int(0.004 * SR)
    g = minimum_filter1d(g, 2 * la + 1)
    g = uniform_filter1d(g, la)
    return x * g[:, None], float(20 * np.log10(g.min()))


def master(x, path):
    """find the gain that gives TARGET_LUFS with the true peak at or below TP_CEIL, verified with ffmpeg"""
    gain_db = 0.0; ceil = TP_CEIL - 0.3; y = x; gr = 0.0; m = {}
    for it in range(12):
        y, gr = limit(x * 10 ** (gain_db / 20), ceil)
        sf.write(path, y, SR, subtype="PCM_24")
        m = measure(path)
        err = TARGET_LUFS - m["I_LUFS"]
        if abs(err) <= 0.05 and m["true_peak_dBTP"] <= TP_CEIL:
            break
        if m["true_peak_dBTP"] > TP_CEIL:
            ceil -= (m["true_peak_dBTP"] - TP_CEIL) + 0.1
        gain_db += err
    m["master_gain_dB"] = round(gain_db, 2)
    m["limiter_max_reduction_dB"] = round(gr, 2)
    m["iterations"] = it + 1
    return y, m


def main():
    report = {"targets": {"I_LUFS": TARGET_LUFS, "true_peak_max_dBTP": TP_CEIL, "voice_over_bed_min_LU": MARGIN_LU}, "langs": {}}
    ok = True
    for lang in ("en", "ja"):
        vo = rd(f"vo-{lang}.wav")
        if vo.ndim > 1:
            vo = vo.mean(axis=1)
        bed = rd(f"bed-{lang}.wav")
        assert bed.shape == (N, 2) and len(vo) == N, (lang, bed.shape, len(vo))
        voice = voice_chain(vo)
        duck = envelope_gain(vo, DUCK_DB)
        vp = write_tmp(f"stem-voice-{lang}.wav", np.stack([voice, voice], axis=1))
        bp = write_tmp(f"stem-bed-{lang}.wav", bed)
        Iv = measure(vp)["I_LUFS"]; Ib = measure(bp)["I_LUFS"]
        gain_bed = (Iv - MARGIN_AIM) - Ib                       # bed level in dB so that voice - bed = 12.5 LU
        g = 10 ** (gain_bed / 20)
        bed_ducked = bed * (duck * g)[:, None]
        mix = np.stack([voice, voice], axis=1) + bed_ducked
        final, m = master(mix, os.path.join(HERE, f"mix-{lang}.wav"))
        # the voice and the bed after the same master gain, measured separately (the margin the listener gets)
        mg = 10 ** (m["master_gain_dB"] / 20)
        Iv_f = measure(write_tmp(f"stem-voice-final-{lang}.wav", np.stack([voice, voice], axis=1) * mg))["I_LUFS"]
        Ib_f = measure(write_tmp(f"stem-bed-final-{lang}.wav", bed * g * mg))["I_LUFS"]
        info = sf.info(os.path.join(HERE, f"mix-{lang}.wav"))
        entry = {**m, "voice_I_LUFS_stem": round(Iv, 2), "bed_I_LUFS_stem_unducked": round(Ib, 2), "bed_gain_dB": round(gain_bed, 2),
                 "voice_over_bed_LU_stems": round(Iv - Ib - gain_bed, 2), "voice_over_bed_LU_final_stems": round(Iv_f - Ib_f, 2),
                 "duck_dB": DUCK_DB, "sample_rate": info.samplerate, "channels": info.channels, "frames": info.frames,
                 "duration_s": round(info.frames / info.samplerate, 6), "sample_peak_dBFS": round(float(20 * np.log10(np.abs(final).max())), 2),
                 "clipped_samples": int((np.abs(final) >= 0.9999).sum())}
        good = (abs(entry["I_LUFS"] - TARGET_LUFS) <= 0.1 and entry["true_peak_dBTP"] <= TP_CEIL and entry["voice_over_bed_LU_stems"] >= MARGIN_LU
                and entry["sample_rate"] == SR and entry["frames"] == N and entry["clipped_samples"] == 0)
        entry["pass"] = bool(good); ok = ok and good
        report["langs"][lang] = entry
        print(lang, json.dumps(entry))
    json.dump(report, open(os.path.join(HERE, "mix-report.json"), "w"), indent=1)
    print("ALL PASS" if ok else "CHECK FAILED")


if __name__ == "__main__":
    main()
