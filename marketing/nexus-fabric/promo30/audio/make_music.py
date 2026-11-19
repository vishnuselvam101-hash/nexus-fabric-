#!/usr/bin/env python3
"""NEXUS FABRIC launch30 - original score + sound design, synthesised entirely in code.

No samples, no recordings, no reference track: every sound below is generated from
oscillators and seeded noise with numpy/scipy. Original composition written for this film
(D minor tension through scenes 1-4, dominant build, D major add9 resolution at 25 s).

Outputs: music.wav, sfx.wav (48 kHz stereo, 30.000 s), sfx-cues-used.json
"""
import json, os, re
import numpy as np
import soundfile as sf
from scipy.signal import butter, sosfilt, sosfiltfilt, fftconvolve

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 48000
DUR = 30.0
N = int(SR * DUR)
T = np.arange(N) / SR
rng = np.random.default_rng(20261119)


def hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def lp(x, fc, order=4):
    return sosfilt(butter(order, fc, "low", fs=SR, output="sos"), x, axis=0)


def hp(x, fc, order=2):
    return sosfilt(butter(order, fc, "high", fs=SR, output="sos"), x, axis=0)


def bp(x, lo, hi, order=2):
    return sosfilt(butter(order, [lo, hi], "band", fs=SR, output="sos"), x, axis=0)


def smooth(t, a, b):
    """0 before a, 1 after b, raised-cosine in between."""
    x = np.clip((t - a) / max(b - a, 1e-9), 0, 1)
    return 0.5 - 0.5 * np.cos(np.pi * x)


def pan(x, p):
    """p in [-1,1] -> equal-power stereo."""
    th = (p + 1) * np.pi / 4
    return np.stack([x * np.cos(th), x * np.sin(th)], axis=1)


def reverb_ir(seconds=2.4, bright=5000.0):
    n = int(seconds * SR)
    t = np.arange(n) / SR
    ir = rng.standard_normal((n, 2)) * np.exp(-t / (seconds / 6.0))[:, None]
    ir = lp(ir, bright, 2)
    ir[: int(0.012 * SR)] *= np.linspace(0, 1, int(0.012 * SR))[:, None]
    return ir / np.sqrt((ir ** 2).sum(axis=0)).max()


def reverb(x, wet, seconds=2.4, bright=5000.0):
    ir = reverb_ir(seconds, bright)
    w = np.stack([fftconvolve(x[:, c], ir[:, c])[:N] for c in range(2)], axis=1)
    return x + wet * w


def tilt_mid(x, lo=1000.0, hi=4000.0, depth_db=-5.0):
    """Broad linear-phase dip so the bed leaves 1-4 kHz to the voice."""
    X = np.fft.rfft(x, axis=0)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    lf = np.log2(np.maximum(f, 1.0))
    c = (np.log2(lo) + np.log2(hi)) / 2
    width = (np.log2(hi) - np.log2(lo)) / 2 + 0.5
    g = depth_db * np.clip(1 - ((lf - c) / width) ** 2, 0, 1)
    return np.fft.irfft(X * (10 ** (g / 20))[:, None], n=len(x), axis=0)


# ------------------------------------------------------------------ score
# (start, end, pad notes, sub root midi)
CHORDS = [
    (0.0, 5.0, [38, 45, 50, 52, 53, 57], 38),      # Dm(add9)
    (5.0, 11.0, [34, 41, 46, 50, 53, 57], 34),     # Bbmaj7
    (11.0, 14.5, [43, 50, 53, 57, 58], 31),        # Gm9
    (14.5, 18.0, [36, 43, 48, 50, 55], 36),        # Csus2
    (18.0, 21.5, [33, 40, 45, 50, 52], 33),        # Asus4
    (21.5, 25.0, [33, 40, 45, 49, 52, 55], 33),    # A7
    (25.0, 30.0, [38, 45, 50, 54, 57, 64, 66], 38) # D(add9) - resolution
]
MOTIF = [  # (time, midi, velocity)
    (1.0, 69, .55), (2.5, 74, .6), (3.5, 76, .5),
    (5.5, 77, .65), (6.5, 74, .55), (7.5, 69, .5), (9.0, 72, .55), (10.0, 74, .6),
    (11.5, 70, .6), (12.0, 74, .6), (13.0, 79, .7), (14.5, 76, .6), (15.0, 72, .55),
    (16.0, 67, .5), (17.0, 76, .6),
    (18.5, 76, .6), (19.0, 69, .5), (19.5, 74, .6), (20.5, 76, .65), (21.0, 69, .55),
    (21.5, 74, .65), (22.5, 76, .7), (23.0, 81, .75), (23.5, 73, .7), (24.0, 76, .75),
    (24.5, 81, .8),
    (25.0, 74, .9), (25.0, 78, .8), (25.0, 81, .7), (25.5, 69, .5), (26.0, 78, .55),
    (27.0, 74, .4),
]


