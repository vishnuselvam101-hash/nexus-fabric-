"""Independent accept/refuse oracle for arbitrary requests.

'valid'        geometry is well defined and inside the product's DECLARED supported range -> must be accepted
'invalid'      no such part exists (non-positive size, branch not fully on the header, opening off the plate ...) -> must be refused
'unsupported'  a part exists but lies in a range the product documents as not implemented
               (docs/backend/REFUSAL_CODES.md, engine 0.4.2, review B2-005): conical apex radius above 1e8 mm,
               equal radii with offset, a developed branch longer than 1e8 mm or beta outside (0, 180) deg,
               tangent/near-tangent saddles (|delta|+r >= R(1-1e-6)), a conical seam gap at the small end of at most
               1e-9 mm (the closure resolution of the profile check)  -> must be refused
               (Until engine 0.4.1 the rules were sin alpha < 1e-3, sin beta <= 0.17 and 99.9 % of a turn; the
               mathematics never required them: a cone is exact for every sin alpha > 0, the developed angle of a cone is
               below a full turn, and the saddle s(phi) is exact for every 0 < beta < 180.)
'edge'         within rounding distance of a boundary: either outcome is acceptable, an accepted result must still be correct.
               Also (engine 0.4.3, work plan task 1.1) a branch whose angle puts the product's own verification in the regime
               where it is not guaranteed to confirm a correct pattern (`branch_conditioning` below, ratio >= 1): the product
               then accepts a pattern its verifier confirms and refuses with `branch_angle_not_verifiable` one it cannot; both
               are allowed, an accepted result must still be correct (run_all.py: in this regime a refusal counts only
               with that code and never when the product's own gates rejected a computed pattern). A refusal with that
               code at a ratio below 1 is a wrong refusal. The ratio mirrors the engine's tolerance definition (RC-28).
"""
import math
import numpy as np
from reference import families as RF

SREF = ('outside', 'inside', 'neutral', 'unknown')
LENGTHS = ['r_dev', 'length', 'r1', 'r2', 'h', 'e', 'header_radius', 'header_dev_radius', 'header_length', 'branch_station', 'branch_radius', 'branch_dev_radius', 'delta', 's_end']
SIGNED = ('e', 'branch_station', 'delta')
APEX_LIMIT = 1e8            # developed-size limit of the conical and branch families, mm (= the magnitude limit)
SEAM_RESOLUTION = 1e-9      # mm: closure resolution of the profile check (G11)
# This is the REFUSAL floor only (a seam gap at or below it is refused). The floor below which the profile gate G11 cannot say PASS is larger
# for big parts (cone: max(1e-9, 2 eps_num)) and is derived from the product's reported eps_num in checks/compare.py seam_resolution_check, not here.
CHORD_LIMIT = 0.01          # mm: chord-error limit of every polyline (TOLERANCE_POLICY section 5, implementation limit)
CHORD_MAX_SEGMENTS = 46080  # the largest number of segments of one polyline (the cap behind the ~4.3e6 mm bound)


def _chord_sag(vertices, mids):
    """Largest distance from an exact midpoint of a segment's arc to the chord of its two vertices (mm)."""
    v0, v1 = vertices[:-1], vertices[1:]
    d = v1 - v0
    cross = d[:, 0] * (mids[:, 1] - v0[:, 1]) - d[:, 1] * (mids[:, 0] - v0[:, 0])
    return float(np.max(np.abs(cross) / np.maximum(np.hypot(d[:, 0], d[:, 1]), 1e-300)))


def chord_sag_eccentric(r1, r2, h, e, n=CHORD_MAX_SEGMENTS):
    """Chord error of the large-end polyline of an eccentric reducer drawn with n uniform segments (reference)."""
    a, H = e * r1 / (r1 - r2), h * r1 / (r1 - r2)
    th = np.arange(4 * n + 1) * (2 * np.pi / (4 * n))
    rho2 = (r1 - a) ** 2 + H * H + 4 * a * r1 * np.sin(th / 2) ** 2
    f = r1 * np.hypot(H, r1 - a * np.cos(th)) / rho2
    psi = np.zeros(2 * n + 1)         # at every second node: the vertices (j = 4i) and the midpoints (j = 4i + 2)
    step = 2 * np.pi / (4 * n)
    psi[1:] = np.cumsum(step / 3 * (f[0:-1:2][: 2 * n] + 4 * f[1::2][: 2 * n] + f[2::2][: 2 * n]))
    rho = np.sqrt(rho2)[::2]
    P = np.stack([rho * np.cos(psi), rho * np.sin(psi)], axis=1)
    return _chord_sag(P[::2], P[1::2][:n]), float(psi[-1])


