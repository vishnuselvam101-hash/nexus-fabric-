"""Work plan task 1.5: the DXF-only mating test. Two plates that the product says form one joint must, from the
two DXF files ALONE, fold back into 3D and meet cut-edge to cut-edge. Nothing here calls the engine's development
code or reads the result's geometry: the fold-back is re-derived from Spec 02 section 4.4 and the conventions the
files state (999 comment block), and the vertex correspondence is SEARCHED, not assumed. This also proves the task
1.3 convention statements suffice to build the part: every number and direction the fold-back needs is stated in
the files, and the one placement the files do not state (the station, a translation along the header axis) is
fitted and then checked against the submitted request.

Derivation (plan rule A9: from Spec 02, never from engine code).

Canonical frame (Spec 02 section 4.4, "Cylinder-on-cylinder, explicit"): header axis = x-axis, contact radius R;
branch contact radius r, axis through (0, delta, 0) with direction (cos beta, 0, sin beta); azimuth phi measured
from +y toward (-sin beta, 0, cos beta). A point of the branch surface at azimuth phi and axial distance s from
(0, delta, 0) is
    P(phi, s) = (s cos beta - r sin phi sin beta,  delta + r cos phi,  s sin beta + r sin phi cos beta).   (4.3)
The saddle is the upper sheet s(phi) of (4.3); the branch end cut lies on it.

Branch fold-back. The branch file states (NF-ANGLE-DATUM) phi = X / R_dev with phi = 0 the seam generator on the
positive-delta side, increasing counter-clockwise seen from the square end toward the header; and (NF-ORIGIN)
Y = 0 at the level of the header axis. So s = Y, measured from the header-axis level, NOT from the station: the
fold-back is P(X / r_b, Y) with r_b the contact radius, and no station enters it. The azimuth direction matches
(4.3): at phi = 0 the offset is +y (the positive-delta side, as stated), and d/dphi at phi = 0 is
r(-sin beta, 0, cos beta), the stated direction of increase.

Header fold-back. Spec 02 gives the opening on the header development as (x, R_dev omega) with
omega = arcsin((delta + r cos phi) / R). The header file states (NF-SEAM-EDGES) that the seam is diametrically
opposite the branch, and (NF-ANGLE-DATUM) that Y = half the circumference is the generator FACING the branch,
with Y increasing toward the positive-delta side. The file's angle is therefore gamma = Y / R_dev = omega + pi:
gamma = pi faces the branch, gamma = 0 is the seam. Put the header cylinder at y^2 + z^2 = R^2 with the branch
"pointing up" = +z (NF-OFFSET-SIGN: positive delta is the observer's LEFT for an observer at the station-0 end
looking toward increasing station with the branch up; that observer's left is +y, and "up" is +z). Then
    P_h = (X, -R sin gamma, -R cos gamma):
at gamma = pi, (X, 0, +R) faces the branch; at gamma = 0, (X, 0, -R) is the seam opposite it; and increasing
gamma moves from +z toward +y (dP/dgamma at pi is (0, +R, 0)), the positive-delta side, exactly as the file
states. Since sin(omega) = sin(gamma - pi) = -sin gamma, the y-coordinate -R sin gamma = R sin omega reproduces
the Spec 02 opening, and x is the station plus the branch x of (4.3).

Contact radii. The files state only DEVELOPED radii (the circumference, and the branch R_dev). The fold-back
needs CONTACT radii. The test therefore requires NF-HEADER-SURFACE = NF-BRANCH-SURFACE = neutral: a neutral
reference surface lies on the contact surface itself (the product's BE-001 rule: neutral demands dev = contact),
so the stated developed radius IS the contact radius. It also requires NF-JOINT = not declared: then the branch
end cut and the header opening are cut on ONE curve of the entered radii (the zero-thickness idealisation the
file states), which is exactly the coincidence tested. A set-on joint cuts the opening at the branch bore, a
different curve, and is outside this test.

Correspondence. The two cut loops are the same intersection curve sampled at the same azimuths (the product
samples both parts with one segment count, RB-004), but the reader does not assume that, nor a start vertex, nor
a direction: it tries both directions x all n cyclic shifts of one loop against the other, fits the single
unknown translation t along the header axis per alignment (t = mean(x_H - x_B); y and z must coincide with no
fit), and takes the alignment of least maximum vertex residual. The fitted t is the station the files do not
state; it is checked against the submitted branch_station (a test input, not an engine output).

Tolerance (a sensitivity bound, not a fitted number). The convention sentences write lengths and angles in mm
and degrees with at most three decimals (DXF_METADATA.md, "Precision of the stated values"), so each true value
lies within +/-0.0005 mm / +/-0.0005 deg of the stated one; the drawn vertices themselves are exact round-trip
decimals. Perturb each stated scalar the fold-back uses (header circumference w_h, branch r_b, delta, beta) by
+/- half a unit of the last place, re-fold, and take the largest vertex displacement; sum the four -> D. Each
coordinate of either fold at the true values differs from the stated-value fold by at most D, so a paired
vertex differs by at most 2D per coordinate; the mean-t fit subtracts a mean that is itself within +/-2D, so
the x residual is at most 4D and the y, z residuals at most 2D: the coincidence residual is at most
sqrt(4^2 + 2^2 + 2^2) D = sqrt(24) D < 5D, and the fitted station differs from the true one by <= 2D < 5D.
The bound used is 5D + 1e-9 x size (the floor covers binary64 arithmetic and the second-order terms of the
perturbation, <= ~1e-7 mm here). Every planted fault moves the cut edges by >= 1.7 mm, far beyond this bound.

Faults. Geometry faults are planted in the DXF-derived geometry as the validator sees it (the seeded_faults
idiom); stated-value faults are planted by rewriting the 999 comment lines of the DXF text and re-reading them,
so the file itself carries the wrong statement. Each must make the mating check FAIL.

  python3 -m checks.mating --url http://127.0.0.1:8802   (from validation/, or via run_all.py stage 0)
"""
import argparse
import math
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import numpy as np
from checks.client import Client, USER, PASSWORD
from checks.product import Job, read_dxf_minimal

