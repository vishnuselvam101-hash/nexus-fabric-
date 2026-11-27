#!/usr/bin/env python3
"""Independent expected values for a mitered (lobster-back) bend. Run: python3 -I verify_miterbend.py

Written from first principles (docs/shapes/spec_miterbend.md), standard library only. It does not use, and was written
before reading, any zumen-kirizu bend code. Nothing here is a factory convention except the parameters marked UNVERIFIED.

Two independent routes to the same numbers, counted by one pass/fail counter:
  Route A  closed forms (tan of the half cut angle, tangent lengths of a circumscribed polygon).
  Route B  3D geometry: the cut curve of every segment is found by intersecting the segment's cylinder with the
           cut plane numerically (a line-plane solve per azimuth), with no tan() formula, then compared with route A.
Checks: (1) t -> 0 limit: the three thickness bases coincide and equal the plain oblique cut of a cylinder;
        (2) closed forms at 90 deg with 2 joints (3 pieces) and 1 joint (2 pieces), plus 3D agreement for many cases;
        (3) neighbouring segments share their cut ellipse in 3D; developed area = circumference x centre length (quadrature).
Exit status 0 only if every check passes.
"""
import math
import sys

# ---- counter -----------------------------------------------------------------------------------------------------
N_OK = 0
N_NG = 0
GROUPS = {}


def check(group, name, got, want, tol=1e-9):
    global N_OK, N_NG
    ok = got is not None and math.isfinite(got) and abs(got - want) <= tol
    if ok:
        N_OK += 1
    else:
        N_NG += 1
        print(f"NG {group} {name}: got {got} want {want} (tol {tol})")
    GROUPS[group] = GROUPS.get(group, 0) + 1


def check_true(group, name, cond):
    global N_OK, N_NG
    if cond:
        N_OK += 1
    else:
        N_NG += 1
        print(f"NG {group} {name}")
    GROUPS[group] = GROUPS.get(group, 0) + 1


# ---- model (spec sections 2-5) ----------------------------------------------------------------------------------
# UNVERIFIED factory number: piece-count rule from public sources (MAFF 9.7.1(1)a, medium confidence). Parameter, not law.
DEFAULT_RULE = [(30.0, 2), (60.0, 3), (90.0, 4)]  # (largest angle in deg included, number of pieces)


def pieces_for_angle(angle_deg, rule=DEFAULT_RULE):
    """Number of pieces for a bend angle; the upper bound of each band is inclusive. Beyond the table: ValueError."""
    if not (angle_deg > 0 and math.isfinite(angle_deg)):
        raise ValueError("angle must be a positive finite number")
    for upper, n in rule:
        if angle_deg <= upper:
            return n
    raise ValueError("angle above the rule table")


def basis_radius(D, t, basis):
    return {"outer": D / 2.0, "mid": (D - t) / 2.0, "inner": D / 2.0 - t}[basis]


class Bend:
    """Centre-line radius R, total angle A (deg), n pieces (J = n-1 joints), straights s1/s2 on the end pieces,
    extra length `allow` added to every piece's centre length (UNVERIFIED factory number, default 0)."""

    def __init__(self, D, t, R, A, n, basis="mid", s1=0.0, s2=0.0, allow=0.0, seam_deg=180.0):
        assert n >= 2 and R > 0 and D > 0 and 0 < t < D / 2 and 0 < A < 180
        self.D, self.t, self.R, self.A, self.n = D, t, R, A, n
        self.J = n - 1
        self.theta = math.radians(A) / (2 * self.J)  # half the deflection of one joint = angle between a cut plane and the cross-section
        self.r = basis_radius(D, t, basis)
        self.basis = basis
        self.s1, self.s2, self.allow = s1, s2, allow
        self.seam = math.radians(seam_deg)  # azimuth of the longitudinal seam, from the outer generatrix

    def cuts(self, k):
        """(tan theta at the start, tan theta at the end) of piece k = 1..n."""
        tt = math.tan(self.theta)
        return (0.0 if k == 1 else tt, 0.0 if k == self.n else tt)

    def centre_length(self, k):
        """Distance between the centres of the start and end cut planes of piece k, plus straight and allowance."""
        tt = math.tan(self.theta)
        if k == 1:
            c = self.R * tt + self.s1
        elif k == self.n:
            c = self.R * tt + self.s2
        else:
            c = 2 * self.R * tt
        return c + self.allow

    def gen_len(self, k, phi):
        """Length of the generatrix of piece k at azimuth phi (0 = outer side, pi = inner side)."""
        ts, te = self.cuts(k)
        return self.centre_length(k) + self.r * math.cos(phi) * (ts + te)

    def edge_curves(self, k, phi):
        """z of the start edge and of the end edge at azimuth phi, z measured along the axis from the start plane centre."""
        ts, te = self.cuts(k)
        c = self.centre_length(k)
        return (-self.r * math.cos(phi) * ts, c + self.r * math.cos(phi) * te)

    def sheet(self, k, m=720):
        """Developed sheet of piece k starting at the seam: (x_i, y_start_i, y_end_i), x = r*phi, i = 0..m."""
        pts = []
        for i in range(m + 1):
            x = 2 * math.pi * self.r * i / m
            phi = self.seam + x / self.r
            ys, ye = self.edge_curves(k, phi)
            pts.append((x, ys, ye))
        return pts


