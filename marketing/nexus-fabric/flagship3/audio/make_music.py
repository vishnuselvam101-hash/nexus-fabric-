#!/usr/bin/env python3
"""NEXUS FABRIC flagship film 3 - the score and the sound on the picture (the bed; the voice is added by make_mix.py).

Writes  audio/bed.wav           54.000 s, 48 kHz, 24-bit, stereo: score and effects, no voice
        audio/bed-report.json   the arrangement: sections, slams, whooshes and impacts, with their times

Tempo 120 BPM: beat 0.5 s, bar 2.0 s. Every hit sits on the beat grid. The slot boundaries 4, 8, 12, 20, 26, 30,
42, 46 and 50 s are bar lines. Key of D minor, with the final chord in D major.
Arc: a pulse from bar 1; a rising build to a hit at 8.0 s (the product appears); a propulsive groove through the
feature run (12 to 20 s) with an accent on every 2 s where a new part appears, and a tick on the five part slams
at 12, 14, 16, 18 and 19 s (the picture's cues.json did not exist when this was written, so the brief's times are used);
a breakdown to a held Bb chord at 30.0 s (AMOEBA arrives); a second build to a peak at 40 to 42 s; a pull-back at
42.0 s for the honesty line; a rebuild to 50.0 s; a clean final hit and a tail that is fully faded by 54.0 s.
Everything is synthesised here from sines, filtered noise, swept noise and envelopes. No sample library is used.
"""
import json, os
from functools import lru_cache
import numpy as np
import soundfile as sf
from scipy.signal import butter, sosfilt, stft, istft

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 48000
DUR = 54.0
N = int(round(SR * DUR))
BPM = 120
BEAT = 60.0 / BPM                     # 0.5 s
BAR = 4 * BEAT                        # 2.0 s
rng = np.random.default_rng(20261120)
BL = np.zeros(N); BR = np.zeros(N)

# chord voicings (Hz) and bass roots (Hz, one octave below the pad's lowest note)
CH = {"Dm": [146.83, 220.0, 293.66, 349.23], "Gm": [196.0, 293.66, 392.0, 466.16], "Bb": [116.54, 174.61, 233.08, 293.66],
      "C": [130.81, 196.0, 261.63, 329.63], "F": [174.61, 220.0, 261.63, 349.23], "D": [146.83, 220.0, 293.66, 369.99]}
ROOT = {"Dm": 73.42, "Gm": 98.0, "Bb": 58.27, "C": 65.41, "F": 87.31, "D": 73.42}

# sections: start, end, kick, snare, hats, bass, chord, pad level, bass level
#   kick: beat | bar (bar lines only) | bar_soft | beat_soft | end (the final hit only)
#   snare: 0 none | 1 beats 2 and 4 | 2 as 1 with a 16th roll into the section end
#   hats: 0 | 8 (eighths) | 16 (sixteenths);  bass: q (quarters) | 8 | 16 | drone (one long note)
SECTIONS = [
    (0.0, 2.0, "beat", 0, 0, "q", "Dm", 0.55, 1.0),       # bar 1: the pulse starts at 0.0 s
    (2.0, 4.0, "beat", 0, 8, "8", "Dm", 0.60, 1.0),
    (4.0, 6.0, "beat", 1, 8, "8", "Bb", 0.70, 1.0),       # slot 2: the question
    (6.0, 8.0, "beat", 2, 16, "8", "C", 0.80, 1.0),       # build to the hit at 8.0 s
    (8.0, 10.0, "beat", 1, 8, "8", "F", 0.70, 1.0),       # slot 3: the product
    (10.0, 12.0, "beat", 1, 8, "8", "F", 0.70, 1.0),
    (12.0, 14.0, "beat", 1, 16, "16", "Dm", 0.80, 1.1),   # slot 4: five parts, accents on 12, 14, 16, 18
    (14.0, 16.0, "beat", 1, 16, "16", "Bb", 0.80, 1.1),
    (16.0, 18.0, "beat", 1, 16, "16", "F", 0.80, 1.1),
    (18.0, 20.0, "beat", 1, 16, "16", "C", 0.80, 1.1),
    (20.0, 22.0, "beat", 1, 8, "8", "Dm", 0.70, 1.0),     # slot 5: the checks
    (22.0, 24.0, "beat", 1, 8, "8", "Gm", 0.70, 1.0),
    (24.0, 26.0, "beat", 1, 8, "8", "Bb", 0.70, 1.0),
    (26.0, 28.0, "beat", 1, 16, "8", "F", 0.70, 1.0),     # slot 6: the DXF
    (28.0, 30.0, "beat", 2, 16, "8", "C", 0.80, 1.0),     # build into AMOEBA
    (30.0, 36.0, "bar", 0, 0, "drone", "Bb", 0.90, 0.9),  # slot 7: breakdown, held chord from 30.0 s
    (36.0, 38.0, "beat", 1, 8, "8", "Dm", 0.80, 1.0),     # second build
    (38.0, 40.0, "beat", 1, 16, "16", "C", 0.90, 1.1),
    (40.0, 42.0, "beat", 2, 16, "16", "Dm", 1.00, 1.1),   # second peak
    (42.0, 46.0, "bar_soft", 0, 0, "drone", "F", 0.45, 0.6),   # slot 8: pulled back for the honesty line
    (46.0, 48.0, "beat_soft", 1, 8, "8", "Dm", 0.60, 0.9),     # slot 9: rebuild
    (48.0, 50.0, "beat", 2, 16, "8", "Bb", 0.90, 1.0),
    (50.0, 54.0, "end", 0, 0, "drone", "D", 1.00, 1.0),   # slot 10: the final hit and the tail
]
SLAMS = [(12.0, "Dm"), (14.0, "Bb"), (16.0, "F"), (18.0, "C")]   # accent: kick, snare, chord stab, tick
TICKS = [12.0, 14.0, 16.0, 18.0, 19.0]                              # the five part slams
WHOOSH = [(4.0, 1.0, True, 0.12), (12.0, 1.0, True, 0.12), (20.0, 1.0, True, 0.11), (26.0, 1.0, True, 0.11),
          (30.0, 1.0, True, 0.12), (42.0, 1.2, False, 0.11), (46.0, 1.0, True, 0.12)]   # (scene change, length, rising into it or falling from it)