TAU = 2.0 * math.pi
# neutral surfaces (dev = contact, BE-001), joint not declared: both cuts on one curve of the entered radii
REQ = {'family': 'header_with_branch', 'header_radius': 150, 'header_dev_radius': 150, 'header_length': 1200,
       'branch_station': 500, 'branch_radius': 100, 'branch_dev_radius': 100, 'beta_deg': 60, 'delta': 20,
       's_end': 400, 'surface_reference': 'neutral'}
# the same joint with values that are NOT exact at three decimals: every stated scalar the fold-back uses is
# rounded by the writing rule (beta 60.0004 -> "60", delta 20.0004 -> "20", r 100.0004 -> R_dev "100",
# w_h 942.4777960... -> "942.478"), so the sensitivity bound is exercised on all four channels at once
REQ_ROUNDED = dict(REQ, beta_deg=60.0004, delta=20.0004, branch_radius=100.0004, branch_dev_radius=100.0004)
HALF_ULP = 5e-4          # half a unit of the last written place (three decimals), mm and deg alike


def mm_fmt(v):
    """A value as the convention sentences write it: at most three decimals, no trailing zeros."""
    s = f'{v:.3f}'.rstrip('0').rstrip('.')
    return '0' if s == '-0' else s


def _num(pattern, text, what):
    m = re.search(pattern, text)
    if not m:
        raise ValueError(f'convention statement missing or unreadable: {what}')
    return float(m.group(1))


def _need(phrase, text, what):
    if phrase not in text:
        raise ValueError(f'convention statement does not state {what}')