def pad_layer():
    out = np.zeros((N, 2))
    xf = 0.9  # crossfade half-width (s)
    for ci, (a, b, notes, _) in enumerate(CHORDS):
        last = ci == len(CHORDS) - 1
        if last:   # swell in fast, long release
            env = smooth(T, a - 0.35, a + 0.45) * (1 - smooth(T, 26.6, 29.7) ** 1.3)
        else:
            env = smooth(T, a - xf if a > 0 else -0.01, a + xf if a > 0 else 1.2) * (1 - smooth(T, b - xf, b + (0.45 if ci == len(CHORDS) - 2 else xf)))
        idx = np.where(env > 1e-5)[0]
        if len(idx) == 0:
            continue
        i0, i1 = idx[0], idx[-1] + 1
        t = T[i0:i1]
        seg = np.zeros((i1 - i0, 2))
        for ni, m in enumerate(notes):
            f0 = hz(m)
            for k, det in enumerate((-9, -4, 0, 5, 10)):          # cents
                f = f0 * 2 ** (det / 1200)
                ph = rng.random()
                drift = 0.0015 * np.sin(2 * np.pi * (0.07 + 0.03 * k) * t + ph * 6)
                saw = 2 * ((f * t * (1 + drift) + ph) % 1.0) - 1
                amp = 1.0 / (1 + 0.35 * ni)
                seg += pan(saw * amp, (k - 2) / 2.5 * (0.6 if m > 44 else 0.15))
        seg /= len(notes) * 2.2
        dark = lp(seg, 420.0)
        mid = lp(seg, 1100.0)
        # filter "opens" slowly through the film, breathes with a slow LFO, opens at the reveal
        open_ = 0.12 + 0.5 * smooth(t, 3, 24.5) ** 1.5 + 0.10 * np.sin(2 * np.pi * 0.11 * t)
        if last:
            open_ = 0.85 - 0.45 * smooth(t, 26.5, 29.5)
        open_ = np.clip(open_, 0, 1)[:, None]
        out[i0:i1] += (dark * (1 - open_) + mid * open_) * env[i0:i1, None]
    return out


def sub_layer():
    out = np.zeros(N)
    for a, b, _, root in CHORDS:
        f = hz(root)
        while f < 40:
            f *= 2
        while f > 80:
            f /= 2
        step = 0.5 if 21.5 <= a < 25 else 1.0
        for t0 in np.arange(a, min(b, 28.5) - 1e-6, step):
            n = int(1.1 * SR)
            i0 = int(t0 * SR)
            n = min(n, N - i0)
            tt = np.arange(n) / SR
            env = (1 - np.exp(-tt / 0.012)) * np.exp(-tt / (0.36 if step == 1.0 else 0.22))
            # gentle pitch settle + 2nd harmonic so it reads on small speakers
            ph = 2 * np.pi * (f * tt + (f * 0.25) * 0.03 * (1 - np.exp(-tt / 0.03)))
            tone = np.sin(ph) + 0.28 * np.sin(2 * ph) + 0.08 * np.sin(3 * ph)
            lvl = 0.42 + 0.38 * smooth(np.array([t0]), 0, 24)[0]
            if t0 >= 25:
                lvl = 0.9 * np.exp(-(t0 - 25) / 1.6)
            if t0 < 1.0:
                lvl *= 0.6
            out[i0:i0 + n] += tone * env * lvl
    return pan(out, 0.0)