RISERS = [(2.0, 2.0, 0.05), (6.0, 2.0, 0.06), (28.0, 2.0, 0.05), (36.0, 2.0, 0.07), (48.0, 2.0, 0.07)]


def tt(d):
    return np.arange(int(round(d * SR))) / SR


@lru_cache(maxsize=None)
def sos(kind, f1, f2=None):
    if kind == "lp":
        return butter(2, f1, "low", fs=SR, output="sos")
    if kind == "hp":
        return butter(2, f1, "high", fs=SR, output="sos")
    return butter(2, [f1, f2], "band", fs=SR, output="sos")


def lp(x, f): return sosfilt(sos("lp", f), x)
def hp(x, f): return sosfilt(sos("hp", f), x)
def bp(x, lo, hi): return sosfilt(sos("bp", lo, hi), x)


def put(sig, t, pan=0.0, gain=1.0):
    """add a mono signal that starts at time t (seconds); pan -1 (left) .. +1 (right); equal power"""
    i0 = int(round(t * SR)); s = np.asarray(sig, dtype=float)
    if i0 < 0:
        s = s[-i0:]; i0 = 0
    n = min(len(s), N - i0)
    if n <= 0:
        return
    a = np.sqrt((1 - pan) / 2) * gain; b = np.sqrt((1 + pan) / 2) * gain
    BL[i0:i0 + n] += a * s[:n]; BR[i0:i0 + n] += b * s[:n]


def kick(amp=0.85, d=0.30):
    t = tt(d)
    f = 44 + 90 * np.exp(-t / 0.028)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.10)
    click = hp(rng.standard_normal(len(t)), 2500) * np.exp(-t / 0.003)
    return amp * (body + 0.25 * click)


def snare(amp=0.32, d=0.22):
    t = tt(d)
    body = np.sin(2 * np.pi * 185 * t) * np.exp(-t / 0.05)
    noise = bp(rng.standard_normal(len(t)), 1800, 7500) * np.exp(-t / 0.085)
    return amp * (0.45 * body + 0.9 * noise)


def hat(amp=0.09, d=0.05, tau=0.018):
    t = tt(d)
    return amp * hp(rng.standard_normal(len(t)), 7000) * np.exp(-t / tau)


def tick(f=2600, amp=0.10, d=0.035, tau=0.006):
    t = tt(d)
    return amp * np.sin(2 * np.pi * f * t) * np.exp(-t / tau)


def bass(f, dur, amp=0.22, tau=None):
    t = tt(dur + 0.05)
    tau = tau or dur * 0.5
    s = sum(np.sin(2 * np.pi * f * k * t) / k ** 1.4 for k in range(1, 5))
    env = np.minimum(1.0, t / 0.005) * np.exp(-t / tau)
    return amp * lp(s * env, 420) / 1.6