def stated_values(hcomments, bcomments):
    """Every number and direction the fold-back needs, read from the 999 comment blocks of the two files.
    Raises (fail closed) when a load-bearing statement is missing, unreadable or contradictory."""
    def keys(comments):
        d = {}
        for ln in comments:
            if ln.startswith('NF-') and ': ' in ln:
                k, v = ln.split(': ', 1)
                if k in d:
                    raise ValueError(f'key stated twice: {k}')
                d[k] = v
        return d

    h, b = keys(hcomments), keys(bcomments)
    for d, side in ((h, 'header'), (b, 'branch')):
        for k in ('NF-UNITS', 'NF-VIEW-SIDE', 'NF-PLATE-AXES', 'NF-SEAM-EDGES', 'NF-ANGLE-DATUM', 'NF-ORIGIN',
                  'NF-END-EDGES', 'NF-ROLLING-AXIS', 'NF-OFFSET-DELTA', 'NF-OFFSET-SIGN', 'NF-BRANCH-ANGLE',
                  'NF-JOINT', 'NF-HEADER-SURFACE', 'NF-BRANCH-SURFACE'):
            if k not in d:
                raise ValueError(f'{side} file does not state {k}')
        if d['NF-UNITS'] != 'mm':
            raise ValueError(f'{side} file states units {d["NF-UNITS"]!r}, not mm')
        if d['NF-VIEW-SIDE'] != 'outside':
            raise ValueError(f'{side} file states marking side {d["NF-VIEW-SIDE"]!r}; the fold-back is derived for the outside view')
        if d['NF-JOINT'] != 'not declared':
            raise ValueError(f'{side} file states joint {d["NF-JOINT"]!r}; the one-curve coincidence needs the joint not declared')
        if d['NF-HEADER-SURFACE'] != 'neutral' or d['NF-BRANCH-SURFACE'] != 'neutral':
            raise ValueError(f'{side} file states surfaces {d["NF-HEADER-SURFACE"]!r}/{d["NF-BRANCH-SURFACE"]!r}; '
                             'the contact radii are the stated developed radii only for neutral surfaces')
        _need('DISPLACED TO THE LEFT OF THE HEADER AXIS', d['NF-OFFSET-SIGN'], 'the positive-delta direction (observer\'s left)')
        _need('BRANCH POINTING UP', d['NF-OFFSET-SIGN'], 'the branch-up direction of the canonical frame')
    # header: X = station, Y = around; seam opposite the branch; Y = w/2 faces it; Y increases toward +delta
    w_h = _num(r'length [0-9.]+ mm; Y = around the header, developed circumference ([0-9.]+) mm', h['NF-PLATE-AXES'], 'header circumference')
    L = _num(r'X = along the header axis \(station\), length ([0-9.]+) mm', h['NF-PLATE-AXES'], 'header length')
    w_h2 = _num(r'at Y = 0 and Y = ([0-9.]+) mm', h['NF-SEAM-EDGES'], 'header seam-edge circumference')
    _need('the seam is diametrically opposite the branch', h['NF-SEAM-EDGES'], 'the seam opposite the branch (gamma = 0 at -z)')
    half = _num(r'Y = ([0-9.]+) mm \(half the circumference\) is the generator facing the branch', h['NF-ANGLE-DATUM'], 'the branch-facing generator')
    _need('Y increases toward the positive-delta side', h['NF-ANGLE-DATUM'], 'the direction of increasing Y')
    _need('station increases with X', h['NF-END-EDGES'], 'the direction of increasing station')
    _need('the seam at the station-0 end', h['NF-ORIGIN'], 'the plate origin')
    _need('the drawn face becomes the outside of the header', h['NF-ROLLING-AXIS'], 'the rolled face')
    # branch: X = around, Y = along the axis from the header-axis level; phi = 0 the +delta seam generator
    w_b = _num(r'X = around the branch, developed circumference ([0-9.]+) mm; Y = along the branch axis', b['NF-PLATE-AXES'], 'branch circumference')
    w_b2 = _num(r'at X = 0 and X = ([0-9.]+) mm', b['NF-SEAM-EDGES'], 'branch seam-edge circumference')
    r_b = _num(r'phi = X / R_dev, R_dev = ([0-9.]+) mm', b['NF-ANGLE-DATUM'], 'branch developed radius')
    _need('phi = 0 is the seam (generator on the positive-delta side)', b['NF-ANGLE-DATUM'], 'the azimuth datum')
    _need('counter-clockwise seen from the square end toward the header', b['NF-ANGLE-DATUM'], 'the direction of increasing phi')
    _need('Y = 0 is the level of the header axis', b['NF-ORIGIN'], 'the axial origin (s = Y, station-independent)')
    _need('the drawn face becomes the outside of the branch', b['NF-ROLLING-AXIS'], 'the rolled face')
    # the placement both files must agree on
    delta_h = _num(r'delta = (-?[0-9.]+) mm', h['NF-OFFSET-DELTA'], 'header delta')
    delta_b = _num(r'delta = (-?[0-9.]+) mm', b['NF-OFFSET-DELTA'], 'branch delta')
    beta_h = _num(r'beta = (-?[0-9.]+) deg', h['NF-BRANCH-ANGLE'], 'header beta')
    beta_b = _num(r'beta = (-?[0-9.]+) deg', b['NF-BRANCH-ANGLE'], 'branch beta')
    if abs(w_h - w_h2) > 1e-3 or abs(w_b - w_b2) > 1e-3:
        raise ValueError('the circumference is stated inconsistently within a file')
    if abs(half - w_h / 2) > 1e-3:
        raise ValueError('the branch-facing generator is not at half the stated circumference')
    if abs(r_b - w_b / TAU) > 1e-3:
        raise ValueError('the branch R_dev and circumference are inconsistent')
    if abs(delta_h - delta_b) > 1e-9 or abs(beta_h - beta_b) > 1e-9:
        raise ValueError('the two files state different delta or beta')
    return {'w_h': w_h, 'L': L, 'w_b': w_b, 'r_b': r_b, 'delta': delta_h, 'beta_deg': beta_h}


