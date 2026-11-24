#!/usr/bin/env python3
"""Music-and-effects bed for the notch film (no voice): runs make_music.py (original score and sound design synthesised
in code; effects follow ../cues.json), then mixes and masters to about -16 LUFS with true peak below -1.5 dBTP.
Output: bed.wav (48 kHz stereo, 30.000 s) and bed-report.json.   Usage: python make_bed.py"""
import json, os, re, subprocess, sys
import numpy as np, soundfile as sf
TARGET = float(os.environ.get("NF_LUFS", "-16"))
HERE = os.path.dirname(os.path.abspath(__file__))
subprocess.run([sys.executable, os.path.join(HERE, "make_music.py")], check=True)
m, sr = sf.read(os.path.join(HERE, "music.wav")); s, _ = sf.read(os.path.join(HERE, "sfx.wav"))
x = m * 0.9 + s * 0.8
def measure(path):
    out = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", path, "-af", "ebur128=peak=true", "-f", "null", "-"], capture_output=True, text=True).stderr
    tail = out[out.rfind("Summary:"):]
    return float(re.search(r"I:\s*(-?[\d.]+) LUFS", tail).group(1)), float(re.search(r"Peak:\s*(-?[\d.]+) dBFS", tail).group(1))
out = os.path.join(HERE, "bed.wav"); gain = 1.0
for _ in range(6):
    y = x * gain; ceil = 10 ** (-1.8 / 20); y = np.where(np.abs(y) > ceil * 0.8, np.sign(y) * (ceil * 0.8 + (ceil * 0.2) * np.tanh((np.abs(y) - ceil * 0.8) / (ceil * 0.2))), y)
    sf.write(out, y, sr, subtype="PCM_24"); i, p = measure(out)
    if abs(i - TARGET) < 0.1: break
    gain *= 10 ** ((TARGET - i) / 20)
rep = {"file": "bed.wav", "integrated_lufs": i, "true_peak_dbfs": p, "samples": int(len(y)), "seconds": len(y) / sr, "clipped_samples": int((np.abs(y) >= 1.0).sum())}
json.dump(rep, open(os.path.join(HERE, "bed-report.json"), "w"), indent=1); print(rep)