def ostinato():
    """Soft gated eighth-note pulse that enters in scene 3 and tightens through scene 4."""
    out = np.zeros(N)
    for a, b, notes, _ in CHORDS:
        if a < 11 or a >= 25:
            continue
        m = notes[2] + 12
        for t0 in np.arange(a, b - 1e-6, 0.25):
            n = int(0.22 * SR)
            i0 = int(t0 * SR)
            tt = np.arange(n) / SR
            f = hz(m if int(round(t0 * 4)) % 4 != 3 else m + 7)
            saw = 2 * ((f * tt) % 1.0) - 1 + 0.6 * (2 * ((f * 1.004 * tt) % 1.0) - 1)
            env = (1 - np.exp(-tt / 0.004)) * np.exp(-tt / 0.06)
            acc = 1.0 if int(round(t0 * 4)) % 2 == 0 else 0.6
            out[i0:i0 + n] += saw * env * acc * (0.15 + 0.85 * smooth(np.array([t0]), 11, 24.5)[0])
    out = lp(out, 750.0)
    st = np.stack([out, np.roll(out, int(0.011 * SR))], axis=1)
    return st


def pluck_layer():
    dry = np.zeros((N, 2))
    for k, (t0, m, vel) in enumerate(MOTIF):
        f = hz(m)
        n = int(1.6 * SR)
        i0 = int(t0 * SR)
        n = min(n, N - i0)
        tt = np.arange(n) / SR
        idx = 2.2 * vel * np.exp(-tt / 0.07)
        y = np.sin(2 * np.pi * f * tt + idx * np.sin(2 * np.pi * 2 * f * tt))
        y += 0.25 * np.sin(2 * np.pi * 0.5 * f * tt)
        y *= (1 - np.exp(-tt / 0.002)) * np.exp(-tt / (0.45 + 0.25 * vel)) * vel
        dry[i0:i0 + n] += pan(y, 0.45 * np.sin(k * 2.4))
    dry = lp(dry, 2600.0, 2)
    # ping-pong delay, dotted eighth of the 120 BPM grid
    d = int(0.375 * SR)
    wet = np.zeros_like(dry)
    cur = dry.copy()
    for rep in range(4):
        cur = np.roll(cur[:, ::-1], d, axis=0) * 0.42
        cur[:d] = 0
        cur = lp(cur, 1800.0, 1)
        wet += cur
    return dry + wet


def air_and_riser():
    noise = rng.standard_normal((N, 2))
    air = hp(lp(noise, 11000.0, 2), 6500.0, 2)
    env = 0.25 * smooth(T, 0, 6) + 0.75 * smooth(T, 8, 24.8) ** 2
    env *= 1 - smooth(T, 24.9, 25.05)
    env += 0.5 * smooth(T, 24.95, 25.4) * (1 - smooth(T, 25.6, 29.0))
    # tonal riser into the reveal: A octave glide under band-passed noise
    r = np.zeros(N)
    i0, i1 = int(22.0 * SR), int(25.0 * SR)
    tt = np.arange(i1 - i0) / SR
    f = hz(57) * 2 ** (tt / 3.0 * 1.0)
    ph = 2 * np.pi * np.cumsum(f) / SR
    r[i0:i1] = (np.sin(ph) + 0.5 * np.sin(2 * ph)) * (tt / 3.0) ** 2.5
    r[i1 - int(0.03 * SR):i1] *= np.linspace(1, 0, int(0.03 * SR))
    r = lp(r, 900.0, 2)
    return air * env[:, None] * 0.05 + pan(r, 0.0) * 0.07


def build_music():
    pad = pad_layer()
    sub = sub_layer()
    ost = ostinato()
    plk = pluck_layer()
    air = air_and_riser()
    bed = reverb(pad * 0.95 + ost * 0.16, 0.22, 2.6, 3500.0)
    plk = reverb(plk * 0.30, 0.35, 2.8, 4500.0)
    mix = bed + plk + air
    mix = hp(mix, 90.0, 2) + sub * 0.34            # keep the lows mono and clean
    mix = tilt_mid(mix)
    # overall dynamic arc: restrained start, slow build, release at 25
    arc = 0.62 + 0.38 * smooth(T, 2, 24.5)
    mix *= arc[:, None]
    mix *= (smooth(T, 0.0, 0.35) * (1 - smooth(T, 28.6, 29.85)))[:, None]   # clean in/out
    mix[int(29.9 * SR):] = 0.0
    return mix


# ------------------------------------------------------------------ sound design
DEFAULT_CUES = [{"t": 0.2, "kind": "draw_on"}, {"t": 5.2, "kind": "scene_start"},
                {"t": 11.2, "kind": "scene_start"}, {"t": 18.2, "kind": "scene_start"},
                {"t": 25.3, "kind": "brand_reveal"}]