def fold_header(H, st):
    """Plate (X, Y) -> canonical 3D: gamma = Y / R_dev, seam (gamma = 0) at -z, branch-facing generator at +z."""
    R = st['w_h'] / TAU
    g = H[:, 1] / R
    return np.stack([H[:, 0], -R * np.sin(g), -R * np.cos(g)], axis=1)


def fold_branch(B, st):
    """Plate (X, Y) -> canonical 3D by Spec 02 (4.3): phi = X / r_b, s = Y from the header-axis level."""
    r, d = st['r_b'], st['delta']
    beta = math.radians(st['beta_deg'])
    cb, sb = math.cos(beta), math.sin(beta)
    phi = B[:, 0] / r
    s = B[:, 1]
    sp, cp = np.sin(phi), np.cos(phi)
    return np.stack([s * cb - r * sp * sb, d + r * cp, s * sb + r * sp * cb], axis=1)


def best_alignment(H3, B3):
    """Least maximum vertex residual over both directions x all n cyclic shifts, fitting only the translation
    along the header axis (t = mean(x_H - x_B)). Returns (residual, t). Assumes no correspondence."""
    n = len(H3)
    i = np.arange(n)
    best_res, best_t = float('inf'), float('nan')
    for direction in (1, -1):
        Bf = B3 if direction == 1 else B3[::-1]
        for k0 in range(0, n, 128):                       # chunked: n^2 vertices would be large for a fine loop
            K = np.arange(k0, min(k0 + 128, n))
            A = Bf[(K[:, None] + i[None, :]) % n]         # (shifts, n, 3)
            t = (H3[None, :, 0] - A[:, :, 0]).mean(axis=1)
            R = H3[None] - A
            R[:, :, 0] -= t[:, None]
            m = np.linalg.norm(R, axis=2).max(axis=1)
            j = int(m.argmin())
            if m[j] < best_res:
                best_res, best_t = float(m[j]), float(t[j])
    return best_res, best_t


