"""True numerical error of product coordinates against the mpmath tier, to test whether the stated
eps_num (and eps_rep) actually bound the error.  Product coordinates are binary64 values taken as exact.

The product's sample parameters are not assumed from its code: uniform sampling is INFERRED from the output
(vertex count, abscissae) and the inference is confirmed by the size of the residual itself."""
import math
import mpmath as mp
import numpy as np
from reference import hp


def _max(v):
    return float(max(v)) if v else 0.0


def cylinder(req, part):
    W = hp.cylinder(req['r_dev'], req['length'])['developed_width']
    return {'shell': _max([abs(mp.mpf(part.measures()['bbox_w']) - W), abs(mp.mpf(part.measures()['bbox_h']) - hp.M(req['length']))])}


def cone(req, part):
    c = hp.right_cone(req['r1'], req['r2'], req['h'])
    L1, L2, phi = c['apex_radius_large'], c['apex_radius_small'], c['sector_angle']
    errs = []
    for name, L in (('large_end', L1), ('small_end', L2)):
        a = part.curve(name)
        errs.append(abs(mp.mpf(a.r) - L))
        errs.append(mp.hypot(mp.mpf(a.c[0]), mp.mpf(a.c[1])))
        # arc as the CAM system will read it: radius and angles in degrees
        for ang_deg, target in ((a.a0, 0), (a.a1, phi)):
            ang = mp.mpf(ang_deg) * mp.pi / 180
            p = (mp.mpf(a.c[0]) + mp.mpf(a.r) * mp.cos(ang), mp.mpf(a.c[1]) + mp.mpf(a.r) * mp.sin(ang))
            errs.append(mp.hypot(p[0] - L * mp.cos(target), p[1] - L * mp.sin(target)))
    for name, ang in (('seam_a', 0), ('seam_b', phi)):
        s = part.curve(name)
        for p in s.pts:
            rad = mp.hypot(mp.mpf(p[0]), mp.mpf(p[1]))
            L = L1 if abs(rad - L1) < abs(rad - L2) else L2
            errs.append(mp.hypot(mp.mpf(p[0]) - L * mp.cos(ang), mp.mpf(p[1]) - L * mp.sin(ang)))
    return {'reducer': _max(errs)}


def eccentric(req, part):
    c = hp.Cone(req['r1'], req['r2'], req['h'], req['e'])
    big = part.curve('large_end').pts
    n = len(big) - 1
    psi = c.psi_table(n)
    errs = []
    for i, p in enumerate(big):
        t = hp.TAU * i / n
        q = c.pattern_point(t, psi[i])
        errs.append(mp.hypot(mp.mpf(p[0]) - q[0], mp.mpf(p[1]) - q[1]))
    small = part.curve('small_end').pts[::-1]
    for i, p in enumerate(small):
        t = hp.TAU * i / n
        q = c.pattern_point(t, psi[i], small=True)
        errs.append(mp.hypot(mp.mpf(p[0]) - q[0], mp.mpf(p[1]) - q[1]))
    return {'reducer': _max(errs)}


def branch(req, part):
    """Normal distance of each saddle vertex from the exact developed saddle (product layout u = r_dev*phi, v = s_end - s inferred from the output)."""
    j = hp.Joint(req['header_radius'], req['branch_radius'], req['beta_deg'], req['delta'])
    rd, s_end = hp.M(req['branch_dev_radius']), hp.M(req['s_end'])
    errs = []
    for p in part.curve('saddle_cut').pts:
        phi = mp.mpf(p[0]) / rd
        ev = mp.mpf(p[1]) - (s_end - j.s(phi))
        slope = mp.diff(j.s, phi) / rd if 0 < phi < hp.TAU else 0
        errs.append(abs(ev) / mp.sqrt(1 + slope ** 2))
    return {'branch': _max(errs)}


def header(req, part):
    j = hp.Joint(req['header_radius'], req['branch_radius'], req['beta_deg'], req['delta'])
    pts = part.curve('branch_opening').pts
    n = len(pts) - 1
    best = None
    for direction in (1, -1):
        errs = []
        for i, p in enumerate(pts):
            phi = hp.TAU * (i if direction == 1 else n - i) / n
            q = j.plate(phi, req['header_dev_radius'], req['branch_station'])
            errs.append(mp.hypot(mp.mpf(p[0]) - q[0], mp.mpf(p[1]) - q[1]))
        m = _max(errs)
        best = m if best is None else min(best, m)
    return {'header': best}


def true_errors(req, job):
    fam = req['family']
    if fam == 'cylinder_shell':
        out = cylinder(req, job.parts['shell'])
    elif fam == 'cone_frustum':
        out = cone(req, job.parts['reducer'])
    elif fam == 'eccentric_reducer':
        out = eccentric(req, job.parts['reducer'])
    elif fam == 'branch_stub':
        out = branch(req, job.parts['branch'])
    else:
        out = dict(header(req, job.parts['header']), **branch(req, job.parts['branch']))
    return {k: {'true_error': v, 'eps_num': job.parts[k].eps_num, 'ratio': (v / job.parts[k].eps_num if job.parts[k].eps_num > 0 else float('inf'))} for k, v in out.items()}
