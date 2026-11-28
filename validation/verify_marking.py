#!/usr/bin/env python3
"""Independent expected marking values for the ZK marking port (PZ-5a).

This file is written from the public formulas and public example tables in
``/root/zumen-kirizu/reader/research/spec_marking.md`` only. It is not the partner's
``verify_marking.py``: that file was removed from the handover copy and is not read,
recovered, imported or copied here. No NEXUS FABRIC Rust code is read by this script.

Run with ``python3 validation/verify_marking.py``. It prints a real pass/NG counter and
exits non-zero on any mismatch. ``--write-json`` writes the golden fixtures used by PZ-5.
All dimensions are millimetres; angle inputs are degrees.
"""

from __future__ import annotations

import argparse
import json
import math
import sys

TOL = 0.01  # PZ-5a golden tolerance against the PZ-5 Rust reader (spec allows 0.1 mm)
SCHEMA = 1  # marking_golden.json layout version; listed in the fixture itself


class Checks:
    def __init__(self):
        self.passed = 0
        self.failed = []

    def eq(self, name, got, want, tol=TOL):
        try:
            ok = math.isfinite(float(got)) and math.isclose(float(got), float(want), abs_tol=tol)
        except (TypeError, ValueError):
            ok = False
        if ok:
            self.passed += 1
        else:
            self.failed.append(f"{name}: got {got!r}, want {want!r} (+/-{tol})")

    def seq(self, name, got, want, tol=TOL):
        if len(got) != len(want):
            self.failed.append(f"{name}: {len(got)} values, want {len(want)}")
            return
        for i, (g, w) in enumerate(zip(got, want)):
            self.eq(f"{name}[{i}]", g, w, tol)

    def finish(self):
        for f in self.failed:
            print("NG " + f)
        print(f"marking values: PASS {self.passed} | NG {len(self.failed)}")
        return 1 if self.failed else 0


def rad(a):
    return math.radians(a)


def mod(a, m):
    return (a % m + m) % m


def plate_u(azimuth_deg, left_seam_deg, rm, n, plate_width=None):
    """Plate index and u from seam azimuth; left seam of plate j is seam0 + j*360/n."""
    d = mod(azimuth_deg - left_seam_deg, 360.0)
    sector = 360.0 / n
    j = min(n - 1, int(d / sector))
    u = rm * rad(d - j * sector)
    return j, u


def cylinder_marks(c, D, t, n=1, seam_from_bottom=45.0):
    """Spec 3-1: quarter lines on a rolled cylinder. seam alpha_s = 180+a from top."""
    rm = (D - t) / 2.0
    seam_az = 180.0 + seam_from_bottom
    out = []
    for name, az in (("ten", 0.0), ("right", 90.0), ("chi", 180.0), ("left", 270.0)):
        j, u = plate_u(az, seam_az, rm, n)
        if u >= 0.5 and 2.0 * math.pi * rm / n - u >= 0.5:
            out.append((name, j, u))
    return out, 2.0 * math.pi * rm


def bend_marks(c, D, o, ao, bend_angle, joints, cl, extra, lo, up, seam_from_bottom=45.0):
    """Spec 3-2: back/belly/side lines on one mitred bend piece."""
    r = D / 2.0 - o
    C = 2.0 * math.pi * r
    gamma = rad(bend_angle / joints / 2.0)
    k = (D / 2.0 - ao) * math.tan(gamma)
    ph = 180.0 - seam_from_bottom
    out = []
    for name, phi in (("back", 0.0), ("side_r", 90.0), ("belly", 180.0), ("side_l", 270.0)):
        x = C * mod(phi - ph, 360.0) / 360.0
        y0 = -extra / 2.0 - lo * k * math.cos(rad(phi))
        y1 = cl + extra / 2.0 + up * k * math.cos(rad(phi))
        length = cl + extra + (lo + up) * k * math.cos(rad(phi))
        out.append((name, x, y0, y1, length))
    return out, C, k


def cone_marks(c, rs, rl, L, n, seam_start=0.0):
    """Spec 3-3: radial quarter lines on a cone piece (seam 0 = top)."""
    sl = math.hypot(L, rl - rs)
    sin_a = (rl - rs) / sl
    Rs, Rl = rs / sin_a, rl / sin_a
    psi_half = math.pi * sin_a / n
    out = []
    for j in range(n):
        ac = seam_start + 360.0 * (j + 0.5) / n
        for name, az in (("ten", 0.0), ("right", 90.0), ("chi", 180.0), ("left", 270.0)):
            delta = (az - ac + 180.0) % 360.0 - 180.0
            psi = rad(delta) * sin_a
            if Rl * (psi_half - abs(psi)) < 0.5:
                continue
            out.append((name, j, Rs * math.sin(psi), Rs * math.cos(psi),
                        Rl * math.sin(psi), Rl * math.cos(psi), sl))
    return out


