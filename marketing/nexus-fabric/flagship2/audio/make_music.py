#!/usr/bin/env python3
"""NEXUS FABRIC flagship film 2 - the score and the sound on the picture (the bed; the voice is added by make_mix.py).

Reads   audio/cues-<lang>.json   (window.NF_CUES and the timeline anchors, exported by `node render.js cues <lang> <out>`)
Writes  audio/bed-<lang>.wav     50.0 s, 48 kHz, 24-bit, stereo: score and effects, no voice

Score: one slow pulse in D (40 beats a minute, the first beat at 0.6 s), a drone on D2 and A2 that opens up around
the equation (13 s to 20 s) and resolves to D at the mark (38 s to 46 s), and a D-major chord when the mark lands.
Effects: one designed sound for each cue the picture asks for (keys, detents, press, result, roll of the steel,
lock, stamp, the DXF stream and pen, the mark). Every effect is kept low; the mix in make_mix.py sets the levels.
Everything is synthesised here from sines, filtered noise and envelopes. No sample library is used.
"""
import json, os
import numpy as np
import soundfile as sf
from scipy.signal import butter, sosfilt

HERE = os.path.dirname(os.path.abspath(__file__))
SR, DUR = 48000, 50.0
N = int(round(SR * DUR))
rng = np.random.default_rng(20261120)
BL = np.zeros(N); BR = np.zeros(N)                       # the bed, left and right


def tt(d):
    return np.arange(int(round(d * SR))) / SR


def put(sig, t, pan=0.0, gain=1.0):
    """add a mono signal that starts at time t (seconds); pan -1 (left) .. +1 (right); equal-power"""
    i0 = int(round(t * SR)); s = np.asarray(sig, dtype=float)
    if i0 < 0:
        s = s[-i0:]; i0 = 0
    n = min(len(s), N - i0)
    if n <= 0:
        return
    a = np.sqrt((1 - pan) / 2) * gain; b = np.sqrt((1 + pan) / 2) * gain
    BL[i0:i0 + n] += a * s[:n]; BR[i0:i0 + n] += b * s[:n]


def hp(x, f):
    return sosfilt(butter(2, f, "high", fs=SR, output="sos"), x)


def lp(x, f):
    return sosfilt(butter(2, f, "low", fs=SR, output="sos"), x)


def bp(x, lo, hi):
    return sosfilt(butter(2, [lo, hi], "band", fs=SR, output="sos"), x)


def unit(x):
    return x / (np.abs(x).max() + 1e-12)


# ------------------------------------------------------------------ effects (each returns a mono signal)
def click(amp=0.2, pan=0.0, f=2600):
    d = 0.025
    x = hp(rng.standard_normal(int(d * SR)), f) * np.exp(-tt(d) / 0.0035)
    return amp * unit(x), pan


def tick(f=2400, amp=0.05, pan=0.0, d=0.04, tau=0.007):
    return amp * np.sin(2 * np.pi * f * tt(d)) * np.exp(-tt(d) / tau), pan


def thud(f0=70.0, amp=0.2, d=0.45, pan=0.0):
    ph = 2 * np.pi * np.cumsum(f0 * (1 + 1.6 * np.exp(-tt(d) / 0.025))) / SR
    return amp * np.sin(ph) * np.exp(-tt(d) / (d / 4.0)), pan


def whoosh(d=1.0, amp=0.05, lo=400, hi=3200, pan=0.0, rise=True):
    x = bp(rng.standard_normal(int(d * SR)), lo, hi)
    u = np.linspace(0, 1, len(x))
    e = np.sin(np.pi * u) ** 1.6
    x = unit(x * e)
    if not rise:
        x = x[::-1].copy()
    return amp * x, pan


def chime(freqs=(293.66, 440.0, 587.33), amp=0.08, d=1.6, pan=0.0, tau=0.7):
    x = np.zeros(int(d * SR)); t = tt(d)
    for k, f in enumerate(freqs):
        x += (0.55 ** k) * np.sin(2 * np.pi * f * t) * np.exp(-t / (tau + 0.25 * k))
    return amp * unit(x), pan


def roll(d=1.7, amp=0.09, pan=0.0):
    """the roll of a steel sheet: band noise that swells, with a rattle of small clicks"""
    x = bp(rng.standard_normal(int(d * SR)), 900, 5200)
    u = np.linspace(0, 1, len(x))
    x = x * (np.sin(np.pi * u) ** 2) * (0.35 + 0.65 * u)
    rattle = np.zeros(len(x))
    for k in range(int(d * 26)):
        i = int((k + rng.uniform(0.2, 0.8)) / 26 * SR)
        if i < len(x):
            rattle[i] += rng.uniform(0.3, 1.0)
    x = unit(x) + 0.35 * unit(hp(rattle, 1500))
    return amp * unit(x), pan