# ---------------------------------------------------------------------------------------------------- task 1.1
# The product's verification of a branch compares its pattern with an independently, numerically intersected 3D curve
# (a solid joint in a tilted frame: each branch ruling intersected with the header cylinder by a root search; the
# azimuth of a point recovered by atan2). Each such check has the tolerance agree + 4 * bound, where `bound` is the
# product's stated a-priori error bound of the compared vertices; it is guaranteed to confirm a correct pattern only if
# its own rounding F stays within agree + 3 * bound. The closed forms below are those of docs/backend/TOLERANCE_POLICY.md
# section 7 (first-order rounding analysis, u = 2^-53). They MIRROR the product's tolerance definition (develop.rs
# saddle_error_bound / opening_error_bound and the verify.rs bounds and constants, as the other limits of this oracle
# mirror the engine's, review C RC-28): this classification is not independent evidence; the geometry of every accepted
# case is still compared with the independent reference.
#   * a root of g(s) = |b + dp s| - R has slope sin(beta) cos(omega) >= sin(beta) root/R, root = sqrt(R^2 - (|delta| + r)^2);
#     the direction across the header axis carries 3u absolute, |s| <= S = (R + r |cos beta|)/sin(beta): the error of the
#     root grows as u S / sin(beta), i.e. 1/sin^2(beta), near 0 and near 180 degrees alike;
#   * the azimuth recovered from a vector of length up to S + r errs by its rounding / r and moves along the curve by
#     |C'| <= r + D, D = r (1 + y/root)/sin(beta).
# The product's bound itself carries the rounding of beta = fl(beta_deg pi/180), 2u beta |ds/dbeta|, which near 180
# degrees (beta ~ pi) grows as 1/sin^2(beta) too: that is why the regime is reached at the shallow end and, for ordinary
# proportions, not at the obtuse end before the 1e8 mm size limit.
U = 2.0 ** -53
TRIG = 2 * U
TAU = 2 * math.pi
FRAME_DEFECT = U        # defect of the stored constants of the tilted frame from orthonormal (exact evaluation: <= 1.0 u)
FRAME_ORIGIN = 0.87     # |frame origin| / scale, sqrt(0.74) rounded up


def _agree(size):
    return 1e-6 * size if 0 < size < 1 else 1e-6


def _terms(R, r, beta, delta):
    sb, cb = abs(math.sin(beta)), abs(math.cos(beta))
    y = abs(delta) + r
    root = math.sqrt(R * R - y * y)
    return sb, cb, y, root, (R + r * cb) / sb, r * (1 + y / root) / sb


def _engine_bounds(R, r, beta, delta, r_dev, R_dev, station):
    """The product's stated a-priori error bounds of the saddle and opening vertices (TOLERANCE_POLICY section 6/7)."""
    sb, cb, y, root, _, _ = _terms(R, r, beta, delta)
    dsdb = r + (R + r) * cb / (sb * sb)
    s_err = ((2 * U * R * R + R * r * (TRIG + U)) / root + 1.5 * U * R + r * (2 * TRIG + 2 * U) + (R + r) * (TRIG + 2 * U)) / sb + dsdb * 2 * U * abs(beta)
    saddle = s_err + 2 * U * TAU * r_dev + 4 * U * TAU * r / sb
    s_abs, d_phi = (R + r) / sb, 4 * U * TAU
    bx = (s_err * cb + 2 * U * abs(beta) * (dsdb * cb + (R + r) + r) + s_abs * cb * (TRIG + U) + r * (2 * TRIG + 2 * U)
          + U * (s_abs * cb + r) + U * (abs(station) + s_abs * cb + r) + (s_abs * cb + r) * d_phi)
    d_omega = (r * (TRIG + U) + U * y + U * R) / root + TRIG * 0.5 * math.pi
    opening = bx + R_dev * (d_omega + 1.3e-16 + 3 * U * math.pi) + R_dev * d_phi
    return saddle, opening


def _root_bound(R, r, beta, delta, scale, station):
    sb, _, y, root, S, _ = _terms(R, r, beta, delta)
    e_b = U * (2 * FRAME_ORIGIN * scale + 6 * abs(station) + 6 * abs(delta) + 15 * r)
    e_f = U * (5 * R + y)
    return (3 * U * S + e_b + e_f) / (sb * root / R) + 8 * U * max(S, R)


