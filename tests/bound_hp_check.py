"""Work plan task 1.2, 40-digit tier of the bound tests (docs/backend/TOLERANCE_POLICY.md section 8).

Reads the lines written by `crates/uem/tests/property_bounds.rs` when NF_BOUND_DUMP is set, and for every
dumped polyline re-evaluates the exact curve with mpmath (40 significant digits), independently of the engine:
  * the vertex error of every dumped vertex against its exact point, to be <= eps_num (the a-priori vertex bound);
  * the chord error of every dumped segment at the OFF-grid samples of crates/uem/tests/property_bounds.rs
    (sample_offsets: irrational fractions and sixteenth midpoints), to be <= eps_rep (the certified bound).
The exact curves are the closed forms of Spec 02 section 4.3 (stub) and section 4.4 (opening), in mpmath.
NOT RUN on this host: mpmath is not installed here (docs/backend/TOLERANCE_POLICY.md section 8.6).
Usage: python3 tests/bound_hp_check.py <dump file>
"""
import math
import sys

import mpmath as mp

mp.mp.dps = 40
TAU = 2.0 * math.pi  # the binary64 value the engine uses
GOLD = 0.6180339887498949  # as in property_bounds.rs


def sample_offsets():
    """The offsets u in (0, 1) of the samples of one segment, identical to property_bounds.rs sample_offsets()."""
    v = [(0.5 + j * GOLD) % 1.0 for j in range(1, 16)]
    v += [(k + 0.5) / 16.0 for k in range(16)]
    return v


def exact_stub(R, r, beta_deg, delta, phi_f):
    """Saddle of the branch stub: (r*phi, s(phi)) with s = (sqrt(R^2 - y^2) - r sin(phi) cos(beta)) / sin(beta)."""
    R, r, d = mp.mpf(R), mp.mpf(r), mp.mpf(delta)
    beta = mp.mpf(beta_deg * (math.pi / 180.0))  # the engine's binary64 to_radians value, exact in mp
    phi = mp.mpf(phi_f)
    y = d + r * mp.cos(phi)
    s = (mp.sqrt(R * R - y * y) - r * mp.sin(phi) * mp.cos(beta)) / mp.sin(beta)
    return (r * phi, s)


def exact_opening(R, r, beta_deg, delta, station, phi_f):
    """Header opening: (x, y) with x = station + s cos(beta) - r sin(phi) sin(beta), y = R (asin(y/R) + pi)."""
    R, r, d, st = mp.mpf(R), mp.mpf(r), mp.mpf(delta), mp.mpf(station)
    beta = mp.mpf(beta_deg * (math.pi / 180.0))
    phi = mp.mpf(phi_f)
    yb = d + r * mp.cos(phi)
    s = (mp.sqrt(R * R - yb * yb) - r * mp.sin(phi) * mp.cos(beta)) / mp.sin(beta)
    x = st + s * mp.cos(beta) - r * mp.sin(phi) * mp.sin(beta)
    y = R * (mp.asin(yb / R) + mp.pi)
    return (x, y)


def dist_segment(q, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    l2 = dx * dx + dy * dy
    t = ((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l2 if l2 > 0 else mp.mpf(0)
    t = min(max(t, mp.mpf(0)), mp.mpf(1))
    px, py = a[0] + t * dx, a[1] + t * dy
    return mp.sqrt((q[0] - px) ** 2 + (q[1] - py) ** 2)


def check_line(line):
    f = line.split()
    family = f[0]
    if family == "stub":
        R, r, beta, delta = (float(v) for v in f[1:5])
        k = 5
        station = None
    else:
        R, r, beta, delta, station = (float(v) for v in f[1:6])
        k = 6
    n = int(f[k])
    eps_rep, eps_num = float(f[k + 1]), float(f[k + 2])
    quads = f[k + 3:]
    rows = [quads[i:i + 5] for i in range(0, len(quads), 5)]
    worst_vertex, worst_chord = 0.0, 0.0
    dphi = TAU / (4 * n) if family == "stub" else TAU / n
    step = 4 if family == "stub" else 1  # the vertex parameter index multiplier of develop's sampling

    def exact(phi_f):
        if family == "stub":
            return exact_stub(R, r, beta, delta, phi_f)
        return exact_opening(R, r, beta, delta, station, phi_f)

    def vertex_phi(i):
        return float(step * i) * dphi

    for row in rows:
        i = int(row[0])
        a = (float(row[1]), float(row[2]))
        b = (float(row[3]), float(row[4]))
        for idx, pt in ((i, a), (i + 1, b)):
            ex = exact(vertex_phi(idx))
            err = mp.sqrt((mp.mpf(pt[0]) - ex[0]) ** 2 + (mp.mpf(pt[1]) - ex[1]) ** 2)
            worst_vertex = max(worst_vertex, float(err))
        p0, p1 = vertex_phi(i), vertex_phi(i + 1)
        A = (mp.mpf(a[0]), mp.mpf(a[1]))
        B = (mp.mpf(b[0]), mp.mpf(b[1]))
        for u in sample_offsets():
            phi = p0 + (p1 - p0) * u
            q = exact(phi)
            worst_chord = max(worst_chord, float(dist_segment(q, A, B)))
    return family, eps_rep, eps_num, worst_vertex, worst_chord


def main(path):
    tally = {}
    for line in open(path):
        if not line.strip():
            continue
        family, eps_rep, eps_num, wv, wc = check_line(line)
        t = tally.setdefault(family, {"cases": 0, "vertex_ratio": 0.0, "chord_ratio": 0.0, "vertex_fail": 0, "chord_fail": 0})
        t["cases"] += 1
        vr = wv / eps_num if eps_num > 0 else (0.0 if wv == 0 else float("inf"))
        cr = wc / eps_rep if eps_rep > 0 else float("inf")
        t["vertex_ratio"] = max(t["vertex_ratio"], vr)
        t["chord_ratio"] = max(t["chord_ratio"], cr)
        t["vertex_fail"] += vr > 1.0
        t["chord_fail"] += cr > 1.0
    for family, t in tally.items():
        print(f"HP {family}: cases {t['cases']}, max vertex error / eps_num {t['vertex_ratio']:.4f} (fail {t['vertex_fail']}), "
              f"max sampled chord error / eps_rep {t['chord_ratio']:.4f} (fail {t['chord_fail']})")
    bad = sum(t["vertex_fail"] + t["chord_fail"] for t in tally.values())
    print("HP result:", "PASS" if bad == 0 else f"FAIL ({bad})")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
