#!/usr/bin/env python3
"""NEXUS FABRIC — launch film soundtrack synthesiser.

70.8 s · 100 BPM (beat = 0.6 s) · D Aeolian resolving to D major add9.
All instruments and SFX are synthesised in code (storyboard §4 + scene sound column).

Outputs
    audio/soundtrack.wav   48 kHz / 24-bit / stereo, -14 LUFS integrated, <= -1.0 dBTP
    audio/cues.json        every scheduled cue with its time

Dependencies: numpy, scipy, soundfile.  Deterministic: fixed seed, fixed call order.
"""

from __future__ import annotations

import itertools
import json
import math
import os
from functools import lru_cache

import numpy as np
import soundfile as sf
from scipy import signal
from scipy.ndimage import minimum_filter1d

SR = 48000
DUR = 70.8
N = int(round(DUR * SR))
BEAT = 0.6
SEED = 20240607
TARGET_LUFS = -14.0
CEILING_DBTP = -1.0
SILENCE = (33.6, 34.2)
TAIL = 69.6
OUT_DIR = os.path.dirname(os.path.abspath(__file__))

_keys = itertools.count(1)
_prng = np.random.default_rng(SEED + 991)


# ---------------------------------------------------------------- utilities

def si(t: float) -> int:
    return int(round(t * SR))


def db(x: float) -> float:
    return 10.0 ** (x / 20.0)


def nf(midi: int) -> float:
    return 440.0 * 2.0 ** ((midi - 69) / 12.0)


def _t(n: int) -> np.ndarray:
    return np.arange(n, dtype=np.float64) / SR


def _noise(n: int) -> np.ndarray:
    return np.random.default_rng([SEED, next(_keys)]).standard_normal(n)


def smooth(u: np.ndarray) -> np.ndarray:
    u = np.clip(u, 0.0, 1.0)
    return u * u * (3.0 - 2.0 * u)


def peak_norm(x: np.ndarray, p: float = 1.0) -> np.ndarray:
    m = float(np.max(np.abs(x))) if x.size else 0.0
    return x * (p / m) if m > 1e-12 else x


def edges(x: np.ndarray, fin: float = 0.003, fout: float = 0.02) -> np.ndarray:
    n = len(x)
    a, b = min(si(fin), n), min(si(fout), n)
    if a > 1:
        x[:a] *= np.linspace(0.0, 1.0, a)
    if b > 1:
        x[-b:] *= np.linspace(1.0, 0.0, b)
    return x


def lp(x, fc, order=2):
    return signal.sosfilt(signal.butter(order, fc / (SR / 2), "low", output="sos"), x)


def hp(x, fc, order=2):
    return signal.sosfilt(signal.butter(order, fc / (SR / 2), "high", output="sos"), x)


def bp(x, lo, hi, order=2):
    sos = signal.butter(order, [lo / (SR / 2), hi / (SR / 2)], "band", output="sos")
    return signal.sosfilt(sos, x)