def stream(d=3.5, amp=0.05, pan=0.0):
    """the DXF text running past: many very short ticks, uneven"""
    x = np.zeros(int(d * SR))
    n = int(d * 24)
    for k in range(n):
        i = int((k + rng.uniform(0.1, 0.9)) / 24 * SR)
        if i < len(x):
            x[i:i + 200] += rng.uniform(0.4, 1.0) * np.exp(-np.arange(min(200, len(x) - i)) / 30.0) * np.sin(2 * np.pi * 3300 * np.arange(min(200, len(x) - i)) / SR)
    return amp * unit(x), pan


def pen(d=1.6, amp=0.02, pan=0.0):
    x = bp(rng.standard_normal(int(d * SR)), 2200, 7000)
    u = np.linspace(0, 1, len(x))
    return amp * unit(x * np.sin(np.pi * u) ** 2), pan


def stamp(amp=0.16, pan=0.0):
    x, _ = thud(f0=95, amp=1.0, d=0.22)
    n, _ = click(amp=1.0, f=1800)
    m = min(len(x), len(n))
    x = x[:m] * 0.8 + n[:m] * 0.45
    return amp * unit(x), pan


def chord(freqs, amp, d):
    return chime(freqs, amp=amp, d=d, tau=1.1)


# ------------------------------------------------------------------ the score (the drone and the pulse)
def drone():
    t = np.arange(N) / SR
    opn = np.interp(t, [0, 12.5, 14.6, 20.0, 37.0, 39.5, 50], [0.10, 0.10, 0.65, 0.65, 0.40, 0.12, 0.10])   # brightness
    env = np.interp(t, [0, 2.0, 46.0, 49.5, 50], [0.0, 1.0, 0.9, 0.0, 0.0])
    out = [np.zeros(N), np.zeros(N)]
    for ch, ph in ((0, 0.0), (1, 1.7)):
        s = np.zeros(N)
        for f, a in ((73.42, 0.22), (110.0, 0.13), (146.83, 0.08)):
            s += a * np.sin(2 * np.pi * f * (1 + 0.0015 * np.sin(2 * np.pi * 0.19 * t + f + ph)) * t + ph)
        for f, a in ((293.66, 0.05), (440.0, 0.03), (587.33, 0.015)):
            s += opn * a * np.sin(2 * np.pi * f * (1 + 0.0010 * np.sin(2 * np.pi * 0.27 * t + ph)) * t + ph)
        out[ch] = env * s
    return out


def pulse():
    """one slow pulse in D: a low D1 thud every 1.5 s from 0.6 s to 46 s; a little stronger on the bar's first beat"""
    p = 1.5; t0 = 0.6
    for k in range(int((46.0 - t0) / p) + 1):
        t = t0 + k * p
        if t > 46.0:
            break
        amp = 0.13 if k % 4 == 0 else 0.085
        if t < 2.0:
            amp *= 0.55
        x, pan = thud(f0=36.71, amp=amp, d=0.42)
        put(x, t, pan)