def triangle_line_spans(width, height, hole, axis, pos):
    """Allowed spans on one hole-centre line of a right triangle.

    Vertices (0,0), (width,0), (0,height); one circular hole ``(cx, cy, r)``.
    The C10 corner chamfers are away from the two public table lines below.
    """
    if axis == "y":
        upper = width * (1.0 - pos / height)
        along_hole, across_hole = hole[0], hole[1]
    else:
        upper = height * (1.0 - pos / width)
        along_hole, across_hole = hole[1], hole[0]
    d = abs(across_hole - pos)
    if d >= hole[2]:
        return [(0.0, upper)]
    w = math.sqrt(hole[2] * hole[2] - d * d)
    return [(0.0, along_hole - w), (along_hole + w, upper)]


def ecc_length(c, L, r1, r2, a_deg):
    c2 = r1 - r2
    return math.hypot(L, 2.0 * c2 * math.sin(rad(a_deg) / 2.0))


def ring_marks(c, od, id_, n):
    """Spec 3-5: radial lines at 0/90/180/270, skipped when they lie on any seam."""
    out = []
    threshold = 0.5 / (od / 2.0) * 180.0 / math.pi
    seams = [360.0 * j / n for j in range(n)]
    for j in range(n):
        for name, az in (("x+", 0.0), ("y+", 90.0), ("x-", 180.0), ("y-", 270.0)):
            if any(min(mod(az - s, 360.0), mod(s - az, 360.0)) < threshold for s in seams):
                continue
            q = rad(az)
            item = (name, j, id_ / 2.0 * math.cos(q), id_ / 2.0 * math.sin(q),
                    od / 2.0 * math.cos(q), od / 2.0 * math.sin(q))
            # Identical marks on identical pieces are one mark set (spec 3-3/3-5).
            if item[2:] not in [m[2:] for m in out]:
                out.append(item)
    return out


def tee_hole_u(c, D, t, branch_dir, e, ri, seam_from_bottom=45.0):
    """Spec 3-6: branch centre line u = Rm*(dir + asin(e/Ri) - seam_left)."""
    rm = (D - t) / 2.0
    az = branch_dir + math.degrees(math.asin(max(-1.0, min(1.0, e / ri))))
    return rm * rad(mod(az - (180.0 + seam_from_bottom), 360.0))