# ---- route B: 3D geometry ---------------------------------------------------------------------------------------
def v_add(a, b): return (a[0] + b[0], a[1] + b[1], a[2] + b[2])
def v_sub(a, b): return (a[0] - b[0], a[1] - b[1], a[2] - b[2])
def v_mul(a, s): return (a[0] * s, a[1] * s, a[2] * s)
def v_dot(a, b): return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
def v_len(a): return math.sqrt(v_dot(a, a))
def v_unit(a): return v_mul(a, 1.0 / v_len(a))


def polyline(b):
    """Centre-line polygon circumscribed about the arc of radius R, bend in the x-y plane, arc centre at the origin.
    The arc starts at (R, 0) heading +y. Returns tangent points/vertices P0..Pn (P0 = arc start, Pn = arc end) and the
    unit directions of the n legs, constructed from the plane geometry, not from the tan() length formulas."""
    R, J, th = b.R, b.J, b.theta
    alpha = 2 * th  # deflection per joint
    verts = [(R, 0.0, 0.0)]
    for j in range(1, J + 1):  # joint vertices lie on the bisector ray of each pair of tangent points at radius R/cos(theta)
        ang = (2 * j - 1) * th
        rad = R / math.cos(th)
        verts.append((rad * math.cos(ang), rad * math.sin(ang), 0.0))
    A = math.radians(b.A)
    verts.append((R * math.cos(A), R * math.sin(A), 0.0))
    dirs = [v_unit(v_sub(verts[i + 1], verts[i])) for i in range(b.n)]
    return verts, dirs, alpha


def seg_frame(b, k):
    """Leg k (1..n): start point, end point, axis direction d, outward (extrados) unit vector e_out, side unit vector e_side.
    Straights s1/s2 extend the first/last leg beyond the tangent points. The allowance is not geometry (3D routes use allow = 0)."""
    verts, dirs, _ = polyline(b)
    d = dirs[k - 1]
    p0, p1 = verts[k - 1], verts[k]
    zhat = (0.0, 0.0, 1.0)
    e_side = zhat
    # outward = away from the arc centre: perpendicular to d in the bend plane, pointing away from the origin
    n1 = (d[1], -d[0], 0.0)
    mid = v_mul(v_add(p0, p1), 0.5)
    e_out = n1 if v_dot(n1, mid) > 0 else v_mul(n1, -1.0)
    ext0 = b.s1 if k == 1 else 0.0
    ext1 = b.s2 if k == b.n else 0.0
    p0 = v_sub(p0, v_mul(d, ext0))
    p1 = v_add(p1, v_mul(d, ext1))
    return p0, p1, d, e_out, e_side


def plane_at_end(b, k, end):
    """(point, unit normal) of the cut plane at the start (end=0) or end (end=1) of leg k: normal is the bisector of the
    two leg directions at a joint; at the free ends it is the leg direction itself."""
    verts, dirs, _ = polyline(b)
    j = k - 1 + end  # vertex index
    if (end == 0 and k == 1) or (end == 1 and k == b.n):
        p = verts[j]
        n = dirs[k - 1]
        if end == 0 and k == 1:
            p = v_sub(p, v_mul(n, b.s1))
        else:
            p = v_add(p, v_mul(n, b.s2))
        return p, n
    nb = v_unit(v_add(dirs[j - 1], dirs[j]))
    return verts[j], nb