def tv_lp(x: np.ndarray, cuts: np.ndarray, q: float = 0.707, blk: int = 128) -> np.ndarray:
    """Time-varying 2-pole lowpass (RBJ), cutoff updated every `blk` samples."""
    y = np.empty_like(x)
    zi = np.zeros(2)
    nb = int(math.ceil(len(x) / blk))
    for i in range(nb):
        s, e = i * blk, min((i + 1) * blk, len(x))
        fc = float(np.clip(cuts[min(i, len(cuts) - 1)], 30.0, 0.45 * SR))
        w0 = 2.0 * math.pi * fc / SR
        cw, sw = math.cos(w0), math.sin(w0)
        al = sw / (2.0 * q)
        a0 = 1.0 + al
        b = np.array([(1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0]) / a0
        a = np.array([1.0, -2.0 * cw / a0, (1.0 - al) / a0])
        y[s:e], zi = signal.lfilter(b, a, x[s:e], zi=zi)
    return y


# ---------------------------------------------------------------- notes

D1, A2, D2, F2, C3 = nf(26), nf(45), nf(38), nf(41), nf(48)
D3, E3, F3, FS3, G3, A3 = nf(50), nf(52), nf(53), nf(54), nf(55), nf(57)
BB2, BB3, C4, D4, E4 = nf(46), nf(58), nf(60), nf(62), nf(64)

DM = [D2, D3, F3, A3]
BB = [BB2, D3, F3, BB3]
FM = [F2, F3, A3, C4]
CM = [C3, E3, G3, C4]
DADD9 = [D2, D3, FS3, A3, E4]
PULSE = [D3, A3, F3, A3, D3, A3, C4, A3]


# ---------------------------------------------------------------- instruments

TAB = 8192


@lru_cache(maxsize=128)
def _saw_table(k: int) -> np.ndarray:
    ph = np.arange(TAB, dtype=np.float64) / TAB
    tab = np.zeros(TAB)
    for h in range(1, k + 1):
        tab += np.sin(2.0 * np.pi * h * ph) / h
    return tab / np.max(np.abs(tab))


def saw(freq: float, n: int, detune: float = 0.0, phase: float = 0.0) -> np.ndarray:
    k = int(np.clip(12000.0 / freq, 4, 96))
    tab = _saw_table(k)
    idx = ((phase + freq * (1.0 + detune) * _t(n)) % 1.0) * TAB
    i0 = idx.astype(np.int64)
    frac = idx - i0
    return tab[i0] * (1.0 - frac) + tab[(i0 + 1) % TAB] * frac


def piano(freq: float, dur: float = 3.5) -> np.ndarray:
    """2-op FM felt piano."""
    n = si(dur)
    t = _t(n)
    tau = 1.7 * (146.83 / freq) ** 0.4
    mod = 1.05 * np.exp(-t / 0.05) * np.sin(2.0 * np.pi * freq * 2.0 * t)
    x = np.sin(2.0 * np.pi * freq * t + mod)
    x += 0.22 * np.sin(2.0 * np.pi * freq * 2.001 * t + 0.5 * mod) * np.exp(-t / 0.4)
    x *= (1.0 - np.exp(-t / 0.005)) * np.exp(-t / tau)
    return edges(peak_norm(lp(x, 3200.0)), 0.001, 0.08)


def sub(freq: float, dur: float, swell: float = 1.0, release: float = 1.0) -> np.ndarray:
    n = si(dur)
    t = _t(n)
    x = np.sin(2.0 * np.pi * freq * t) + 0.05 * np.sin(4.0 * np.pi * freq * t)
    return peak_norm(x * smooth(t / swell) * smooth((dur - t) / release))


def pluck(freq: float, dur: float = 0.6, tau: float = 0.09, tri: float = 0.0) -> np.ndarray:
    n = si(dur)
    t = _t(n)
    s = np.sin(2.0 * np.pi * freq * t)
    if tri > 0.0:
        s = (1.0 - tri) * s + tri * signal.sawtooth(2.0 * np.pi * freq * t, width=0.5)
    x = s * (1.0 - np.exp(-t / 0.002)) * np.exp(-t / tau)
    return edges(peak_norm(lp(x, 5000.0)), 0.001, 0.03)


def pad(freqs, length: float, f0: float, f1: float,
        tail: float = 1.6, attack: float = 0.9) -> np.ndarray:
    """Filtered saw pad, cutoff automation, gentle stereo width (detune mix)."""
    n = si(length + tail)
    left = np.zeros(n)
    right = np.zeros(n)
    for f in freqs:
        a = saw(f, n, -0.0016, float(_prng.random()))
        b = saw(f, n, +0.0016, float(_prng.random()))
        left += a + 0.55 * b
        right += 0.55 * a + b
    cuts = np.geomspace(max(f0, 40.0), max(f1, 40.0), int(math.ceil(n / 128)))
    left = tv_lp(left, cuts)
    right = tv_lp(right, cuts)
    t = _t(n)
    env = smooth(t / attack) * smooth((length + tail - t) / tail)
    return peak_norm(np.stack([left * env, right * env], axis=1))


def kick() -> np.ndarray:
    n = si(0.45)
    t = _t(n)
    f = 44.0 + 60.0 * np.exp(-t / 0.02)
    x = np.sin(2.0 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.14)
    x += 0.10 * _noise(n) * np.exp(-t / 0.003)
    x = lp(x, 2200.0) * smooth(t / 0.004)
    return edges(peak_norm(x), 0.001, 0.03)


def snare() -> np.ndarray:
    n = si(0.32)
    t = _t(n)
    x = bp(_noise(n), 900.0, 3600.0) * np.exp(-t / 0.075)
    x += 0.35 * np.sin(2.0 * np.pi * 190.0 * t) * np.exp(-t / 0.05)
    x *= smooth(t / 0.002)
    return edges(peak_norm(x), 0.001, 0.03)


def hat(tau: float = 0.028) -> np.ndarray:
    n = si(0.10)
    t = _t(n)
    x = lp(hp(_noise(n), 5200.0), 11000.0)
    x *= np.exp(-t / tau) * smooth(t / 0.001)
    return edges(peak_norm(x), 0.0005, 0.01)


# ---------------------------------------------------------------- SFX (<= 4 kHz)

def sfx_ui_tick() -> np.ndarray:
    n = si(0.05)
    t = _t(n)
    x = np.sin(2.0 * np.pi * 1200.0 * t) * np.exp(-t / 0.004)
    x += 0.5 * _noise(n) * np.exp(-t / 0.0012)
    return edges(peak_norm(lp(x, 4000.0)), 0.0005, 0.005)


def sfx_gate_thunk() -> np.ndarray:
    n = si(0.20)
    t = _t(n)
    x = np.sin(2.0 * np.pi * 60.0 * t) * np.exp(-t / 0.030)
    x += 0.18 * lp(_noise(n), 1800.0) * np.exp(-t / 0.002)
    x = lp(x, 4000.0) * smooth(t / 0.002)
    return edges(peak_norm(x), 0.0005, 0.02)


def sfx_refuse_damp() -> np.ndarray:
    n = si(0.35)
    t = _t(n)
    x = lp(_noise(n), 220.0, order=4) * np.exp(-t / 0.06)
    x += 0.4 * np.sin(2.0 * np.pi * 70.0 * t) * np.exp(-t / 0.05)
    x = lp(x, 4000.0) * smooth(t / 0.003)
    return edges(peak_norm(x), 0.001, 0.03)


def sfx_hold_latch() -> np.ndarray:
    """Descending minor third, 0.3 s. Held in reserve: S07 now uses the soft
    dimension tick at 38.4 (the hold / withheld-DXF idea was removed)."""
    n = si(0.30)
    t = _t(n)
    f = np.where(t < 0.15, nf(62), nf(59))
    x = np.sin(2.0 * np.pi * np.cumsum(f) / SR) * np.exp(-(t % 0.15) / 0.06)
    x = lp(x, 4000.0) * smooth(t / 0.003)
    return edges(peak_norm(x), 0.001, 0.03)


def sfx_dimension_tick() -> np.ndarray:
    n = si(0.04)
    t = _t(n)
    x = np.sin(2.0 * np.pi * 1600.0 * t) * np.exp(-t / 0.003)
    x += 0.25 * _noise(n) * np.exp(-t / 0.001)
    return edges(peak_norm(lp(x, 4000.0)), 0.0005, 0.004)


def sfx_paper_drop() -> np.ndarray:
    n = si(0.20)
    t = _t(n)
    x = bp(_noise(n), 400.0, 2600.0) * np.exp(-t / 0.035)
    x = lp(x, 4000.0) * smooth(t / 0.004)
    return edges(peak_norm(x), 0.001, 0.02)


def sfx_stamp_soft() -> np.ndarray:
    n = si(0.25)
    t = _t(n)
    x = np.sin(2.0 * np.pi * 180.0 * t) * np.exp(-t / 0.045)
    x += 0.35 * lp(_noise(n), 2200.0) * np.exp(-t / 0.012)
    x = lp(x, 4000.0) * smooth(t / 0.002)
    return edges(peak_norm(x), 0.001, 0.02)


def sfx_reveal_air(dur: float) -> np.ndarray:
    n = si(dur)
    t = _t(n)
    x = lp(_noise(n), 3200.0) * (t / dur) ** 2.5
    return edges(peak_norm(x), 0.05, 0.02)


def pink(n: int) -> np.ndarray:
    w = np.fft.rfft(_noise(n))
    f = np.fft.rfftfreq(n, 1.0 / SR)
    w[1:] /= np.sqrt(f[1:])
    w[0] = 0.0
    x = np.fft.irfft(w, n)
    return x / (np.std(x) + 1e-12)


# ---------------------------------------------------------------- mix bus

class Mix:
    def __init__(self):
        self.bus = {"music": np.zeros((N, 2)), "sfx": np.zeros((N, 2)), "bed": np.zeros((N, 2))}
        self.cues = []
        self.scene = "S01"

    def add(self, bus, sig, t, gain=1.0, pan=0.0, name="", note=""):
        x = np.asarray(sig, dtype=np.float64)
        if x.ndim == 1:
            gl = gain * math.cos((pan + 1.0) * math.pi / 4.0)
            gr = gain * math.sin((pan + 1.0) * math.pi / 4.0)
            x = np.stack([x * gl, x * gr], axis=1)
        else:
            x = x * gain
        i = si(t)
        j = min(N, i + len(x))
        if j > i:
            self.bus[bus][i:j] += x[:j - i]
        self.cues.append({
            "scene": self.scene,
            "name": name,
            "bus": bus,
            "start": round(t, 4),
            "end": round(t + len(x) / SR, 4),
            "gain_db": round(20.0 * math.log10(max(gain, 1e-9)), 2),
            "pan": round(pan, 3),
            "note": note,
        })


def in_silence(t: float) -> bool:
    return SILENCE[0] - 1e-9 <= t < SILENCE[1] - 1e-9


def pulse_grid(m: Mix, t0: float, t1: float, level: float):
    for k in range(int(round((t1 - t0) / 0.3))):
        t = round(t0 + 0.3 * k, 4)
        if in_silence(t):
            continue
        f = PULSE[k % len(PULSE)]
        acc = (k % 4 == 0)
        m.add("music", pluck(f, 0.45, tau=0.16, tri=0.45), t,
              level * (1.35 if acc else 1.0), name="pulse",
              note=f"8th note, {f:.2f} Hz, triangle/sine")


def hat_grid(m: Mix, t0: float, t1: float):
    for k in range(int(round((t1 - t0) / 0.15))):
        t = round(t0 + 0.15 * k, 4)
        if in_silence(t):
            continue
        acc = (k % 4 == 0)
        m.add("music", hat(0.030 if acc else 0.022), t,
              db(-25.0 if acc else -30.0), pan=(0.28 if k % 2 else -0.28),
              name="hat", note="16th, filtered noise")


# ---------------------------------------------------------------- schedule

def render() -> Mix:
    m = Mix()

    # bed: room-air, pink noise, -42 dB, ducked 6 dB under music from 11.4
    m.scene = "bed"
    n_air = si(TAIL)
    air = pink(n_air)
    duck = np.full(n_air, db(-6.0))
    d0, d1 = si(11.0), si(12.0)
    duck[:d0] = 1.0
    duck[d0:d1] = np.linspace(1.0, db(-6.0), d1 - d0)
    m.add("bed", air * duck, 0.0, db(-42.0), name="room-air",
          note="pink noise bed, -42 dBFS RMS, ducked 6 dB from 11.4")

    # S01 · 0.0–6.0
    m.scene = "S01"
    m.add("music", sub(D1, 11.4, swell=5.0, release=1.2), 0.0, db(-30.0),
          name="sub-swell", note="D1 sine swell")
    m.add("music", piano(D3, 4.2), 1.2, db(-13.0), name="felt-piano", note="D3")
    m.add("music", piano(A2, 4.6), 3.6, db(-14.0), name="felt-piano", note="A2")

    # S02 · 6.0–11.4
    m.scene = "S02"
    for t, f in zip([6.6, 6.9, 7.2, 7.5, 7.8], [D3, F3, G3, A3, C4]):
        m.add("music", pluck(f, 0.5, tau=0.09, tri=0.0), t, db(-16.0),
              name="pluck", note=f"sine, 90 ms decay, {f:.2f} Hz")
    m.add("music", pad(DM, 9.0, 300.0, 420.0, attack=1.4), 8.4, db(-21.0),
          name="pad", note="Dm, cutoff 300→420 Hz")

    # S03 · 11.4–17.4
    m.scene = "S03"
    pulse_grid(m, 11.4, 33.6, db(-22.0))
    for k in range(9):
        m.add("sfx", sfx_ui_tick(), round(12.6 + 0.15 * k, 4), db(-23.0),
              name="ui-tick", note="dimension field 1..9")

    # S04 · 17.4–23.4
    m.scene = "S04"
    for k in range(14):
        t = round(17.4 + 1.2 * k, 4)
        if t >= 33.6:
            break
        m.add("music", kick(), t, db(-15.0), name="kick", note="beats 1 & 3")
    m.add("music", pad(DM, 6.0, 400.0, 900.0, attack=1.2), 17.4, db(-16.0),
          name="pad", note="Dm, cutoff 400→900 Hz")
    for t in (18.6, 19.2, 19.8):
        m.add("sfx", sfx_ui_tick(), t, db(-27.0), name="ui-tick", note="column label")

    # S05 · 23.4–28.8
    m.scene = "S05"
    hat_grid(m, 23.4, 33.6)
    m.add("music", pad(DM, 10.2, 400.0, 4000.0, attack=1.0), 23.4, db(-14.0),
          name="pad", note="Dm, cutoff 400→4000 Hz")
    for k in range(13):
        m.add("sfx", sfx_gate_thunk(), round(24.6 + 0.15 * k, 4), db(-19.5),
              name="gate-thunk", note=f"gate {k + 1}/13")

    # S06 · 28.8–34.2
    m.scene = "S06"
    m.add("sfx", sfx_refuse_damp(), 29.4, db(-19.0), name="refuse-damp",
          note="low-passed noise thud, dry")
    m.add("sfx", sfx_reveal_air(1.2), 32.4, db(-19.5), name="reveal-air",
          note="reverse swell 32.4→33.6, cut for the silent beat")

    # S07 · 34.2–41.4 (reveal)
    m.scene = "S07"
    for t, chord, c0, c1, nm in (
        (34.2, DM, 2400.0, 3000.0, "Dm"),
        (36.0, BB, 2600.0, 3200.0, "Bb"),
        (37.8, FM, 2800.0, 3400.0, "F"),
        (39.6, CM, 3000.0, 2600.0, "C"),
    ):
        m.add("music", pad(chord, 1.8, c0, c1, attack=0.25), t, db(-12.0),
              name="pad", note=f"reveal {nm}")
    m.add("music", sub(D1, 7.2, swell=0.2, release=0.8), 34.2, db(-19.0),
          name="sub", note="D1 reveal")
    for t in (34.2, 36.6, 39.0):
        m.add("music", kick(), t, db(-13.5), name="kick", note="half-time")
    for t in (35.4, 37.8, 40.2):
        m.add("music", snare(), t, db(-19.0), name="snare", note="half-time")
    m.add("sfx", sfx_dimension_tick(), 38.4, db(-25.0), name="dimension-tick",
          note="942.478 mm dimension line (replaces hold-latch)")
    pulse_grid(m, 34.2, 57.0, db(-23.0))
    hat_grid(m, 34.2, 46.8)

    # S08 · 41.4–46.8
    m.scene = "S08"
    m.add("music", pad(DM, 5.4, 1800.0, 1500.0, attack=0.8), 41.4, db(-15.0),
          name="pad", note="Dm sustain, groove stripped to pulse + pad")
    m.add("music", piano(D3, 3.6), 41.4, db(-15.0), name="felt-piano", note="scene change D3")
    m.add("sfx", sfx_paper_drop(), 42.0, db(-20.0), name="paper-drop",
          note="120 ms band-passed noise")

    # S09 · 46.8–52.2
    m.scene = "S09"
    m.add("music", pad(BB, 5.4, 1600.0, 1400.0, attack=0.9), 46.8, db(-15.0),
          name="pad", note="Bb sustain, hats out")
    m.add("music", piano(F3, 3.6), 46.8, db(-15.0), name="felt-piano", note="scene change F3")
    m.add("sfx", sfx_stamp_soft(), 47.4, db(-19.5), name="stamp-soft",
          note="180 Hz body + short noise")

    # S10 · 52.2–57.0
    m.scene = "S10"
    m.add("music", pad(FM, 4.8, 1400.0, 1200.0, attack=1.0), 52.2, db(-18.0),
          name="pad", note="F, pad thins")
    m.add("music", piano(A3, 3.6), 53.4, db(-15.0), name="felt-piano", note="single note A3")

    # S11 · 57.0–62.4
    m.scene = "S11"
    m.add("music", pad(CM, 5.4, 1500.0, 1800.0, attack=1.0), 57.0, db(-17.0),
          name="pad", note="C, pulse out, piano + pad only")
    m.add("music", piano(G3, 3.6), 57.0, db(-15.0), name="felt-piano", note="scene change G3")
    for t, f in ((57.3, A3), (57.6, D4)):
        m.add("music", pluck(f, 0.5, tau=0.09, tri=0.0), t, db(-18.0),
              name="pluck", note=f"phone entry, {f:.2f} Hz")

    # S12 · 62.4–70.8
    m.scene = "S12"
    m.add("music", pad(DADD9, 5.6, 2200.0, 1100.0, tail=1.6, attack=0.8), 62.4, db(-13.0),
          name="pad", note="D major add9 (D–F#–A–E), releases to 69.6")
    m.add("music", piano(D3, 5.6), 63.6, db(-13.5), name="felt-piano", note="D3")
    m.add("music", sub(D1, 7.2, swell=0.6, release=6.4), 62.4, db(-21.0),
          name="sub", note="D1 decays to silence at 69.6")
    return m


# ---------------------------------------------------------------- master chain

def mono_low(x: np.ndarray, fc: float = 120.0) -> np.ndarray:
    sos = signal.butter(2, fc / (SR / 2), "low", output="sos")
    lo = signal.sosfilt(sos, x, axis=0)
    return x - lo + lo.mean(axis=1, keepdims=True)


def master_env() -> np.ndarray:
    e = np.ones(N)
    a, b = si(SILENCE[0]), si(SILENCE[1])
    r_in, r_out = si(0.02), si(0.005)
    e[a:b] = 0.0
    e[a - r_in:a] *= np.linspace(1.0, 0.0, r_in)
    e[b:b + r_out] *= np.linspace(0.0, 1.0, r_out)
    t0 = si(TAIL)
    f = si(0.05)
    e[t0:] = 0.0
    e[t0 - f:t0] *= np.linspace(1.0, 0.0, f)
    fi = si(0.05)
    e[:fi] *= np.linspace(0.0, 1.0, fi)
    return e


def k_weight(x: np.ndarray) -> np.ndarray:
    f0, g, q = 1681.974450955533, 3.99984385397334, 0.7071752369554196
    k = math.tan(math.pi * f0 / SR)
    vh = 10.0 ** (g / 20.0)
    vb = vh ** 0.4996667741545416
    a0 = 1.0 + k / q + k * k
    b = np.array([(vh + vb * k / q + k * k) / a0,
                  2.0 * (k * k - vh) / a0,
                  (vh - vb * k / q + k * k) / a0])
    a = np.array([1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0])
    y = signal.lfilter(b, a, x, axis=0)
    f0, q = 38.13547087602444, 0.5003270373238773
    k = math.tan(math.pi * f0 / SR)
    den = 1.0 + k / q + k * k
    b = np.array([1.0, -2.0, 1.0]) / den
    a = np.array([1.0, 2.0 * (k * k - 1.0) / den, (1.0 - k / q + k * k) / den])
    return signal.lfilter(b, a, y, axis=0)


def integrated_lufs(x: np.ndarray) -> float:
    y = k_weight(x)
    blk, step = si(0.4), si(0.1)
    if len(y) < blk:
        return -math.inf
    nb = 1 + (len(y) - blk) // step
    idx = np.arange(nb) * step
    cs = np.cumsum(np.vstack([np.zeros((1, y.shape[1])), y * y]), axis=0)
    z = (cs[idx + blk] - cs[idx]).sum(axis=1) / blk
    loud = -0.691 + 10.0 * np.log10(np.maximum(z, 1e-30))
    keep = loud > -70.0
    if not np.any(keep):
        return -math.inf
    rel = -0.691 + 10.0 * math.log10(float(np.mean(z[keep]))) - 10.0
    gated = keep & (loud > rel)
    if not np.any(gated):
        gated = keep
    return float(-0.691 + 10.0 * math.log10(float(np.mean(z[gated]))))


def _oversampled_peak(x: np.ndarray) -> np.ndarray:
    """Per-base-sample peak across channels, 4x oversampled."""
    pk = np.zeros(len(x))
    for c in range(x.shape[1]):
        up = signal.resample_poly(x[:, c], 4, 1)
        pk = np.maximum(pk, np.max(np.abs(up.reshape(-1, 4)), axis=1))
        del up
    return pk


def true_peak_db(x: np.ndarray) -> float:
    return 20.0 * math.log10(max(float(np.max(_oversampled_peak(x))), 1e-12))


def limit(x: np.ndarray, ceiling_db: float = CEILING_DBTP,
          lookahead: float = 0.002, release: float = 0.08) -> np.ndarray:
    ceil_lin = db(ceiling_db)
    la = max(1, si(lookahead))
    pk = _oversampled_peak(x)
    g = np.minimum(1.0, ceil_lin / np.maximum(pk, 1e-12))
    g = minimum_filter1d(g, size=2 * la + 1, mode="nearest")
    a = math.exp(-1.0 / (SR * release))
    gs = signal.lfilter([1.0 - a], [1.0, -a], g, zi=[a * g[0]])[0]
    return x * np.minimum(gs, g)[:, None]


def master(x: np.ndarray):
    gain = 1.0
    y = x
    lufs, tp = -math.inf, 0.0
    for _ in range(8):
        y = limit(x * gain)
        tp = true_peak_db(y)
        if tp > ceiling_fix(tp):
            y *= db(CEILING_DBTP - tp)
        lufs = integrated_lufs(y)
        tp = true_peak_db(y)
        err = TARGET_LUFS - lufs
        if abs(err) <= 0.05 and tp <= CEILING_DBTP:
            break
        gain *= db(err)
    return y, lufs, tp


def ceiling_fix(tp: float) -> float:
    return CEILING_DBTP


# ---------------------------------------------------------------- output

SECTIONS = [
    ("Intro", 0.0, 11.4, "sub D1 swell, felt piano, room-air; no percussion"),
    ("Build A", 11.4, 23.4, "8th pulse from 11.4; kick beats 1 & 3 from 17.4; pad Dm"),
    ("Build B", 23.4, 33.6, "16th hats; pad cutoff 400→4000 Hz; riser 32.4→33.6"),
    ("Silence", 33.6, 34.2, "everything out"),
    ("Reveal", 34.2, 41.4, "pad Dm–Bb–F–C, half-time kick/snare, sub D1"),
    ("Sustain", 41.4, 57.0, "pulse + pad; hats out 46.8; piano per scene change"),
    ("Resolve", 57.0, 62.4, "pulse out; piano + pad"),
    ("End", 62.4, 69.6, "D major add9 62.4; piano D3 63.6; sub decay to 69.6"),
    ("Black tail", 69.6, 70.8, "silence"),
]


def main():
    mix = render()
    x = mix.bus["music"] + mix.bus["sfx"] + mix.bus["bed"]
    mix.bus.clear()
    x = mono_low(x, 120.0)
    x *= master_env()[:, None]

    y, lufs, tp = master(x)
    peak = float(np.max(np.abs(y)))
    if peak >= 1.0:
        y *= 0.999 / peak
        lufs, tp = integrated_lufs(y), true_peak_db(y)

    wav = os.path.join(OUT_DIR, "soundtrack.wav")
    sf.write(wav, y, SR, subtype="PCM_24")

    cues = sorted(mix.cues, key=lambda c: (c["start"], c["bus"], c["name"]))
    doc = {
        "meta": {
            "title": "NEXUS FABRIC — launch film soundtrack",
            "sample_rate": SR,
            "channels": 2,
            "bit_depth": 24,
            "duration_s": DUR,
            "frames_60fps": 4248,
            "bpm": 100,
            "beat_s": BEAT,
            "key": "D Aeolian → D major add9",
            "seed": SEED,
            "target_lufs": TARGET_LUFS,
            "ceiling_dbtp": CEILING_DBTP,
            "measured_lufs": round(lufs, 2),
            "measured_dbtp": round(tp, 2),
            "low_end_mono_below_hz": 120.0,
            "stereo_width": "pad and hats only",
        },
        "sections": [
            {"name": n, "start": a, "end": b, "note": d} for n, a, b, d in SECTIONS
        ],
        "windows": [
            {"name": "fade-in", "start": 0.0, "end": 0.05},
            {"name": "full-silence", "start": SILENCE[0], "end": SILENCE[1]},
            {"name": "black-tail-silence", "start": TAIL, "end": DUR},
        ],
        "reserved": [
            {"name": "hold-latch", "note": "descending minor third 0.3 s; superseded by dimension-tick at 38.4"},
        ],
        "cues": cues,
    }
    with open(os.path.join(OUT_DIR, "cues.json"), "w", encoding="utf-8") as fh:
        json.dump(doc, fh, ensure_ascii=False, indent=2)
        fh.write("\n")

    print(f"wrote {wav}  ({DUR:.1f} s, {SR} Hz, 24-bit, stereo)")
    print(f"cues  {len(cues)}")
    print(f"LUFS  {lufs:.2f} (integrated, BS.1770-4 gated)")
    print(f"peak  {tp:.2f} dBTP (4x oversampled)")


if __name__ == "__main__":
    main()
