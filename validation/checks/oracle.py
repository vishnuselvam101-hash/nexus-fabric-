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
'edge'         within rounding distance of a boundary: either outcome is acceptable, an accepted result must still be correct
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
        return 'valid', ''
    return 'invalid', 'unknown family'