def cut_point(b, k, end, phi):
    """3D point where the cylinder of leg k meets its start/end cut plane, at azimuth phi, by a line-plane solve."""
    p0, p1, d, e_out, e_side = seg_frame(b, k)
    base = p0
    ring = v_add(v_mul(e_out, b.r * math.cos(phi)), v_mul(e_side, b.r * math.sin(phi)))
    q, n = plane_at_end(b, k, end)
    # point = base + ring + z*d ; (point - q).n = 0
    z = v_dot(v_sub(q, v_add(base, ring)), n) / v_dot(d, n)
    return v_add(v_add(base, ring), v_mul(d, z)), z


# ---- cases -------------------------------------------------------------------------------------------------------
def route_a_vs_3d(b, tag):
    group = "B 3D cut vs closed form"
    for k in range(1, b.n + 1):
        for i in range(0, 24):
            phi = 2 * math.pi * i / 24 + 0.013
            pe0, z0 = cut_point(b, k, 0, phi)
            pe1, z1 = cut_point(b, k, 1, phi)
            g3 = v_len(v_sub(pe1, pe0))
            check(group, f"{tag} k{k} phi{i} generatrix", g3, b.gen_len(k, phi), 1e-7 * max(1.0, b.R))
    # centre-to-centre length: 3D distance between the two plane centres along the axis
    for k in range(1, b.n + 1):
        p0, p1, d, _, _ = seg_frame(b, k)
        q0, n0 = plane_at_end(b, k, 0)
        q1, n1 = plane_at_end(b, k, 1)
        # centre of each plane on the axis: axis point where the plane meets the axis (phi irrelevant: r = 0 ring)
        z0 = v_dot(v_sub(q0, p0), n0) / v_dot(d, n0)
        z1 = v_dot(v_sub(q1, p0), n1) / v_dot(d, n1)
        check("B 3D cut vs closed form", f"{tag} k{k} centre length", z1 - z0, b.centre_length(k), 1e-7 * max(1.0, b.R))


def shared_ellipse(b, tag):
    group = "C neighbours share the cut ellipse (3D)"
    for k in range(1, b.n):
        for i in range(0, 36):
            phi = 2 * math.pi * i / 36 + 0.021
            pa, _ = cut_point(b, k, 1, phi)
            pb, _ = cut_point(b, k + 1, 0, phi)
            check(group, f"{tag} joint {k} phi{i}", v_len(v_sub(pa, pb)), 0.0, 1e-7 * max(1.0, b.R))


def quadrature_area(b, tag):
    group = "C developed area = circumference x centre length (quadrature)"
    for k in range(1, b.n + 1):
        pts = b.sheet(k, 2000)
        a = 0.0
        for (x0, s0, e0), (x1, s1, e1) in zip(pts, pts[1:]):
            a += 0.5 * ((e0 - s0) + (e1 - s1)) * (x1 - x0)
        check(group, f"{tag} k{k}", a, 2 * math.pi * b.r * b.centre_length(k), 1e-7 * a)
        # closed sheet: both seam edges equal the generatrix at the seam, and the curves close up
        check(group, f"{tag} k{k} seam edge", pts[0][2] - pts[0][1], b.gen_len(k, b.seam), 1e-9 * max(1.0, a))
        check(group, f"{tag} k{k} closes", pts[-1][1] - pts[0][1], 0.0, 1e-9 * max(1.0, abs(pts[0][1])))


def t_to_zero(group="A t->0 limit"):
    for (D, R, A, n) in [(1000.0, 1500.0, 90.0, 3), (800.0, 600.0, 60.0, 3), (300.0, 900.0, 30.0, 2), (1200.0, 1200.0, 90.0, 4)]:
        t = 1e-9
        bs = {name: Bend(D, t, R, A, n, name) for name in ("inner", "mid", "outer")}
        for k in range(1, n + 1):
            for i in range(0, 12):
                phi = 2 * math.pi * i / 12 + 0.3
                ref = bs["mid"].edge_curves(k, phi)
                for name in ("inner", "outer"):
                    got = bs[name].edge_curves(k, phi)
                    check(group, f"D{D} R{R} A{A} n{n} k{k} phi{i} basis {name} == mid (t -> 0)", got[0], ref[0], 1e-6)
                    check(group, f"D{D} R{R} A{A} n{n} k{k} phi{i} basis {name} == mid end", got[1], ref[1], 1e-6)
        # one cut of a straight cylinder of radius D/2 by a plane tilted theta (independent routine): z = -x*tan(theta)
        b = bs["mid"]
        th = b.theta
        r = b.r
        for i in range(0, 16):
            phi = 2 * math.pi * i / 16 + 0.1
            # cylinder axis z, plane through the origin with normal (sin th, 0, cos th) (tilted by th from the cross-section)
            nx, nz = math.sin(th), math.cos(th)
            x = r * math.cos(phi)
            z_plane = -nx * x / nz
            for k in range(2, n):  # middle pieces: start edge uses +tan, end edge uses +tan about the end centre
                ys, ye = b.edge_curves(k, phi)
                check(group, f"cylinder cut theta={math.degrees(th):.3f} k{k} phi{i} start", ys, z_plane, 1e-6)
            ys1, ye1 = b.edge_curves(1, phi)
            check(group, f"first piece end edge theta={math.degrees(th):.3f} phi{i}", ye1 - b.centre_length(1), -z_plane, 1e-6)
        # theta -> 0 (tiny angle): every cut curve is flat
        tiny = Bend(D, 1e-9, R, 1e-9, n, "mid")
        for k in range(1, n + 1):
            ys, ye = tiny.edge_curves(k, 0.7)
            check(group, f"A->0 k{k} flat start", ys, 0.0, 1e-6)
            check(group, f"A->0 k{k} flat end", ye - tiny.centre_length(k), 0.0, 1e-6)