def pad(chord, dur, amp, cutoff=1400, attack=0.25, release=0.4):
    """detuned, harmonically rich chord that holds for dur seconds and releases after it"""
    t = tt(dur + release)
    s = np.zeros(len(t))
    for f in CH[chord]:
        for k in range(1, 7):
            for dt in (-0.0015, 0.0015):
                s += np.sin(2 * np.pi * f * k * (1 + dt) * t) / k
    env = np.minimum(1.0, t / attack) * np.where(t <= dur, 1.0, np.exp(-(t - dur) / (release / 3)))
    return amp * lp(s * env, cutoff) / 9.0


def stab(chord, amp=0.22, d=0.5, tau=0.14, cutoff=3200):
    t = tt(d)
    s = sum(np.sin(2 * np.pi * f * k * t) / k for f in CH[chord] for k in range(1, 6))
    env = np.exp(-t / tau) * np.minimum(1.0, t / 0.004)
    return amp * lp(s * env, cutoff) / 9.0


def sweep(d, f0, f1, amp=0.12, shape=1.6):
    """swept noise: a whoosh whose centre frequency glides from f0 to f1 (log scale) over d seconds"""
    n = int(round(d * SR)); x = rng.standard_normal(n)
    nper, hop = 2048, 512
    f, t, Z = stft(x, SR, window="hann", nperseg=nper, noverlap=nper - hop)
    u = np.clip(t / max(t[-1], 1e-9), 0, 1)
    fc = f0 * (f1 / f0) ** u
    M = np.exp(-0.5 * ((np.log2(np.maximum(f, 1.0))[:, None] - np.log2(fc)[None, :]) / 0.7) ** 2)
    _, y = istft(Z * M, SR, window="hann", nperseg=nper, noverlap=nper - hop)
    y = y[:n] if len(y) >= n else np.pad(y, (0, n - len(y)))
    y = y / (np.abs(y).max() + 1e-12)
    return amp * y * np.sin(np.pi * np.linspace(0, 1, n)) ** shape


def crash(amp=0.10, d=1.8, tau=0.7):
    t = tt(d)
    return amp * hp(rng.standard_normal(len(t)), 3000) * np.exp(-t / tau)