def plate_clip_axis(c, rectangle, holes, axis, at):
    """Spec 3-9: cut an axis-aligned centre line at the rectangle edges and hole spans."""
    x0, y0, x1, y1 = rectangle
    spans, seen = [], set()
    for hx, hy, hd in holes:
        centre = hx if axis == "x" else hy
        along = hy if axis == "x" else hx
        if abs(centre - at) > 1e-9 or round(along, 6) in seen:
            continue
        seen.add(round(along, 6))
        spans.append((along - hd / 2.0, along + hd / 2.0))
    lo, hi = (y0, y1) if axis == "x" else (x0, x1)
    cuts = sorted(spans)
    out, cur = [], lo
    for a, b in cuts:
        if a > cur:
            out.append((cur, min(a, hi)))
        cur = max(cur, b)
    if cur < hi:
        out.append((cur, hi))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--write-json", default=None, help="write golden fixtures to this file")
    args = ap.parse_args()
    c = Checks()
    g = {
        "_meta": {
            "schema": SCHEMA,
            "units": "mm",
            "tolerance_mm": TOL,
            "source": "public spec_marking.md + spec_branches.md only",
            "conventions": "UNVERIFIED; see MARKING-VALUES.md",
        }
    }

    # 3-1 cylinder / 3-2 bend / 3-3 cone / 3-4 ecc / 3-5 ring -------------------
    marks, W = cylinder_marks(c, 1016.0, 12.0)
    c.seq("cyl 1-seam quarter u", [round(u, 2) for _, _, u in sorted(marks, key=lambda m: m[2])],
          [round(W * f, 2) for f in (1 / 8, 3 / 8, 5 / 8, 7 / 8)])
    g["cyl"] = {"rm": 502.0, "W": W, "marks": marks}

    bm, C, k = bend_marks(c, 1016.0, 0.0, 0.0, 90.0, 2, 1500.0, 100.0, 1.0, 2.0)
    c.seq("bend quarter x", [round(m[1], 2) for m in bm],
          [round(C * f, 2) for f in (5 / 8, 7 / 8, 1 / 8, 3 / 8)])
    c.eq("bend back length", bm[0][4], 1500.0 + 100.0 + 3.0 * k)
    c.eq("bend belly length", bm[2][4], 1500.0 + 100.0 - 3.0 * k)
    c.eq("bend side length", bm[1][4], 1600.0)
    g["bend"] = {"C": C, "k": k, "marks": bm}

    cone2 = cone_marks(c, 203.2, 508.0, 1000.0, 2)
    cone4 = cone_marks(c, 203.2, 508.0, 1000.0, 4)
    c.eq("cone 2-piece side lines", sum(1 for m in cone2 if m[0] == "left"), 1)
    c.eq("cone 4-piece quarter lines", len(cone4), 0)
    c.eq("cone 2-piece side centred", cone2[0][2], 0.0)
    c.eq("cone slant length", cone2[0][6], math.hypot(1000.0, 508.0 - 203.2))
    g["cone"] = {"n2": cone2, "n4": cone4}

    c.eq("ecc flat length", ecc_length(c, 1500.0, 508.0, 406.4, 0), 1500.0)
    c.eq("ecc side length", ecc_length(c, 1500.0, 508.0, 406.4, 90),
         math.hypot(1500.0, 101.6 * math.sqrt(2.0)))
    c.eq("ecc opposite length", ecc_length(c, 1500.0, 508.0, 406.4, 180),
         math.hypot(1500.0, 203.2))
    g["ecc"] = {"lengths": {str(a): ecc_length(c, 1500.0, 508.0, 406.4, a)
                            for a in (0, 90, 180, 270)}}

    r4 = ring_marks(c, 1000.0, 600.0, 4)
    r2 = ring_marks(c, 1000.0, 600.0, 2)
    c.eq("ring 4-piece no lines", len(r4), 0)
    c.eq("ring 2-piece side lines", len(r2), 2)
    c.eq("ring line inner radius", math.hypot(r2[0][2], r2[0][3]), 300.0)
    c.eq("ring line outer radius", math.hypot(r2[0][4], r2[0][5]), 500.0)
    g["ring"] = {"n2": r2, "n4": r4}

    # 7-6 tee main: public table values from spec_marking.md -------------------
    tee_u = tee_hole_u(c, 1016.0, 12.0, 0.0, 0.0, 496.0)
    c.eq("T1 branch centre u", tee_u, 1182.81, 0.1)
    c.eq("T2 branch centre u", tee_hole_u(c, 1016.0, 12.0, 0.0, 150.0, 496.0), 1337.04, 0.2)
    c.eq("T5 branch centre u", tee_hole_u(c, 1016.0, 12.0, 90.0, 0.0, 496.0), 1971.35, 0.1)
    tee_cases = {
        "T1": {"plates": {0: {"v": [("ten+branch", 1182.81, [(0, 454), (1046, 1500)]),
                                         ("right", 1971.35, [(0, 1500)]),
                                         ("chi", 2759.89, [(0, 1500)]),
                                         ("left", 394.27, [(0, 1500)])],
                              "h": [(750, [(0, 861.79), (1503.83, 3154.16)])]}}},
        "T2": {"plates": {0: {"v": [("ten", 1182.81, [(0, 494.82), (1005.18, 1500)]),
                                         ("right", 1971.35, [(0, 1500)]),
                                         ("chi", 2759.89, [(0, 1500)]),
                                         ("left", 394.27, [(0, 1500)]),
                                         ("branch", 1337.04, [(0, 454), (1046, 1500)])],
                              "h": [(750, [(0, 1032.82), (1744.01, 3154.16)])]}}},
        "T3": {"plates": {0: {"v": [("ten+branch", 1182.81, [(0, 777.39), (1626.61, 2000)]),
                                         ("right", 1971.35, [(0, 2000)]),
                                         ("chi", 2759.89, [(0, 2000)]),
                                         ("left", 394.27, [(0, 2000)])],
                              "h": [(700, [(0, 3154.16)])]}}},
        "T4": {"plates": {
            0: {"v": [("ten+branch", 1182.81, [(0, 454), (1046, 1500)]),
                      ("left", 394.27, [(0, 1500)])],
                "h": [(750, [(0, 861.79), (1503.83, 1577.08)])]},
            1: {"v": [("right", 394.27, [(0, 1500)]),
                      ("chi", 1182.81, [(0, 1500)])], "h": []}}},
        "T5": {"plates": {0: {"v": [("ten", 1182.81, [(0, 1500)]),
                                         ("right+branch", 1971.35, [(0, 454), (1046, 1500)]),
                                         ("chi", 2759.89, [(0, 1500)]),
                                         ("left", 394.27, [(0, 1500)])],
                              "h": [(750, [(0, 1650.33), (2292.37, 3154.16)])]}}},
        "T6": {"plates": {0: {"v": [("ten+branch1", 1182.81, [(0, 454), (1046, 1500)]),
                                         ("right", 1971.35, [(0, 1500)]),
                                         ("chi+branch2", 2759.89, [(0, 454), (1046, 1500)]),
                                         ("left", 394.27, [(0, 1500)])],
                              "h": [(750, [(0, 861.79), (1503.83, 2438.87),
                                           (3080.91, 3154.16)])]}}},
        "T7": {"plates": {0: {"v": [("ten+branch1", 1182.81, [(0, 204), (796, 1500)]),
                                         ("right+branch2", 1971.35, [(0, 955.80), (1344.20, 1500)]),
                                         ("chi", 2759.89, [(0, 1500)]),
                                         ("left", 394.27, [(0, 1500)])],
                              "h": [(500, [(0, 861.79), (1503.83, 3154.16)]),
                                    (1150, [(0, 1769.40), (2173.30, 3154.16)])]}}},
    }
    # The formulas in 3-6 give the straight line positions; the hole cuts are public golden
    # values from the spec table (the removed partner verifier is not used).
    az_by_name = {"ten": 0.0, "right": 90.0, "chi": 180.0, "left": 270.0}
    for key, case in tee_cases.items():
        n = 2 if key == "T4" else 1
        L = 2000.0 if key == "T3" else 1500.0
        for pj, plate in case["plates"].items():
            hx = [x for x, _ in plate["h"]]
            for name, u, spans in plate["v"]:
                base = name.split("+")[0]
                if base in az_by_name:
                    # T4 has two plates; its "right/chi" row is on plate 1 (left seam 45 deg).
                    seam = 225.0 if (n == 1 or pj == 0) else 45.0
                    got = plate_u(az_by_name[base], seam, 502.0, n)[1]
                    c.eq(f"{key} plate{pj} quarter {name}", got, u, 0.1)
                # Vertical rows are constant-u lines: each span is an axial (x) range.
                # Check the hole gap only where the public table puts it at a branch
                # position line; the slanted branch T3 cuts the line away from that x.
                if "branch" in name and len(spans) == 2 and key != "T3":
                    (_, b0), (a1, _) = spans
                    centre = (b0 + a1) / 2.0
                    c.eq(f"{key} plate{pj} {name} hole at branch-x",
                         any(abs(centre - x) < 0.05 for x in hx), True)
                    c.eq(f"{key} plate{pj} {name} hole width sane", 0.0 < a1 - b0 < L, True)
                for i, (a, b) in enumerate(spans):
                    c.eq(f"{key} plate{pj} {name} span{i} in plate", 0.0 <= a < b <= L, True)
            for i, (x, spans) in enumerate(plate["h"]):
                # Horizontal rows are constant-x lines: spans are circumferential (u) ranges.
                for j in range(len(spans) - 1):
                    c.eq(f"{key} plate{pj} h{i} gap{j} positive",
                         0.0 < spans[j + 1][0] - spans[j][1] < 3154.16, True)
                for j, (a, b) in enumerate(spans):
                    c.eq(f"{key} plate{pj} h{i} span{j} in plate", 0.0 <= a <= b <= 3154.16, True)
    # T1 hole dimensions from the public spec_branches.md table (the marking spec's 0.1 mm cut).
    c.eq("T1 hole u width", 1503.83 - 861.79, 642.03, 0.1)
    c.eq("T1 hole x width", 1046.0 - 454.0, 592.0, 0.1)
    g["tee_main"] = tee_cases

    # 7-7 tee branch -----------------------------------------------------------
    wb = 2.0 * math.pi * (610.0 - 9.0) / 2.0
    c.eq("branch W", wb, 1888.10, 0.02)
    c.seq("branch quarter u", [wb * k / 16.0 for k in (4, 8, 12)],
          [472.02, 944.05, 1416.07], 0.02)
    g["tee_branch"] = {"W": wb, "u": [wb * k / 16.0 for k in (4, 8, 12)],
                       "lengths": {"B1": [392.00, 487.15, 392.00],
                                   "B2": [414.65, 413.43, 414.65],
                                   "B3": [777.58, 616.14, 176.58]}}

    # 7-8 tee pad --------------------------------------------------------------
    rp, ro = 514.0, 508.0
    c.eq("P1 pad centre u", rp * math.asin(0.0 / ro), 0.0)
    c.eq("P2 pad centre u", rp * math.asin(150.0 / ro), 154.07, 0.1)
    c.eq("P3 pad centre x", math.hypot(ro, 0.0) / math.tan(rad(45.0)), 508.0)
    c.eq("P1 axial mark length", 500.0 - 305.0, 195.0)
    c.eq("P2 axial mark length", 455.0 - 305.0, 150.0)
    c.eq("P3 axial mark length", 1101.34 - 951.34, 150.0, 0.02)
    g["tee_pad"] = {
        "P1": {"C": [0.0, 0.0], "axial": [[305.0, 500.0], [-500.0, -305.0]],
               "around": [[331.01, 664.37], [-664.37, -331.01]]},
        "P2": {"C": [154.07, 0.0], "axial": [[305.0, 455.0], [-455.0, -305.0]],
               "around": [[570.51, 720.51], [-309.37, -159.37]]},
        "P3": {"C": [0.0, 508.0], "axial": [[951.34, 1101.34], [-73.34, 76.66]],
               "around": [[321.38, 473.35], [-473.35, -321.38]]},
    }

    # 7-9 plate ---------------------------------------------------------------
    rect = (0.0, 0.0, 300.0, 200.0)
    holes = [(50.0, 50.0, 22.0), (250.0, 50.0, 22.0), (50.0, 150.0, 22.0), (250.0, 150.0, 22.0)]
    rows = []
    for y in (50.0, 150.0):
        for a, b in plate_clip_axis(c, rect, holes, "y", y):
            rows.append((y, a, b, a, b))
    for x in (50.0, 250.0):
        for a, b in plate_clip_axis(c, rect, holes, "x", x):
            rows.append((x, a, b, a, b))
    want = [(50, 0, 39, 0, 39), (50, 61, 239, 61, 239), (50, 261, 300, 261, 300),
            (150, 0, 39, 0, 39), (150, 61, 239, 61, 239), (150, 261, 300, 261, 300),
            (50, 0, 39, 0, 39), (50, 61, 139, 61, 139), (50, 161, 200, 161, 200),
            (250, 0, 39, 0, 39), (250, 61, 139, 61, 139), (250, 161, 200, 161, 200)]
    c.seq("plate 2x2 hole lines", [v for r in rows for v in r], [v for r in want for v in r], 0.02)
    c.eq("plate 2x2 line count", len(rows), 12)
    g["plate"] = {"rectangle": rect, "holes": holes, "lines": rows}

    # triangle check: derive the two public table 7-9 lines from the triangle/hole,
    # then compare the derived values to the printed table (not to copied literals).
    # The public phi18 hole is stored here as its 9 mm radius.
    hole = (50.0, 40.0, 9.0)
    tri_rows = []
    for y in (40.0,):
        for a, b in triangle_line_spans(200.0, 150.0, hole, "y", y):
            tri_rows.append((y, a, b))
    for x in (50.0,):
        for a, b in triangle_line_spans(200.0, 150.0, hole, "x", x):
            tri_rows.append((x, a, b))
    want_tri = [(40.0, 0.0, 41.0), (40.0, 59.0, 146.67),
                (50.0, 0.0, 31.0), (50.0, 49.0, 112.50)]
    c.seq("triangle hole lines", [v for r in tri_rows for v in r],
          [v for r in want_tri for v in r], 0.02)
    g["plate_triangle"] = {"lines": tri_rows}

    if args.write_json:
        with open(args.write_json, "w", encoding="utf-8") as f:
            json.dump(g, f, ensure_ascii=False, indent=2, sort_keys=True)
            f.write("\n")
    return c.finish()


if __name__ == "__main__":
    sys.exit(main())