def closed_forms_90():
    g = "B closed forms 90 deg"
    D, t, R = 1016.0, 12.0, 1500.0
    # 3 pieces, 2 joints: theta = 22.5 deg, tan = sqrt(2) - 1
    b = Bend(D, t, R, 90.0, 3, "mid")
    q = math.sqrt(2.0) - 1.0
    r = (D - t) / 2.0
    check(g, "n3 half cut angle deg", math.degrees(b.theta), 22.5, 1e-12)
    check(g, "n3 tan(theta)", math.tan(b.theta), q, 1e-14)
    check(g, "n3 middle centre length 2R(sqrt2-1)", b.centre_length(2), 2 * R * q, 1e-9)
    check(g, "n3 end centre length R(sqrt2-1) (half of middle)", b.centre_length(1), R * q, 1e-9)
    check(g, "n3 end centre length last", b.centre_length(3), R * q, 1e-9)
    check(g, "n3 middle outer generatrix 2(R+r)(sqrt2-1)", b.gen_len(2, 0.0), 2 * (R + r) * q, 1e-9)
    check(g, "n3 middle inner generatrix 2(R-r)(sqrt2-1)", b.gen_len(2, math.pi), 2 * (R - r) * q, 1e-9)
    check(g, "n3 end piece outer generatrix (R+r)(sqrt2-1)", b.gen_len(1, 0.0), (R + r) * q, 1e-9)
    check(g, "n3 end piece inner generatrix (R-r)(sqrt2-1)", b.gen_len(1, math.pi), (R - r) * q, 1e-9)
    check(g, "n3 side generatrix = centre length", b.gen_len(2, math.pi / 2), 2 * R * q, 1e-9)
    check(g, "n3 swing (cut curve amplitude) r(sqrt2-1)", r * math.tan(b.theta), r * q, 1e-9)
    check(g, "n3 sum of centre lengths 4R(sqrt2-1)", sum(b.centre_length(k) for k in range(1, 4)), 4 * R * q, 1e-9)
    check(g, "n3 sheet width = circumference", 2 * math.pi * b.r, math.pi * (D - t), 1e-9)
    check(g, "n3 middle sheet area (closed form C*c)", 2 * math.pi * r * b.centre_length(2), math.pi * (D - t) * 2 * R * q, 1e-6)
    # 2 pieces, 1 joint: theta = 45 deg, tan = 1
    b2 = Bend(D, t, R, 90.0, 2, "mid")
    check(g, "n2 half cut angle deg", math.degrees(b2.theta), 45.0, 1e-12)
    check(g, "n2 end centre length = R", b2.centre_length(1), R, 1e-9)
    check(g, "n2 outer generatrix R+r", b2.gen_len(1, 0.0), R + r, 1e-9)
    check(g, "n2 inner generatrix R-r", b2.gen_len(1, math.pi), R - r, 1e-9)
    # 4 pieces, 3 joints: theta = 15 deg, tan = 2 - sqrt(3)
    b4 = Bend(D, t, R, 90.0, 4, "mid")
    q4 = 2.0 - math.sqrt(3.0)
    check(g, "n4 half cut angle deg", math.degrees(b4.theta), 15.0, 1e-12)
    check(g, "n4 middle centre length 2R(2-sqrt3)", b4.centre_length(2), 2 * R * q4, 1e-9)
    check(g, "n4 sum of centre lengths 6R(2-sqrt3)", sum(b4.centre_length(k) for k in range(1, 5)), 6 * R * q4, 1e-9)
    # basis radii for the same plate
    for basis, want in (("outer", D / 2.0), ("mid", (D - t) / 2.0), ("inner", D / 2.0 - t)):
        check(g, f"radius of basis {basis}", basis_radius(D, t, basis), want, 1e-12)
        bb = Bend(D, t, R, 90.0, 3, basis)
        check(g, f"circumference of basis {basis}", 2 * math.pi * bb.r, 2 * math.pi * want, 1e-9)
    # long bend vs arc: polygon circumscribed about the arc is longer than the arc and tends to it as n grows
    prev = None
    for n in (2, 3, 4, 6, 12, 48):
        bn = Bend(D, t, R, 90.0, n, "mid")
        total = sum(bn.centre_length(k) for k in range(1, n + 1))
        check_true(g, f"polygon n{n} longer than the arc", total > R * math.pi / 2)
        if prev is not None:
            check_true(g, f"polygon n{n} shorter than n-1 pieces", total < prev)
        prev = total
    check(g, "n48 within 0.05 % of the arc", prev / (R * math.pi / 2), 1.0, 5e-4)