def _point_rounding(R, r, delta, scale, station, S):
    return U * (3 * FRAME_ORIGIN * scale + 5 * abs(station) + 5 * abs(delta) + 13 * r + 4 * S)


def branch_conditioning(req):
    """Largest ratio F / (agree + 3 bound) over the 3D-intersection checks the product runs on this request."""
    R, r, delta = req['header_radius'], req['branch_radius'], req['delta']
    beta = math.radians(req['beta_deg'])
    r_dev, s_end = req['branch_dev_radius'], req['s_end']
    sb, cb, y, root, S, D = _terms(R, r, beta, delta)
    asm = req['family'] == 'header_with_branch'
    R_dev, L, st = (req['header_dev_radius'], req['header_length'], req['branch_station']) if asm else (R, 0.0, 0.0)
    saddle, opening = _engine_bounds(R, r, beta, delta, r_dev, R_dev, st)
    ratios = [(_root_bound(R, r, beta, delta, R, 0.0) + 2 * U * TAU * D) / (_agree(max(R, s_end)) + 3 * saddle)]
    if asm:
        q = abs(st) + S + abs(delta) + r
        e_pt = _root_bound(R, r, beta, delta, R, st) + _point_rounding(R, r, delta, R, st, S)
        dx = e_pt + 4 * U * q
        d_omega = math.sqrt(2) * (e_pt + (4 * U + FRAME_DEFECT) * q) / R + 2 * U * TAU
        f_open = dx + R_dev * (d_omega + 3 * U * TAU + 1.3e-16) + 3 * U * TAU * (D + r + R_dev * r / root)
        ratios.append(f_open / (_agree(max(R, L)) + 3 * opening))
        qj = S + r
        e_open = U * (FRAME_ORIGIN * R + 3 * (abs(st) + S + r) + 5 * R) + R * (3 * math.pi * U + 4 * U)
        e_bp = _point_rounding(R, r, delta, R, st, S)
        e_p = max(e_open, e_bp + 2 * U * TAU * r)
        e_o = U * (FRAME_ORIGIN * R + 3 * abs(st) + 3 * abs(delta))
        da = e_p + e_o + qj * (7 * U + 2 * FRAME_DEFECT)
        db = e_p + e_o + qj * (4 * U + 1.5 * FRAME_DEFECT)
        f_joint = e_p + _root_bound(R, r, beta, delta, R, st) + e_bp + (r + D) * (math.hypot(da, db) / r + 2 * U * TAU)
        ratios.append(f_joint / (_agree(max(R, s_end)) + 3 * (opening + saddle)))
    return max(ratios)


def chord_sag_branch(R, r, r_dev, beta_deg, delta, n=CHORD_MAX_SEGMENTS):
    """Chord error of the saddle polyline of a branch drawn with n uniform segments in the angle (reference)."""
    bb = math.radians(beta_deg)
    ph = np.arange(2 * n + 1) * (np.pi / n)
    s = (np.sqrt(np.maximum(R * R - (delta + r * np.cos(ph)) ** 2, 0.0)) - r * np.sin(ph) * math.cos(bb)) / math.sin(bb)
    P = np.stack([r_dev * ph, s], axis=1)
    return _chord_sag(P[::2], P[1::2][:n])


def _chord_class(sag, what):
    if abs(sag - CHORD_LIMIT) <= 0.03 * CHORD_LIMIT:
        return 'edge', f'{what} at the chord-error cap'
    if sag > CHORD_LIMIT:
        return 'unsupported', f'{what} cannot be drawn within 0.01 mm with 46 080 segments'
    return None


def sref_consistent(sref, contact, dev):
    """Spec 02 6.1: the development surface lies inside the wall, r_inside <= r_dev <= r_outside."""
    same = abs(dev - contact) <= 1e-12 * max(abs(dev), abs(contact))
    if sref == 'outside':
        return same or dev < contact
    if sref == 'inside':
        return same or dev > contact
    if sref == 'neutral':
        return same
    return True


def _pos(req, names):
    for n in names:
        v = req.get(n)
        if not isinstance(v, (int, float)) or isinstance(v, bool) or not math.isfinite(v) or v <= 0:
            return n
    return None