# ------------------------------------------------------------------ the picture's cues -> sounds
def sound_for(c):
    k = c["k"]; d = float(c.get("d", 0.6) or 0.6); i = int(c.get("i", 0) or 0)
    if k == "line_draw": return whoosh(max(d, 0.6), 0.05, 500, 3500, pan=-0.2)
    if k == "drop_in": return thud(f0=82, amp=0.17, d=0.35)
    if k == "seat": return thud(f0=96, amp=0.11, d=0.18)
    if k == "ignite": return chime((220.0, 440.0), amp=0.07, d=1.1, tau=0.5)
    if k == "misfit": return chime((293.66, 415.3), amp=0.07, d=0.9, tau=0.25)
    if k == "turn": return whoosh(max(d, 0.5), 0.045, 300, 2400, pan=0.3)
    if k == "match": return chime((587.33, 880.0), amp=0.06, d=1.2, tau=0.4)
    if k == "window_out": return whoosh(max(d, 0.6), 0.05, 250, 2000, pan=0.0, rise=False)
    if k == "click": return click(amp=0.16, pan=0.0)
    if k == "key": return click(amp=0.12, pan=-0.1, f=3200)
    if k == "detent": return tick(f=3200, amp=0.045, pan=0.12, d=0.02, tau=0.004)
    if k == "press": return thud(f0=72, amp=0.12, d=0.16)
    if k == "release": return click(amp=0.07, pan=0.0, f=2000)
    if k == "result": return chime((293.66, 440.0, 587.33), amp=0.10, d=1.8, tau=0.9)
    if k == "toast": return tick(f=1800, amp=0.04, d=0.05)
    if k == "trace": return pen(max(d, 0.5), 0.012, 0.2)
    if k == "notch": return tick(f=1400 + 90 * i, amp=0.035, pan=0.0, d=0.03, tau=0.005)
    if k == "swap": return click(amp=0.07, pan=0.0, f=2200)
    if k == "dive": return whoosh(max(d, 0.8), 0.08, 200, 2800, pan=0.0, rise=True)
    if k == "eq_on": return chime((146.83, 220.0, 293.66), amp=0.05, d=1.4, tau=0.9)
    if k == "quarter": return tick(f=2600, amp=0.03, pan=0.0, d=0.03, tau=0.006)
    if k == "plate_in": return whoosh(max(d, 0.6), 0.05, 600, 4200, pan=0.25)
    if k == "lift": return thud(f0=110, amp=0.05, d=0.3, pan=-0.2)
    if k == "roll": return roll(max(d, 1.2), 0.07, pan=0.0)
    if k == "lock": return thud(f0=120, amp=0.16, d=0.35)
    if k == "numbers": return tick(f=2000, amp=0.06, d=0.04, tau=0.008)
    if k == "land": return tick(f=1500, amp=0.07, d=0.06, tau=0.012)
    if k == "window_in": return whoosh(max(d, 0.5), 0.045, 300, 3000, rise=True)
    if k == "stamp": return stamp(amp=0.15)
    if k == "scroll": return whoosh(max(d, 0.4), 0.025, 1200, 4800, pan=0.0)
    if k == "open": return click(amp=0.07, f=2200)
    if k == "chips": return tick(f=2300, amp=0.04, d=0.03, tau=0.006)
    if k == "state": return chime((440.0,), amp=0.05, d=1.0, tau=0.35)
    if k == "stream": return stream(max(d, 1.0), 0.04)
    if k == "pen": return pen(max(d, 0.8), 0.018)
    if k == "stamp_nr": return stamp(amp=0.12)
    if k == "morph": return whoosh(max(d, 0.6), 0.05, 400, 3000, rise=False)
    if k == "mark": return chord((146.83, 220.0, 293.66, 369.99), 0.10, 3.0)
    if k == "word": return chime((587.33,), amp=0.05, d=1.4, tau=0.6)
    if k == "line_in": return tick(f=2800, amp=0.04, d=0.03, tau=0.006)
    if k == "tick": return tick(f=2200 + 240 * i, amp=0.05, d=0.04, tau=0.008)
    if k == "brand": return chime((293.66, 440.0), amp=0.06, d=2.0, tau=0.8)
    if k == "cta": return tick(f=1700, amp=0.045, d=0.05, tau=0.01)
    return None


def main():
    results = {}
    for lang in ("en", "ja"):
        global BL, BR
        BL = np.zeros(N); BR = np.zeros(N)
        cues = json.load(open(os.path.join(HERE, f"cues-{lang}.json"), encoding="utf-8"))["cues"]
        dl, dr = drone()
        BL += dl; BR += dr
        pulse()
        counts = {}
        for c in cues:
            s = sound_for(c)
            if s is None:
                continue
            if isinstance(s, tuple) and len(s) == 2 and isinstance(s[0], np.ndarray):
                x, pan = s
            else:
                x, pan = s
            put(x, float(c["t"]), pan=float(pan or 0.0))
            counts[c["k"]] = counts.get(c["k"], 0) + 1
        # the mark: the chord that the picture lands on, with the final D
        results[lang] = {"cues_used": sum(counts.values()), "kinds": len(counts)}
        peak = float(max(np.abs(BL).max(), np.abs(BR).max()))
        g = 0.5 / peak if peak > 0.5 else 1.0                          # keep the bed below -6 dBFS before the mix
        out = np.stack([BL * g, BR * g], axis=1)
        sf.write(os.path.join(HERE, f"bed-{lang}.wav"), out, SR, subtype="PCM_24")
        results[lang].update({"bed_peak_dBFS_before_scaling": round(20 * np.log10(peak), 2), "bed_scale_dB": round(20 * np.log10(g), 2)})
        print(lang, results[lang])
    json.dump(results, open(os.path.join(HERE, "bed-report.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