def rule_checks():
    g = "D piece-count rule (parameter)"
    for a, n in [(1, 2), (25, 2), (30, 2), (30.0001, 3), (45, 3), (60, 3), (60.0001, 4), (75, 4), (90, 4)]:
        check(g, f"angle {a}", pieces_for_angle(a), n, 0)
    for bad in (0, -5, float("nan"), 90.5, 180):
        try:
            pieces_for_angle(bad)
            check_true(g, f"angle {bad} refused", False)
        except ValueError:
            check_true(g, f"angle {bad} refused", True)
    # the rule is a parameter: another table gives another count
    check(g, "alternative rule 3/4/5 at 50 deg", pieces_for_angle(50, [(45, 3), (67.5, 4), (90, 5)]), 4, 0)
    # a joint count from the rule feeds theta = A/(2*(n-1))
    for a in (20, 45, 80):
        n = pieces_for_angle(a)
        check(g, f"theta for A={a} (n={n})", Bend(500, 8, 700, a, n).theta, math.radians(a) / (2 * (n - 1)), 1e-15)


def main():
    rule_checks()
    t_to_zero()
    closed_forms_90()
    cases = [
        (1016.0, 12.0, 1500.0, 90.0, 3, "mid", 0.0, 0.0, 0.0),
        (1016.0, 12.0, 1500.0, 90.0, 4, "outer", 0.0, 0.0, 0.0),
        (1016.0, 12.0, 1500.0, 90.0, 2, "inner", 0.0, 0.0, 0.0),
        (609.6, 9.0, 914.4, 45.0, 3, "mid", 300.0, 250.0, 0.0),
        (2641.6, 16.0, 2600.0, 90.0, 4, "outer", 300.0, 300.0, 4.0),
        (508.0, 8.0, 400.0, 30.0, 2, "mid", 0.0, 100.0, 0.0),
        (1524.0, 12.0, 1500.0, 60.0, 3, "mid", 500.0, 400.0, 0.0),
        (800.0, 10.0, 640.0, 120.0, 5, "mid", 0.0, 0.0, 0.0),  # above 90 deg: geometry holds even if the count rule stops at 90
        (500.0, 6.0, 750.0, 15.0, 2, "outer", 0.0, 0.0, 2.5),
    ]
    for (D, t, R, A, n, basis, s1, s2, allow) in cases:
        for seam in (180.0, 45.0):
            b = Bend(D, t, R, A, n, basis, s1, s2, allow, seam)
            tag = f"D{D} t{t} R{R} A{A} n{n} {basis} s{s1}/{s2} allow{allow} seam{seam}"
            # the allowance is a fabrication addition, not geometry: the 3D routes use allow = 0, the allowance must be additive
            b0 = Bend(D, t, R, A, n, basis, s1, s2, 0.0, seam)
            route_a_vs_3d(b0, tag)
            shared_ellipse(b0, tag)
            quadrature_area(b, tag)
            for k in range(1, n + 1):
                check("B allowance is additive", f"{tag} k{k}", b.gen_len(k, 0.4) - b0.gen_len(k, 0.4), allow, 1e-9)
    print(f"\nverify_miterbend: {N_OK} OK, {N_NG} NG")
    for g, n in GROUPS.items():
        print(f"  {n:5d}  {g}")
    return 0 if N_NG == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