def classify(req):
    fam = req.get('family')
    for k, v in req.items():
        if k not in ('family', 'surface_reference') and (not isinstance(v, (int, float)) or isinstance(v, bool) or not math.isfinite(v)):
            return 'invalid', f'{k} is not a finite number'
    # declared implementation limit of the build (contract since engine 0.2.0): magnitudes within 1e-6 .. 1e8 mm
    for k in LENGTHS:
        if k in req:
            x = abs(req[k])
            if x > 1e8 or (0 < req[k] < 1e-6 and k not in SIGNED):
                return 'unsupported', f'{k} outside the declared magnitude range 1e-6 .. 1e8 mm'
            if abs(x - 1e8) < 1e-3 or (k not in SIGNED and abs(x - 1e-6) < 1e-15):
                return 'edge', 'at the magnitude limit'
    if fam == 'cylinder_shell':
        bad = _pos(req, ['r_dev', 'length'])
        return ('invalid', f'{bad} <= 0') if bad else ('valid', '')
    if fam in ('cone_frustum', 'eccentric_reducer'):
        bad = _pos(req, ['r1', 'r2', 'h'])
        if bad:
            return 'invalid', f'{bad} <= 0'
        r1, r2, h = req['r1'], req['r2'], req['h']
        e = req.get('e', 0.0)
        if e < 0:
            return 'invalid', 'e < 0'
        if r1 < r2:
            return 'invalid', 'r1 < r2 (large end first)'
        if r1 == r2:
            return 'unsupported', 'equal radii'
        d = r1 - r2
        hyp = math.hypot(h, d)
        sa = d / hyp
        # 1 - sin(alpha) without cancellation: (hyp - d) / hyp = h^2 / (hyp (hyp + d))
        one_minus_sa = h * h / (hyp * (hyp + d))
        if fam == 'cone_frustum':
            l1 = r1 / sa
            if abs(l1 - APEX_LIMIT) <= 1e-6 * APEX_LIMIT:
                return 'edge', 'at the apex-radius limit'
            if l1 > APEX_LIMIT:
                return 'unsupported', 'apex radius above 1e8 mm'
            # seam gap at the small end, l2 (2 pi - Phi), Phi = 2 pi sin(alpha)
            gap = (r2 / sa) * 2 * math.pi * one_minus_sa
            band = 1e-3 * SEAM_RESOLUTION + 4e-15 * (r2 / sa)       # rounding of the engine's sector angle times l2
            if abs(gap - SEAM_RESOLUTION) <= band:
                return 'edge', 'at the seam-closure limit'
            if gap < SEAM_RESOLUTION:
                return 'unsupported', 'seam gap at the small end below the 1e-9 mm closure resolution'
        if fam == 'eccentric_reducer':
            a, H = e * r1 / (r1 - r2), h * r1 / (r1 - r2)
            rho_min = math.hypot(r1 - a, H)
            rho_max = math.sqrt((r1 - a) ** 2 + H * H + 4 * a * r1)
            if abs(rho_max - APEX_LIMIT) <= 1e-6 * APEX_LIMIT:
                return 'edge', 'at the apex-radius limit'
            if rho_max > APEX_LIMIT:
                return 'unsupported', 'apex radius above 1e8 mm'
            # the chord-error cap (a polyline of at most 46 080 segments within 0.01 mm); only evaluated where it can bind
            if rho_max * (2 * np.pi / CHORD_MAX_SEGMENTS) ** 2 / 8 > 1e-4:
                sag, _ = chord_sag_eccentric(r1, r2, h, e)
                hit = _chord_class(sag, 'eccentric reducer')
                if hit:
                    return hit
            th = (np.arange(4000) + 0.5) * 2 * np.pi / 4000
            psi = float(np.sum(r1 * np.sqrt(H * H + (r1 - a * np.cos(th)) ** 2) / (r1 * r1 - 2 * a * r1 * np.cos(th) + a * a + H * H)) * 2 * np.pi / 4000)
            if psi > 2 * np.pi * (1 - 1e-2):
                # near a full turn: the defect 2 pi - psi needs more digits than binary64 gives; use the 30-digit tier
                import mpmath as mp
                from reference import hp
                with mp.workdps(30):
                    c = hp.Cone(r1, r2, h, e)
                    defect = 2 * mp.pi - mp.quad(c.dpsi, mp.linspace(0, 2 * mp.pi, 65))
                gap = float(rho_min * (r2 / r1) * defect)
                band = 1e-3 * SEAM_RESOLUTION + 4e-15 * rho_min * (r2 / r1) + 4e-13 * rho_min * (r2 / r1) * psi / (2 * np.pi)
                if abs(gap - SEAM_RESOLUTION) <= band:
                    return 'edge', 'at the seam-closure limit'
                if gap < SEAM_RESOLUTION:
                    return 'unsupported', 'seam gap at the small end below the 1e-9 mm closure resolution'
        return 'valid', ''
    if fam in ('branch_stub', 'header_with_branch'):
        names = ['header_radius', 'branch_radius', 'branch_dev_radius', 's_end'] + (['header_dev_radius', 'header_length'] if fam == 'header_with_branch' else [])
        bad = _pos(req, names)
        if bad:
            return 'invalid', f'{bad} <= 0'
        if fam == 'header_with_branch' and req.get('surface_reference') not in SREF:
            return 'invalid', 'surface_reference'
        R, r, beta, delta = req['header_radius'], req['branch_radius'], req['beta_deg'], req['delta']
        if fam == 'header_with_branch':
            for c, d in (('header_radius', 'header_dev_radius'), ('branch_radius', 'branch_dev_radius')):
                if not sref_consistent(req['surface_reference'], req[c], req[d]):
                    return 'invalid', f'{d} impossible for a contact radius declared {req["surface_reference"]} (negative wall thickness)'
        if not (0 < beta < 180):
            return 'invalid', 'beta outside (0, 180)'
        g = abs(delta) + r
        if abs(g - R * (1 - 1e-6)) <= 1e-12 * R:
            return 'edge', 'at the tangency limit'
        if g > R:
            return 'invalid', '|delta| + r > R: branch not fully on the header'
        if g >= R * (1 - 1e-6):
            return 'unsupported', 'tangent / near-tangent'
        # developed size of the branch: closed form of the saddle height (exact for every 0 < beta < 180)
        bb = math.radians(beta)
        if math.sin(bb) == 0.0 or bb == 0.0 or abs(math.sin(bb)) < 1e-300:
            return 'unsupported', 'developed branch of unbounded length (sin beta underflows)'
        ph = np.linspace(0, 2 * np.pi, 20001)
        sc = (np.sqrt(np.maximum(R * R - (delta + r * np.cos(ph)) ** 2, 0.0)) - r * np.sin(ph) * math.cos(bb)) / math.sin(bb)
        size = max(float(sc.max()), float(sc.max() - sc.min()))
        if abs(size - APEX_LIMIT) <= 1e-6 * APEX_LIMIT:
            return 'edge', 'at the developed-size limit'
        if size > APEX_LIMIT:
            return 'unsupported', 'developed branch longer than 1e8 mm'
        if max(R, req['branch_dev_radius'], size) > 1e5:
            hit = _chord_class(chord_sag_branch(R, r, req['branch_dev_radius'], beta, delta), 'branch saddle')
            if hit:
                return hit
        j = RF.Joint3D(R, r, beta, delta, req.get('branch_station', 0.0))
        phi = np.linspace(0, 2 * np.pi, 20001)
        s = j.s_of_phi(phi)
        smax = float(np.nanmax(s))
        if abs(req['s_end'] - smax) < 1e-5 * (R + abs(smax)):
            return 'edge', 's_end at the top of the saddle'
        if req['s_end'] <= smax:
            return 'invalid', 'square end inside the saddle'
        if fam == 'header_with_branch':
            x, _ = j.header_coords(j.branch_point(phi, s))
            L = req['header_length']
            lo, hi = float(x.min()), float(x.max())
            if min(abs(lo), abs(L - hi)) < 1e-5 * (R + L):
                return 'edge', 'opening at a plate end'
            if lo <= 0 or hi >= L:
                return 'invalid', 'opening not inside the plate'
        # task 1.1: in this regime the product either accepts a pattern its verification confirms or refuses the angle
        # with branch_angle_not_verifiable. Band 1e-12 below 1: the product evaluates the same closed form in binary64 with
        # a different order of about 60 operations (each relative error <= u) and its own sin/cos/radians (<= 2 ulp);
        # first order the two ratios differ by at most about 100 u = 1.1e-14 relative, so 1e-12 covers it with a
        # factor of about 100 and is far below any change of the angle that matters (the ratio moves by 2e-12 when
        # beta moves by 1e-12 relative)
        k = branch_conditioning(req)
        if k >= 1 - 1e-12:
            return 'edge', f'the verification is not guaranteed to confirm this branch angle (ratio {k:.3g} >= 1, task 1.1)'
        return 'valid', ''
    return 'invalid', 'unknown family'