def mate_tolerance(H, B, st):
    """5D + 1e-9 x size, D the summed half-ulp sensitivity of the two fold-backs (module docstring)."""
    H0, B0 = fold_header(H, st), fold_branch(B, st)
    D = 0.0
    for key in ('w_h', 'r_b', 'delta', 'beta_deg'):
        worst = 0.0
        for sign in (1.0, -1.0):
            st2 = dict(st)
            st2[key] = st[key] + sign * HALF_ULP
            dh = float(np.abs(fold_header(H, st2) - H0).max())
            db = float(np.abs(fold_branch(B, st2) - B0).max())
            worst = max(worst, dh, db)
        D += worst
    size = max(1.0, float(np.abs(H).max()), float(np.abs(B).max()))
    return 5.0 * D + 1e-9 * size


def judge(H, B, st, station):
    """The mating verdict on one pair of cut loops: (ok, residual, fitted station, tolerance)."""
    H3, B3 = fold_header(H, st), fold_branch(B, st)
    res, t = best_alignment(H3, B3)
    tol = mate_tolerance(H, B, st)
    return (res <= tol and abs(t - station) <= tol), res, t, tol


def cut_loops(hpart, bpart, st):
    """The two cut loops from the DXF entities, closure vertices dropped, with the closure itself checked."""
    hp = hpart.curve('branch_opening').pts
    bp = bpart.curve('saddle_cut').pts
    if len(hp) != len(bp):
        raise ValueError(f'the cut loops have different vertex counts ({len(hp)} vs {len(bp)})')
    if np.abs(hp[-1] - hp[0]).max() > 1e-9:
        raise ValueError('the header opening does not close on its first vertex')
    if abs(bp[-1][0] - st['w_b']) > 1e-3 or abs(bp[-1][1] - bp[0][1]) > 1e-9 or abs(bp[0][0]) > 1e-9:
        raise ValueError('the branch saddle does not close on the seam ruling (X = 0 and X = w_b, same Y)')
    return hp[:-1].copy(), bp[:-1].copy()


def tamper_999(text, key, pattern, repl):
    """Rewrite one stated value in the 999 comment lines of a DXF text (the statement the mating reader reads)."""
    lines = text.split('\n')
    hit = False
    for i in range(len(lines) - 1):
        if lines[i].strip() == '999' and lines[i + 1].startswith(key + ': '):
            new = re.sub(pattern, repl, lines[i + 1], count=1)
            hit |= new != lines[i + 1]
            lines[i + 1] = new
    if not hit:
        raise ValueError(f'could not plant the fault: no 999 line {key} matching {pattern}')
    return '\n'.join(lines)


