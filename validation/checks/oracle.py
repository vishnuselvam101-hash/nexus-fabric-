"""Independent accept/refuse oracle for arbitrary requests.

'valid'        geometry is well defined and inside the product's DECLARED supported range -> must be accepted
'invalid'      no such part exists (non-positive size, branch not fully on the header, opening off the plate ...) -> must be refused
'unsupported'  a part exists but lies in a range the product documents as not implemented
               (docs/LEARNING_AND_STRESS_TESTING.md, web/app.js input limits): near-cylindrical cones (sin alpha < 1e-3),
               equal radii with offset, grazing branches (sin beta <= 0.17), tangent/near-tangent saddles (|delta|+r >= R(1-1e-6)),
               developed angle >= 0.999 * 2 pi  -> must be refused
'edge'         within rounding distance of a boundary: either outcome is acceptable, an accepted result must still be correct
"""
import math
import numpy as np
from reference import families as RF

SREF = ('outside', 'inside', 'neutral', 'unknown')


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
        sa = (r1 - r2) / math.hypot(h, r1 - r2)
        if abs(sa - 1e-3) < 1e-9:
            return 'edge', 'at the near-cylindrical limit'
        if sa < 1e-3:
            return 'unsupported', 'near-cylindrical'
        if fam == 'eccentric_reducer':
            a, H = e * r1 / (r1 - r2), h * r1 / (r1 - r2)
            th = (np.arange(4000) + 0.5) * 2 * np.pi / 4000
            psi = float(np.sum(r1 * np.sqrt(H * H + (r1 - a * np.cos(th)) ** 2) / (r1 * r1 - 2 * a * r1 * np.cos(th) + a * a + H * H)) * 2 * np.pi / 4000)
            if abs(psi / (2 * np.pi) - 0.999) < 1e-6:
                return 'edge', 'at the overlap limit'
            if psi >= 0.999 * 2 * np.pi:
                return 'unsupported', 'developed angle within 0.1 % of a full turn'
        return 'valid', ''
    if fam in ('branch_stub', 'header_with_branch'):
        names = ['header_radius', 'branch_radius', 'branch_dev_radius', 's_end'] + (['header_dev_radius', 'header_length'] if fam == 'header_with_branch' else [])
        bad = _pos(req, names)
        if bad:
            return 'invalid', f'{bad} <= 0'
        if fam == 'header_with_branch' and req.get('surface_reference') not in SREF:
            return 'invalid', 'surface_reference'
        R, r, beta, delta = req['header_radius'], req['branch_radius'], req['beta_deg'], req['delta']
        if not (0 < beta < 180):
            return 'invalid', 'beta outside (0, 180)'
        sb = math.sin(math.radians(beta))
        if abs(sb - 0.17) < 1e-9:
            return 'edge', 'at the grazing limit'
        if sb <= 0.17:
            return 'unsupported', 'grazing angle'
        g = abs(delta) + r
        if abs(g - R * (1 - 1e-6)) <= 1e-12 * R:
            return 'edge', 'at the tangency limit'
        if g > R:
            return 'invalid', '|delta| + r > R: branch not fully on the header'
        if g >= R * (1 - 1e-6):
            return 'unsupported', 'tangent / near-tangent'
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