RANGE_RE = re.compile(r"(\d+(?:\.\d+)?)\s*-\s*(\d+(?:\.\d+)?)\s*s\b")
LIST_RE = re.compile(r"at\s+((?:\d+(?:\.\d+)?\s*,\s*)+\d+(?:\.\d+)?)\s*s\b")
STEPS_RE = re.compile(r"(two|three|four|five|six|\d+)\s+(?:soft\s+)?steps?\s+(\d+(?:\.\d+)?)\s*s\s+apart")
WORDNUM = {"two": 2, "three": 3, "four": 4, "five": 5, "six": 6}


def load_cues():
    """Read ../cues.json (film specialist's sync points: [{t, kind, note}]) when present,
    else the documented defaults. Returns a list of concrete sound events."""
    p = os.path.join(HERE, "..", "cues.json")
    source = "cues.json"
    raw = None
    if os.path.exists(p):
        try:
            raw = json.load(open(p, encoding="utf-8"))
            if isinstance(raw, dict):
                raw = raw.get("cues") or raw.get("events") or []
        except Exception:
            raw = None
    if not raw:
        raw, source = DEFAULT_CUES, "defaults (scene starts + 0.2 s, brand reveal 25.3 s)"
    ev = []          # (t, type, gain, extra)
    whooshes = []    # (priority, t, gain)
    for c in raw:
        if not isinstance(c, dict) or not isinstance(c.get("t"), (int, float)):
            continue
        t = float(c["t"]); kind = str(c.get("kind", c.get("type", ""))).lower(); note = str(c.get("note", ""))
        if not (0 <= t < DUR - 0.3):
            continue
        times = [t]
        m = LIST_RE.search(note)
        if m:
            times = [float(x) for x in m.group(1).split(",")]
        m = STEPS_RE.search(note)
        if m:
            k = WORDNUM.get(m.group(1)) or int(m.group(1))
            times = [t + i * float(m.group(2)) for i in range(k)]
        rng_ = RANGE_RE.search(note)
        if kind == "brand_reveal":
            ev.append((t, "impact", 1.0, None)); whooshes.append((0, t, 0.85))
        elif kind == "scene_start":
            if t > 0.5:
                whooshes.append((1, t, 1.0)); ev.append((t, "tick", 0.6, None))
        elif kind == "transition":
            whooshes.append((2, t, 0.6))
        elif kind.startswith("impact"):
            ev.append((t, "impact", 0.45, None))
        elif kind in ("camera_move", "sweep", "unroll"):
            a, b = (float(rng_.group(1)), float(rng_.group(2))) if rng_ else (t, t + 1.5)
            ev.append((a, "sweep", 0.8, b))
        elif kind == "draw_on":
            for x in times:
                ev.append((x, "tick", 0.55 if len(times) > 1 else 0.45, None))
        elif kind == "reveal":
            for x in times:
                ev.append((x, "glint", 0.6 if len(times) == 1 else 0.45, None))
    kept = []
    for pr, t, g in sorted(whooshes):          # brand > scene start > transition; no pile-ups
        if all(abs(t - k[0]) >= 0.7 for k in kept):
            kept.append((t, "whoosh", g, None))
    ev = sorted(ev + kept, key=lambda e: e[0])
    return [{"t": round(e[0], 3), "type": e[1], "gain": e[2], **({"until": e[3]} if e[3] else {})} for e in ev], source


def sfx_glint(out, t0, gain=1.0):
    """Very soft two-partial sine blip for on-screen reveals (no bell, no 'ding')."""
    n = min(int(0.35 * SR), N - int(t0 * SR)); i0 = int(t0 * SR)
    tt = np.arange(n) / SR
    y = (np.sin(2 * np.pi * 880 * tt) + 0.5 * np.sin(2 * np.pi * 1320 * tt + 0.3)) * np.exp(-tt / 0.07)
    y *= 1 - np.exp(-tt / 0.004)
    out[i0:i0 + n] += pan(y * 0.07 * gain, -0.2)


def sfx_sweep(out, t0, t1, gain=1.0):
    """Slow airy movement bed for camera moves / unrolls: band-passed noise, drifting pan."""
    i0 = int(t0 * SR); i1 = min(N, int(t1 * SR)); n = i1 - i0
    if n <= 0:
        return
    tt = np.arange(n) / SR; d = n / SR
    env = np.sin(np.pi * np.clip(tt / d, 0, 1)) ** 1.5
    noise = rng.standard_normal(n)
    y = (bp(noise, 300, 700) * 0.8 + bp(noise, 6000, 9500) * 0.35) * env
    th = (0.5 * np.sin(np.pi * tt / d - np.pi / 2) + 1) * np.pi / 4
    out[i0:i1] += np.stack([y * np.cos(th), y * np.sin(th)], axis=1) * 0.07 * gain