def run(cl):
    """Submit the neutral one-curve joint, require the two DXF files to mate, and require every planted fault to
    FAIL. Returns (ok, lines) in the seeded_faults style; run_all.py wires it into stage 0."""
    lines, ok = [], True
    ctx = None
    for label, req in (('exact stated values', REQ), ('rounded stated values', REQ_ROUNDED)):
        j = Job(cl, cl.calculate(req))
        if not j.accepted or 'header' not in j.parts or 'branch' not in j.parts:
            ok = False
            lines.append(f'FAIL mating ({label}): the joint was not accepted with both files: {j.refusal_text()[:160]}')
            continue
        try:
            st = stated_values(j.parts['header'].meta_min['comment'], j.parts['branch'].meta_min['comment'])
            H, B = cut_loops(j.parts['header'], j.parts['branch'], st)
            good, res, t, tol = judge(H, B, st, req['branch_station'])
        except Exception as ex:
            ok = False
            lines.append(f'FAIL mating ({label}): {type(ex).__name__}: {ex}')
            continue
        ok &= good
        lines.append(('ok  ' if good else 'FAIL') + f' mating ({label}): {len(H)} vertices, best of {2 * len(H)} alignments: '
                     f'residual {res:.3e} mm <= tol {tol:.3e} mm; fitted station {t:.6f} mm (submitted {req["branch_station"]})'
                     + ('' if good else '  -- the cut edges do not coincide'))
        if label == 'exact stated values':
            ctx = (j, H, B, st, req['branch_station'])
    if ctx is None:
        return ok, lines
    j, H, B, st, station = ctx
    hc, bc = j.parts['header'].meta_min['comment'], j.parts['branch'].meta_min['comment']

    def report(name, caught, res, tol):
        nonlocal ok
        ok &= caught
        lines.append(('ok  ' if caught else 'MISS') + f' mating fault: {name}: '
                     + (f'residual {res:.3e} mm > tol {tol:.3e} mm (FAIL as required)' if caught else f'NOT detected (residual {res:.3e} mm, tol {tol:.3e} mm)'))

    # geometry faults, planted in the DXF-derived geometry (the seeded_faults idiom)
    Bm = B.copy(); Bm[:, 0] = st['w_b'] - Bm[:, 0]                     # saddle mirrored: drawn from the other face
    good, res, t, tol = judge(H, Bm, st, station); report('branch saddle mirrored (X -> w_b - X)', not good, res, tol)
    c = H.mean(axis=0); Hs = c + 1.02 * (H - c)                        # opening scaled about its centroid
    good, res, t, tol = judge(Hs, B, st, station); report('header opening scaled x1.02 about its centroid', not good, res, tol)
    By = B + [0.0, 2.0]                                                # saddle lifted 2 mm along the branch axis
    good, res, t, tol = judge(H, By, st, station); report('branch saddle shifted +2 mm along the branch axis', not good, res, tol)

    # stated-value faults, planted by rewriting the 999 lines of the DXF text and re-reading them
    def stated_fault(name, htext, btext):
        nonlocal ok
        try:
            st2 = stated_values(read_dxf_minimal(htext)[1]['comment'], read_dxf_minimal(btext)[1]['comment'])
            good, res, t, tol = judge(H, B, st2, station)
        except Exception as ex:
            ok = False
            lines.append(f'FAIL mating fault: {name}: could not plant: {ex}')
            return
        report(name, not good, res, tol)

    ht = j.parts['header'].dxf_text
    bt = j.parts['branch'].dxf_text
    b2 = mm_fmt(st['beta_deg'] + 2.0)
    stated_fault('stated beta +2 deg (both files)',
                 tamper_999(ht, 'NF-BRANCH-ANGLE', r'beta = -?[0-9.]+ deg', f'beta = {b2} deg'),
                 tamper_999(bt, 'NF-BRANCH-ANGLE', r'beta = -?[0-9.]+ deg', f'beta = {b2} deg'))
    d3 = mm_fmt(st['delta'] + 3.0)
    stated_fault('stated delta +3 mm (both files)',
                 tamper_999(ht, 'NF-OFFSET-DELTA', r'delta = -?[0-9.]+ mm', f'delta = {d3} mm'),
                 tamper_999(bt, 'NF-OFFSET-DELTA', r'delta = -?[0-9.]+ mm', f'delta = {d3} mm'))
    w1 = mm_fmt(st['w_h'] * 1.01)
    ht2 = tamper_999(ht, 'NF-PLATE-AXES', r'developed circumference [0-9.]+ mm', f'developed circumference {w1} mm')
    ht2 = tamper_999(ht2, 'NF-SEAM-EDGES', r'at Y = 0 and Y = [0-9.]+ mm', f'at Y = 0 and Y = {w1} mm')
    # the half-circumference datum too, so the file stays self-consistent: a coherent lie, not a detectable typo
    ht2 = tamper_999(ht2, 'NF-ANGLE-DATUM', r'Y = [0-9.]+ mm \(half the circumference\)',
                     f'Y = {mm_fmt(st["w_h"] * 1.01 / 2)} mm (half the circumference)')
    stated_fault('stated header circumference +1 %', ht2, bt)
    return ok, lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--url', default='http://127.0.0.1:8802')
    ap.add_argument('--user', default=USER)
    ap.add_argument('--password', default=PASSWORD)
    ap.add_argument('--setup-token', default=os.environ.get('NF_SETUP_TOKEN'))
    a = ap.parse_args()
    ok, lines = run(Client(a.url).connect(a.user, a.password, a.setup_token))
    print('\n'.join(lines))
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()