def main():
    events = []
    for t0, t1, kmode, snm, hats, bmode, chord, plev, blev in SECTIONS:
        # pad: the chord of the section (the breakdown holds Bb from 30.0 s to 36.0 s)
        put(pad(chord, t1 - t0, 0.075 * plev), t0, pan=-0.25)
        put(pad(chord, t1 - t0, 0.060 * plev, cutoff=2200, attack=0.35), t0, pan=0.25)
        ds = min(1.25, plev / 0.8)          # drum and bass level follows the section level, so 0 to 8 s rises and 40 to 42 s peaks
        events.append({"t": t0, "section": f"{t0:g}-{t1:g}", "chord": chord, "kick": kmode, "snare": snm, "hats": hats, "bass": bmode, "level": round(ds, 3)})
        b0, b1 = int(round(t0 / BEAT)), int(round(t1 / BEAT))
        for b in range(b0, b1):
            t = b * BEAT; bar_line = (b % 4 == 0)
            # kick
            if kmode == "beat" or (kmode == "beat_soft" and True) or (kmode == "bar" and bar_line) or (kmode == "bar_soft" and bar_line) or (kmode == "end" and abs(t - t0) < 1e-9):
                amp = 0.85 * (0.6 if kmode in ("beat_soft", "bar_soft") else 1.0) * (0.8 if kmode == "bar" and t0 >= 30 and t0 < 36 else 1.0)
                if kmode == "end":
                    amp = 1.0
                put(kick(amp * ds), t, pan=0.0)
            # snare on beats 2 and 4
            if snm and (b % 4) in (1, 3):
                put(snare(0.32 * ds), t, pan=0.1)
            # hats
            if hats == 8:
                for k, off in enumerate((0.0, 0.25)):
                    put(hat((0.075 if k == 0 else 0.10) * ds), t + off, pan=(0.3 if k == 0 else -0.3))
            elif hats == 16:
                for k, off in enumerate((0.0, 0.125, 0.25, 0.375)):
                    put(hat([0.10, 0.045, 0.075, 0.045][k] * ds), t + off, pan=(0.3 if k % 2 == 0 else -0.3))
            # bass
            root = ROOT[chord] * blev * 0.9
            if bmode == "q":
                put(bass(ROOT[chord], BEAT * 0.9, 0.24 * blev * ds), t)
            elif bmode == "8":
                for k, off in enumerate((0.0, 0.25)):
                    put(bass(ROOT[chord] if k == 0 else 2 * ROOT[chord], 0.22, 0.20 * blev * ds), t + off)
            elif bmode == "16":
                for k, off in enumerate((0.0, 0.125, 0.25, 0.375)):
                    put(bass(ROOT[chord] if k in (0, 2) else 2 * ROOT[chord], 0.11, 0.17 * blev * ds), t + off)
        if snm == 2:                                           # 16th snare roll into the end of the section
            for k in range(8):
                put(snare(0.16 + 0.02 * k), t1 - 0.5 + k * 0.0625, pan=-0.1)
        if bmode == "drone":
            put(bass(ROOT[chord], t1 - t0, 0.20 * blev, tau=(t1 - t0) * 0.8), t0)

    for t, chord in SLAMS:                                     # accent on each new part
        put(kick(1.0), t); put(snare(0.5), t, pan=0.1); put(stab(chord, 0.16), t); put(tick(2600, 0.10), t, pan=-0.15)
        events.append({"t": t, "accent": chord})
    for t in TICKS:                                            # the five part slams
        put(tick(2600 + (int(t) % 3) * 240, 0.10), t, pan=(0.15 if t % 2 == 0 else -0.15))
    put(tick(3000, 0.09), 19.0, pan=0.2)
    events.append({"t": 19.0, "tick": "fifth part"})

    put(kick(1.0), 8.0); put(crash(0.10, 1.6), 8.0, pan=0.2); put(stab("F", 0.22, d=1.0, tau=0.35), 8.0)   # the product
    events.append({"t": 8.0, "hit": "product appears"})
    put(stab("Bb", 0.24, d=1.5, tau=0.6), 30.0); put(bass(ROOT["Bb"], 2.0, 0.35), 30.0); put(kick(0.8), 30.0)  # AMOEBA
    events.append({"t": 30.0, "hit": "AMOEBA, held Bb chord"})

    for t, d, rise, amp in WHOOSH:                             # whooshes on the scene changes
        f0, f1 = (300.0, 3200.0) if rise else (3200.0, 300.0)
        put(sweep(d, f0, f1, amp), t - (d if rise else 0.0), pan=(-0.3 if rise else 0.3))
        events.append({"t": t, "whoosh": "rising into the cut" if rise else "pulls back"})
    for t, d, amp in RISERS:                                   # restrained risers inside the build sections
        put(sweep(d, 250.0, 2600.0, amp), t, pan=0.0)

    # the final hit at 50.0 s and a tail that is fully faded by 54.0 s
    put(kick(1.0), 50.0); put(crash(0.16, 2.6, 1.0), 50.0, pan=-0.2)
    put(stab("D", 0.30, d=3.0, tau=1.0, cutoff=4200), 50.0)
    put(pad("D", 3.5, 0.10, cutoff=2400, attack=0.01, release=0.6), 50.0, pan=0.0)
    put(bass(ROOT["D"], 3.0, 0.30, tau=1.2), 50.0)
    events.append({"t": 50.0, "hit": "final hit and tail"})

    # end: a short cosine fade from 53.0 s to 54.0 s, so the file ends silent with no cut-off
    t = np.arange(N) / SR
    fade = np.clip((54.0 - t) / 1.0, 0.0, 1.0)
    fade = 0.5 - 0.5 * np.cos(np.pi * fade)
    BL[:] *= fade; BR[:] *= fade

    peak = float(max(np.abs(BL).max(), np.abs(BR).max()))
    g = 0.5 / peak if peak > 0.5 else 1.0                        # keep the bed below -6 dBFS before the mix
    out = np.stack([BL * g, BR * g], axis=1)
    sf.write(os.path.join(HERE, "bed.wav"), out, SR, subtype="PCM_24")
    report = {"bpm": BPM, "beat_s": BEAT, "bar_s": BAR, "duration_s": round(N / SR, 6), "frames": N, "sample_rate": SR,
              "peak_dBFS_before_scaling": round(20 * np.log10(peak), 2), "scale_dB": round(20 * np.log10(g), 2),
              "tail_silent_from_s": 54.0, "sections": [list(s[:2]) + [s[2], s[3], s[4], s[5], s[6]] for s in SECTIONS],
              "slams_s": [t for t, _ in SLAMS], "ticks_s": TICKS, "whooshes": [[t, d, "rise" if r else "fall"] for t, d, r, _ in WHOOSH],
              "events": events}
    json.dump(report, open(os.path.join(HERE, "bed-report.json"), "w"), indent=1)
    print(f"bed.wav written: {N / SR:.3f} s, peak {20 * np.log10(peak):.2f} dBFS before scaling ({20 * np.log10(g):.2f} dB)")


if __name__ == "__main__":
    main()