def sfx_tick(out, t0, gain=1.0):
    n = int(0.09 * SR)
    i0 = int(t0 * SR)
    n = min(n, N - i0)
    tt = np.arange(n) / SR
    y = np.sin(2 * np.pi * 2100 * tt) * np.exp(-tt / 0.006) * 0.5
    y += np.sin(2 * np.pi * 520 * tt) * np.exp(-tt / 0.018) * 0.6
    y += bp(rng.standard_normal(n), 5000, 9000) * np.exp(-tt / 0.004) * 0.25
    y *= 1 - np.exp(-tt / 0.0006)
    out[i0:i0 + n] += pan(y * 0.22 * gain, 0.15)


def sfx_whoosh(out, t0, gain=1.0):
    """Filtered-noise swell that crests on the cue and falls away after it."""
    pre, post = 0.55, 0.50
    i0 = max(0, int((t0 - pre) * SR)); i1 = min(N, int((t0 + post) * SR))
    n = i1 - i0
    tt = np.arange(n) / SR + i0 / SR - t0
    env = np.where(tt < 0, np.exp(tt / 0.17), np.exp(-tt / 0.14))
    env *= smooth(tt, -pre, -pre + 0.1)
    noise = rng.standard_normal(n)
    lo = bp(noise, 250, 900) * 0.9
    hi = bp(noise, 5000, 10000) * 0.45
    sweep = 0.5 + 0.5 * np.clip((tt + pre) / pre, 0, 1.3)       # brighter towards the crest
    y = (lo * (1.3 - sweep * 0.6) + hi * sweep) * env
    p = np.clip(tt / pre, -1, 1) * 0.6                           # travels left -> right
    th = (p + 1) * np.pi / 4
    out[i0:i1] += np.stack([y * np.cos(th), y * np.sin(th)], axis=1) * 0.20 * gain


def sfx_impact(out, t0, gain=1.0):
    n = min(int(2.4 * SR), N - int(t0 * SR))
    i0 = int(t0 * SR)
    tt = np.arange(n) / SR
    f = 46 + 38 * np.exp(-tt / 0.05)
    thump = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-tt / 0.32) * (1 - np.exp(-tt / 0.004))
    body = lp(rng.standard_normal(n), 380.0, 2) * np.exp(-tt / 0.12) * 0.5
    shim = hp(rng.standard_normal((n, 2)), 6000.0, 2) * (np.exp(-tt / 0.55) * (1 - np.exp(-tt / 0.03)))[:, None]
    y = pan((thump * 0.9 + body) * 0.42, 0.0) + shim * 0.035
    y[-int(0.05 * SR):] *= np.linspace(1, 0, int(0.05 * SR))[:, None]
    out[i0:i0 + n] += y * gain


def build_sfx():
    cues, source = load_cues()
    out = np.zeros((N, 2))
    for c in cues:
        t0, g = c["t"], c["gain"]
        {"impact": sfx_impact, "whoosh": sfx_whoosh, "tick": sfx_tick, "glint": sfx_glint}.get(
            c["type"], lambda o, t, gg: sfx_sweep(o, t, c["until"], gg))(out, t0, g)
    out = reverb(out, 0.10, 1.4, 6000.0)
    out *= (1 - smooth(T, 29.0, 29.85))[:, None]
    out[int(29.9 * SR):] = 0.0
    json.dump({"source": source, "cues": cues}, open(os.path.join(HERE, "sfx-cues-used.json"), "w"), indent=1)
    return out, source


def main():
    music = build_music()
    music *= 10 ** (-6 / 20) / np.abs(music).max()       # peak -6 dBFS stem
    sf.write(os.path.join(HERE, "music.wav"), music, SR, subtype="PCM_24")
    sfx, source = build_sfx()
    sfx *= 10 ** (-9 / 20) / np.abs(sfx).max()           # peak -9 dBFS stem
    sf.write(os.path.join(HERE, "sfx.wav"), sfx, SR, subtype="PCM_24")
    print("music.wav, sfx.wav written; sfx cues from:", source)


if __name__ == "__main__":
    main()
