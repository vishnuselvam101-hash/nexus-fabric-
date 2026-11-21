#!/usr/bin/env python3
"""Builds the benchmark corpus (validation/corpus/cases/*.json).

Run once; the JSON files are the corpus and are committed.  Expected values come from
  (a) closed forms that can be checked by hand (stated in each case's "derivation"),
  (c) the mpmath tier of the reference (reference/hp.py, 25-40 digits),
  (d) the float tier of the reference (3D construction + numerical intersection) for assembly layouts.
Nothing here calls the product.

Case schema
  id, group, title, derivation
  request                      JSON body for POST /api/projects/{id}/calculations  (or "raw_body" for malformed JSON)
  expect                       "accept" | "refuse"
  refusal {kind, mentions}     kind: "engineering" (HTTP 200, no pattern, no DXF) | "malformed" (HTTP 400)
  quantities {part: {name: [value, abs_tol]}}   compared with the product's reported quantities
  measures   {part: {name: [value, abs_tol]}}   compared with values measured from the product's DXF by the validator
  relation {type, ...}         metamorphic relation to a second request (see run_all.py)
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import mpmath as mp
import numpy as np
from reference import hp
from reference import families as RF

OUT = os.path.join(HERE, 'cases')
os.makedirs(OUT, exist_ok=True)
CASES = []
POLY = 3e-5     # relative allowance where the product's 720/360-segment polyline is measured against an exact curve


def f(x):
    return float(x)


def case(id, group, title, request, derivation, expect='accept', **kw):
    c = {'id': id, 'group': group, 'title': title, 'request': request, 'expect': expect, 'derivation': derivation}
    c.update(kw)
    CASES.append(c)
    return c


def refuse(id, title, request, mentions, derivation, kind='engineering', raw_body=None):
    c = case(id, 'refusal', title, request, derivation, expect='refuse', refusal={'kind': kind, 'mentions': mentions})
    if raw_body is not None:
        c['raw_body'] = raw_body


def q(v, tol=None, rel=1e-11):
    v = f(v)
    return [v, tol if tol is not None else max(abs(v) * rel, 1e-12)]


def hwb(R, r, beta, delta, L, st, s_end, R_dev=None, r_dev=None, sref='outside'):
    return {'family': 'header_with_branch', 'header_radius': R, 'header_dev_radius': R if R_dev is None else R_dev, 'header_length': L, 'branch_station': st,
            'branch_radius': r, 'branch_dev_radius': r if r_dev is None else r_dev, 'beta_deg': beta, 'delta': delta, 's_end': s_end, 'surface_reference': sref}


def stub(R, r, beta, delta, s_end, r_dev=None):
    return {'family': 'branch_stub', 'header_radius': R, 'branch_radius': r, 'branch_dev_radius': r if r_dev is None else r_dev, 'beta_deg': beta, 'delta': delta, 's_end': s_end}


def layout(req):
    """Float-tier reference layout of an assembly in the natural plate frame
    (x = station from the plate end, y = developed arc from the seam opposite the branch)."""
    j = RF.Joint3D(req['header_radius'], req['branch_radius'], req['beta_deg'], req['delta'], req.get('branch_station', 0.0))
    phi = np.linspace(0, 2 * np.pi, 400001)
    s = j.s_of_phi(phi)
    out = {'s_min': float(np.nanmin(s)), 's_max': float(np.nanmax(s)), 's0': float(s[0])}
    if 'header_length' in req:
        x, g = j.header_coords(j.branch_point(phi, s))
        y = g * req['header_dev_radius']
        out.update(xmin=float(x.min()), xmax=float(x.max()), ymin=float(y.min()), ymax=float(y.max()))
    return out


def assembly_expect(req, with_hp=False):
    lay = layout(req)
    R, L, Rd, rd = req['header_radius'], req.get('header_length'), req.get('header_dev_radius'), req['branch_dev_radius']
    scale = max(R, req['s_end'], L or 0, 2 * math.pi * (Rd or 0))
    tp = POLY * scale
    W = 2 * math.pi * rd
    mb = {'bbox_w': q(W), 'bbox_h': [req['s_end'] - lay['s_min'], tp], 'len_seam_a': q(req['s_end'] - lay['s0'], rel=1e-10), 'len_seam_b': q(req['s_end'] - lay['s0'], rel=1e-10), 'len_square_end': q(W)}
    qb = {'developed_width': q(W), 's_min': [lay['s_min'], tp], 's_max': [lay['s_max'], tp], 's_at_0': q(lay['s0'], rel=1e-10)}
    measures, quantities = {'branch': mb}, {'branch': qb}
    if L is not None:
        WH = 2 * math.pi * Rd
        measures['header'] = {'bbox_w': q(L), 'bbox_h': q(WH), 'outline_area': q(L * WH), 'hole_xmin': [lay['xmin'], tp], 'hole_xmax': [lay['xmax'], tp],
                              'hole_ymin': [lay['ymin'], tp], 'hole_ymax': [lay['ymax'], tp], 'hole_count': [1, 0]}
        quantities['header'] = {'developed_width': q(WH), 'length': q(L), 'opening_x_min': [lay['xmin'], tp], 'opening_x_max': [lay['xmax'], tp]}
    if with_hp:
        h = hp.joint_quantities(R, req['branch_radius'], req['beta_deg'], req['delta'], Rd, rd, req['s_end'])
        mb['len_saddle_cut'] = [f(h['saddle_developed_length']), POLY * f(h['saddle_developed_length'])]
        mb['outline_area'] = [f(h['branch_pattern_area']), POLY * f(h['branch_pattern_area'])]
        qb['s_at_90'] = q(h['s_at_90'], rel=1e-10)
        if L is not None:
            measures['header']['hole_perimeter'] = [f(h['opening_perimeter']), POLY * f(h['opening_perimeter'])]
            measures['header']['hole_area'] = [f(h['opening_area']), 3 * POLY * f(h['opening_area'])]
            quantities['header']['opening_perimeter'] = [f(h['opening_perimeter']), POLY * f(h['opening_perimeter'])]
    return quantities, measures


# =========================================================== (a) closed forms checkable by hand
pi = math.pi
case('A-CYL-01', 'closed_form', 'Plain shell r=500, L=1000 (CYL-02)', {'family': 'cylinder_shell', 'r_dev': 500, 'length': 1000},
     'Rectangle 2*pi*500 x 1000 = 3141.592653589793 x 1000; area 1000*pi*1000.',
     quantities={'shell': {'developed_width': q(1000 * pi), 'length': q(1000), 'area': q(1e6 * pi)}},
     measures={'shell': {'outline_area': q(1e6 * pi), 'outline_perimeter': q(2 * (1000 * pi + 1000)), 'bbox_w': q(1000 * pi), 'bbox_h': q(1000), 'hole_count': [0, 0]}})
case('A-CYL-02', 'closed_form', 'Shell with r = 1/(2 pi): unit width', {'family': 'cylinder_shell', 'r_dev': 1 / (2 * pi), 'length': 3},
     'Width 2*pi*r = 1 exactly (up to rounding of the input), area 3.',
     quantities={'shell': {'developed_width': q(1.0, rel=1e-14), 'area': q(3.0, rel=1e-14)}}, measures={'shell': {'outline_area': q(3.0, rel=1e-14), 'bbox_w': q(1.0, rel=1e-14)}})
h_half = math.sqrt(100 ** 2 - 50 ** 2)
case('A-CONE-HALFDISC', 'closed_form', 'Right cone with sin(alpha)=1/2: development is a half annulus', {'family': 'cone_frustum', 'r1': 100, 'r2': 50, 'h': h_half},
     'r1-r2=50, slant s=100 (h=sqrt(100^2-50^2)=86.60254037844386) so sin(alpha)=1/2, sector angle = pi. L1=200, L2=100. '
     'Half annulus: area = pi/2*(200^2-100^2) = 15000*pi; bounding box 400 x 200; chord across the large arc = 400; outline = pi*200 + pi*100 + 2*100.',
     quantities={'reducer': {'sector_angle': q(pi), 'slant_length': q(100), 'sin_half_angle': q(0.5), 'apex_radius_large': q(200), 'apex_radius_small': q(100), 'area': q(15000 * pi), 'chord_large_arc': q(400)}},
     measures={'reducer': {'outline_area': q(15000 * pi, rel=1e-10), 'outline_perimeter': q(300 * pi + 200, rel=1e-10), 'bbox_w': q(400, rel=1e-10), 'bbox_h': q(200, rel=1e-10), 'len_seam_a': q(100), 'len_seam_b': q(100), 'len_large_end': q(200 * pi), 'len_small_end': q(100 * pi)}})
case('A-CONE-345', 'closed_form', 'Right cone on a 3-4-5 triangle: sector angle 1.2*pi', {'family': 'cone_frustum', 'r1': 400, 'r2': 100, 'h': 400},
     'r1-r2=300, h=400, s=500, sin(alpha)=3/5, sector = 2*pi*3/5 = 216 deg. L1=400/0.6=666.666..., L2=166.666.... Area = pi*(r1+r2)*s = 250000*pi. '
     'Sector > 180 deg: bounding box width = 2*L1, height = L1 + L1*sin(36 deg).',
     quantities={'reducer': {'sector_angle': q(1.2 * pi), 'slant_length': q(500), 'sin_half_angle': q(0.6), 'apex_radius_large': q(2000 / 3), 'apex_radius_small': q(500 / 3), 'area': q(250000 * pi), 'chord_large_arc': q(2 * 2000 / 3 * math.sin(0.6 * pi))}},
     measures={'reducer': {'outline_area': q(250000 * pi, rel=1e-10), 'bbox_w': q(4000 / 3, rel=1e-10), 'bbox_h': q(2000 / 3 * (1 + math.sin(math.radians(36))), rel=1e-10), 'len_large_end': q(800 * pi), 'len_small_end': q(200 * pi)}})
case('A-CONE-01', 'closed_form', 'CONE-01 of the reference file', {'family': 'cone_frustum', 'r1': 200, 'r2': 100, 'h': 300},
     's = sqrt(300^2+100^2) = 100*sqrt(10); sin(alpha) = 1/sqrt(10); sector = 2*pi/sqrt(10); area = pi*300*100*sqrt(10).',
     quantities={'reducer': {'sector_angle': q(2 * pi / math.sqrt(10)), 'slant_length': q(100 * math.sqrt(10)), 'area': q(30000 * pi * math.sqrt(10)), 'apex_radius_large': q(200 * math.sqrt(10))}},
     measures={'reducer': {'outline_area': q(30000 * pi * math.sqrt(10), rel=1e-10), 'len_large_end': q(400 * pi), 'len_small_end': q(200 * pi), 'len_seam_a': q(100 * math.sqrt(10))}})
# 90 degree tee, centred: s = sqrt(R^2 - r^2 cos^2 phi); opening x = -r sin phi, omega = asin(r cos phi / R)
R, r = 150, 75
rq = hwb(R, r, 90, 0, 1000, 400, 300)
case('A-TEE-90', 'closed_form', '90 deg centred tee, r/R = 1/2: saddle and opening extents in closed form', rq,
     'Centred 90 deg tee: s(phi) = sqrt(R^2 - r^2 cos^2 phi), so s_min = sqrt(150^2-75^2) = 129.9038105676658 at phi=0,180 and s_max = R = 150 at phi=90,270. '
     'Opening on the plate: along the axis it spans station -/+ r = 325..475; around the header it spans R*(pi -/+ asin(r/R)) = 150*(pi -/+ pi/6) = 392.699..549.779 '
     'measured from a seam diametrically opposite the branch.  Plate 1000 x 2*pi*150.  Branch pattern: width 2*pi*75, seam length s_end - s(0) = 300 - 129.90381, height 300 - 129.90381.',
     quantities={'header': {'opening_x_min': [325, POLY * 1000], 'opening_x_max': [475, POLY * 1000], 'developed_width': q(300 * pi)},
                 'branch': {'s_min': q(math.sqrt(R * R - r * r), rel=1e-10), 's_max': q(150, rel=1e-10), 's_at_0': q(math.sqrt(R * R - r * r)), 's_at_90': q(150), 'developed_width': q(150 * pi)}},
     measures={'header': {'bbox_w': q(1000), 'bbox_h': q(300 * pi), 'hole_xmin': [325, POLY * 1000], 'hole_xmax': [475, POLY * 1000], 'hole_ymin': [150 * 5 * pi / 6, POLY * 1000], 'hole_ymax': [150 * 7 * pi / 6, POLY * 1000],
                          'hole_cx': [400, 1e-6], 'hole_cy': [150 * pi, 1e-6]},
               'branch': {'bbox_w': q(150 * pi), 'bbox_h': q(300 - math.sqrt(R * R - r * r), rel=1e-10), 'len_seam_a': q(300 - math.sqrt(R * R - r * r))}})
for k, (R, r, beta, delta, tab) in enumerate([
        (150, 100, 90, 0, {0: math.sqrt(150 ** 2 - 100 ** 2), 90: 150.0}),
        (150, 100, 60, 0, {0: math.sqrt(150 ** 2 - 100 ** 2) / math.sin(math.radians(60)), 90: (150 - 100 * math.cos(math.radians(60))) / math.sin(math.radians(60))}),
        (150, 80, 90, 40, {0: 90.0, 90: math.sqrt(150 ** 2 - 40 ** 2)})]):
    case(f'A-BR-0{k + 1}', 'closed_form', f'Branch stub R={R} r={r} beta={beta} delta={delta}: hand values at phi = 0 and 90 deg', stub(R, r, beta, delta, 400),
         'phi=0 is the ruling farthest out along the offset direction: y = delta + r, s = sqrt(R^2 - (delta+r)^2)/sin(beta). '
         'phi=90 is the ruling in the plane of the two axis directions on the heel side: s = (sqrt(R^2 - delta^2) - r cos(beta))/sin(beta).',
         quantities={'branch': {'s_at_0': q(tab[0], rel=1e-12), 's_at_90': q(tab[90], rel=1e-12), 'developed_width': q(2 * pi * r)}},
         measures={'branch': {'len_seam_a': q(400 - tab[0], rel=1e-12), 'len_seam_b': q(400 - tab[0], rel=1e-12), 'bbox_w': q(2 * pi * r)}})
# B2-005 (engine 0.4.2): shallow angles and slender / flat cones, formerly refused by implementation limits, are exact and accepted
for beta in (5, 175):
    sbr, cbr = math.sin(math.radians(beta)), math.cos(math.radians(beta))
    t0 = math.sqrt(150 ** 2 - 100 ** 2) / sbr
    t90 = (150 - 100 * cbr) / sbr
    case(f'A-BR-GRAZE-{beta}', 'closed_form', f'Branch stub R=150 r=100 beta={beta} (shallow angle, accepted since 0.4.2)', stub(150, 100, beta, 0, 4000),
         'The saddle is exact for every 0 < beta < 180: phi=0: s = sqrt(R^2 - r^2)/sin(beta); phi=90: s = (R - r cos(beta))/sin(beta). Developed size s_max = (R + |r cos(beta)|)/sin(beta) = 2864 mm, far below 1e8 mm.',
         quantities={'branch': {'s_at_0': q(t0, rel=1e-12), 's_at_90': q(t90, rel=1e-12), 'developed_width': q(200 * pi)}},
         measures={'branch': {'len_seam_a': q(4000 - t0, rel=1e-12), 'bbox_w': q(200 * pi)}})
h_slender = hp.right_cone(200, 199.9, 300)
case('A-CONE-SLENDER', 'closed_form', 'CONE-02: right cone with sin(alpha) = 3.3e-4 (near-cylindrical, accepted since 0.4.2)', {'family': 'cone_frustum', 'r1': 200, 'r2': 199.9, 'h': 300},
     'Closed forms are exact for any sin(alpha) > 0 (r1 - r2 = 0.1 is exact): apex radius L1 = r1/sin(alpha) = 6e5 mm, sector angle 2 pi sin(alpha), area pi (r1 + r2) s (values from the mpmath tier).',
     quantities={'reducer': {k: q(f(h_slender[k]), rel=1e-11) for k in ('slant_length', 'sin_half_angle', 'apex_radius_large', 'apex_radius_small', 'sector_angle', 'area', 'chord_large_arc')}})
h_flat = hp.right_cone(300, 50, 0.01)
case('A-CONE-FLAT-0.01', 'closed_form', 'Nearly flat cone r1=300 r2=50 h=0.01: sector 99.9999 % of a turn (accepted since 0.4.2)', {'family': 'cone_frustum', 'r1': 300, 'r2': 50, 'h': 0.01},
     'A cone sector is always below a full turn; here the seam gap l2 (2 pi - Phi) = 50 * 2 pi * 8e-10 = 2.5e-7 mm is above the 1e-9 mm closure resolution, so the pattern is accepted (values from the mpmath tier).',
     quantities={'reducer': {k: q(f(h_flat[k]), rel=1e-11) for k in ('slant_length', 'sin_half_angle', 'apex_radius_large', 'apex_radius_small', 'area')}})
# B2-005 boundary corpus (engine 0.4.2): for every loosened rule, a case just INSIDE (accepted, values from the 50-digit tier) and a case just
# OUTSIDE (refused), 1 % to 7 % either side of the limit; the limit itself is an 'edge' for the oracle (either outcome, a result must be right).
def _h_for_cone_gap(g, r1=300, r2=50):
    """h of the cone (r1, r2, h) whose seam gap l2 (2 pi - Phi) = l2 * 2 pi * (1 - sin alpha) equals g (mpmath, 40 digits)."""
    with mp.workdps(40):
        g = mp.mpf(g)
        d, l2 = mp.mpf(r1 - r2), mp.mpf(r2)
        for _ in range(6):
            l2 = r2 / (1 - g / (2 * mp.pi * l2))
        u = g / (2 * mp.pi * l2)
        return float(d * mp.sqrt(2 * u - u * u) / (1 - u))


def _seam_gap(r1, r2, h):
    with mp.workdps(40):
        d, hh = mp.mpf(r1 - r2), mp.mpf(h)
        hyp = mp.sqrt(hh * hh + d * d)
        return float((r2 * hyp / d) * 2 * mp.pi * (hh * hh / (hyp * (hyp + d))))


# seam gap l2 (2 pi - Phi) at the small end against the 1e-9 mm closure resolution (cone and eccentric reducer with e = 0, same sector)
for tag, g in (('below', 0.95e-9), ('above', 1.05e-9)):
    hg = _h_for_cone_gap(g)
    gg = _seam_gap(300, 50, hg)
    for fam, extra in (('cone_frustum', {}), ('eccentric_reducer', {'e': 0})):
        req = dict({'family': fam, 'r1': 300, 'r2': 50, 'h': hg}, **extra)
        ttl = f'{fam} r1=300 r2=50 h={hg!r}: seam gap {gg:.4g} mm, {tag} the 1e-9 mm closure resolution'
        der = f'The seam gap at the small end is l2 (2 pi - Phi) = {gg:.4g} mm (mpmath, 40 digits), {"below" if tag == "below" else "above"} the 1e-9 mm closure resolution of the profile check (5 % {"under" if tag == "below" else "over"} the limit).'
        if tag == 'below':
            refuse(f'E-{fam}-seam-gap-{tag}', ttl, req, ['seam', 'closure'], der)
        elif fam == 'cone_frustum':
            hc = hp.right_cone(300, 50, hg)
            case(f'B-{fam}-seam-gap-{tag}', 'closed_form', ttl, req, der,
                 quantities={'reducer': {k: q(f(hc[k]), rel=1e-11) for k in ('slant_length', 'sin_half_angle', 'apex_radius_large', 'apex_radius_small', 'area')}})
        else:
            case(f'B-{fam}-seam-gap-{tag}', 'closed_form', ttl, req, der)
# Q2(b): the profile gate cannot say PASS when the seam gap is below twice the representation error of the adjoining polylines (eccentric reducer,
# e = 0: a polyline sector of 720 segments, eps_rep about 2.9e-3 mm); the validator measures the seam clearance and the chord error itself.
for tag, g in (('inconclusive', 3e-3), ('pass', 5e-2)):
    hg = _h_for_cone_gap(g)
    gg = _seam_gap(300, 50, hg)
    case(f'B-eccentric_reducer-seam-vs-eps-rep-{tag}', 'closed_form', f'eccentric reducer r1=300 r2=50 h={hg!r} e=0: seam gap {gg:.4g} mm ({"below 2 eps_rep: the profile gate is INCONCLUSIVE" if tag == "inconclusive" else "above 2 x 0.01 mm: the profile gate passes"})',
         {'family': 'eccentric_reducer', 'r1': 300, 'r2': 50, 'h': hg, 'e': 0},
         f'Seam gap {gg:.4g} mm (mpmath). The polyline sector has eps_rep at most the 0.01 mm chord limit and (for 720 segments of the 300 mm arc) about 2.9e-3 mm: a gap of {gg:.3g} mm is {"smaller" if tag == "inconclusive" else "larger"} than twice that, so the seam {"cannot" if tag == "inconclusive" else "can"} be told from a closed one at the representation error.')
# Q2(b) must-fix 2 (engine 0.4.2 review): the cone seam resolution is max(1e-9 mm, 2 eps_num), not the bare 1e-9 mm. At a large size the numerical
# error of the written vertices (eps_num ~ 7e-8 mm for an apex radius of 1e7 mm) puts the floor at ~1.5e-7 mm. A seam gap between 1e-9 mm and
# that floor must NOT get G11 PASS (INCONCLUSIVE; the DXF cannot represent the seam and G15 may reject the job, so either outcome is allowed but
# the state must never be APPROVED and the gate must be INCONCLUSIVE); a gap about twice the floor is accepted with G11 PASS.
for tag, g in (('in-band', 1e-7), ('above-band', 3e-7)):
    hg = _h_for_cone_gap(g, 1e7, 9e6)
    gg = _seam_gap(1e7, 9e6, hg)
    req = {'family': 'cone_frustum', 'r1': 1e7, 'r2': 9e6, 'h': hg}
    der = (f'Seam gap l2 (2 pi - Phi) = {gg:.4g} mm (mpmath, 40 digits) for a cone of apex radius about 1e7 mm. The numerical error of its written vertices is about 7e-8 mm, '
           f'so the seam cannot be told from a closed one below about 1.5e-7 mm (twice that error): the gap is {"inside that band, so G11 is INCONCLUSIVE, never PASS" if tag == "in-band" else "above that band, so G11 passes"}.')
    if tag == 'in-band':
        c = case(f'B-cone-seam-vs-eps-num-{tag}', 'closed_form', f'cone r1=1e7 r2=9e6 h={hg!r}: seam gap {gg:.4g} mm, between 1e-9 mm and 2 eps_num (G11 INCONCLUSIVE)', req, der, expect='either')
        c['g11'] = 'INCONCLUSIVE'
    else:
        hc = hp.right_cone(1e7, 9e6, hg)
        c = case(f'B-cone-seam-vs-eps-num-{tag}', 'closed_form', f'cone r1=1e7 r2=9e6 h={hg!r}: seam gap {gg:.4g} mm, above 2 eps_num (G11 PASS)', req, der,
                 quantities={'reducer': {k: q(f(hc[k]), rel=1e-11) for k in ('slant_length', 'sin_half_angle', 'apex_radius_large', 'apex_radius_small', 'area')}})
        c['g11'] = 'PASS'
# apex radius against the 1e8 mm developed-size limit (cone, r1 = 1e5, r2 = 99999: l1 = r1 hypot(h, 1))
for tag, hyp_t in (('below', 999.0), ('above', 1001.0)):
    hh = math.sqrt(hyp_t ** 2 - 1)
    req = {'family': 'cone_frustum', 'r1': 100000, 'r2': 99999, 'h': hh}
    l1 = 1e5 * hyp_t
    if tag == 'below':
        hc = hp.right_cone(100000, 99999, hh)
        case('B-cone-apex-below-1e8', 'closed_form', f'cone r1=1e5 r2=99999 h={hh!r}: apex radius {l1:.4g} mm, 1 % below the 1e8 mm limit', req,
             f'Apex radius L1 = r1 hypot(h, r1 - r2)/(r1 - r2) = {l1:.6g} mm (1 % below the 1e8 mm developed-size limit); closed forms from the mpmath tier.',
             quantities={'reducer': {k: q(f(hc[k]), rel=1e-11) for k in ('slant_length', 'sin_half_angle', 'apex_radius_large', 'apex_radius_small', 'sector_angle', 'area')}})
    else:
        refuse('E-cone-apex-above-1e8', f'cone r1=1e5 r2=99999 h={hh!r}: apex radius {l1:.4g} mm, 1 % above the 1e8 mm limit', req, ['apex', 'radius'],
               f'Apex radius L1 = {l1:.6g} mm, 1 % above the 1e8 mm developed-size limit.')
# developed branch length: refused above 1e8 mm (1 % above the limit, and its mirror: beta -> 180 - beta gives the same developed saddle);
# accepted at beta 0.003 deg and 179.997 deg (s_max = 4.8e6 mm). Between that and 1e8 mm the geometry is correct (measured 4.2e-8 mm against a
# 31-digit reference at beta = 1.4e-4 deg, within eps_num) but the product's own verification, conditioned by 1 / sin^2 beta, cannot confirm it;
# since engine 0.4.3 such an angle is refused with branch_angle_not_verifiable (work plan task 1.1; cases B-/E-br-1.1-* and B-/E-hwb-1.1-* below).
# 0.003 deg lies inside the regime where the verification is no longer guaranteed (ratio about 39, oracle.branch_conditioning) but the
# verification confirms it: it stays accepted. 179.997 deg is outside the regime (ratio 0.21).
for tag, bd, size in (('below', 0.003, 4.775e6), ('above', 0.000141821, 1.01e8)):
    for name, b in ((f'beta-{bd}', bd), (f'beta-180-{bd}', 180 - bd)):
        if tag == 'below':
            req = stub(150, 100, b, 0, 5.2e6)
            sb, cb = math.sin(math.radians(b)), math.cos(math.radians(b))
            case(f'B-br-verified-floor-{name}', 'closed_form', f'Branch stub R=150 r=100 beta={b!r}: developed length {size:.3g} mm, the shallowest angle the verification covers', req,
                 f'Developed size max(s_max, s_max - s_min) = {size:.4g} mm (dense evaluation of the closed form): accepted and verified. phi=0: s = sqrt(R^2 - r^2)/sin(beta); phi=90: s = (R - r cos(beta))/sin(beta).',
                 quantities={'branch': {'s_at_0': q(math.sqrt(150 ** 2 - 100 ** 2) / sb, rel=1e-9), 's_at_90': q((150 - 100 * cb) / sb, rel=1e-9), 'developed_width': q(200 * pi)}})
        else:
            req = stub(150, 100, b, 0, 0.995e8)
            refuse(f'E-br-size-above-{name}', f'Branch stub R=150 r=100 beta={b!r}: developed length {size:.3g} mm, 1 % above the 1e8 mm limit', req, ['beta', '1e8'],
                   f'Developed size {size:.3g} mm, 1 % above the 1e8 mm limit: refused.')
# Work plan task 1.1 (engine 0.4.3): the boundary of the regime in which the product's verification is no longer guaranteed to confirm a
# correct branch pattern. ratio = F / (agree + 3 bound) of checks/oracle.py branch_conditioning (closed form, TOLERANCE_POLICY.md section 7).
# Just inside (ratio 0.9): must be accepted and agree with the reference. Outside, where the verification measurably cannot confirm the
# pattern: must be refused (code branch_angle_not_verifiable). Inside the regime with the verification confirming: must be accepted.
from checks.oracle import branch_conditioning


def _angle_at(make, lo, hi, target=0.9):
    """Angle in (lo, hi) at which branch_conditioning(make(beta)) = target (bisection; monotone on either side of 90 deg)."""
    a, b = lo, hi
    rising = branch_conditioning(make(b)) > branch_conditioning(make(a))
    for _ in range(60):
        m = 0.5 * (a + b)
        if (branch_conditioning(make(m)) > target) == rising:
            b = m
        else:
            a = m
    return 0.5 * (a + b)


def _asm_layout(R, r, beta, delta):
    """Header with branch on a plate just long enough for the opening (reference float tier: 3D construction)."""
    j = RF.Joint3D(R, r, beta, delta)
    ph = np.linspace(0, 2 * np.pi, 40001)
    s = j.s_of_phi(ph)
    x, _ = j.header_coords(j.branch_point(ph, s))
    return hwb(R, r, beta, delta, float(math.ceil(x.max() - x.min() + 2 * R)), float(math.ceil(-x.min() + R)), float(math.ceil(np.nanmax(s) + R)))


def _branch_q(R, r, b):
    sb, cb = math.sin(math.radians(b)), math.cos(math.radians(b))
    return {'s_at_0': q(math.sqrt(R ** 2 - r ** 2) / sb, rel=1e-9), 's_at_90': q((R - r * cb) / sb, rel=1e-9), 'developed_width': q(2 * pi * r)}


_st = lambda b: stub(150, 100, b, 0, float(math.ceil(250 / math.sin(math.radians(b)) + 150)))
b_in = round(_angle_at(_st, 0.005, 1.0), 7)
case('B-br-1.1-acute-inside', 'closed_form', f'Branch stub R=150 r=100 beta={b_in!r}: just inside the angle the verification is guaranteed to confirm (ratio 0.9)', _st(b_in),
     f'ratio F/(agree + 3 bound) = {branch_conditioning(_st(b_in)):.4f} < 1 (closed form, TOLERANCE_POLICY.md section 7): the verification is guaranteed to confirm a correct '
     f'pattern, so the request must be accepted. phi=0: s = sqrt(R^2 - r^2)/sin(beta); phi=90: s = (R - r cos(beta))/sin(beta).', hp=True, quantities={'branch': _branch_q(150, 100, b_in)})
refuse('E-br-1.1-acute-outside', 'Branch stub R=150 r=100 beta=0.002: the verification cannot confirm the pattern', _st(0.002), ['beta_deg'],
       f'ratio F/(agree + 3 bound) = {branch_conditioning(_st(0.002)):.1f}: the bound on the rounding of the verification (it grows as 1/sin^2 beta) is 87 times the part of '
       'its tolerance left after the engine\'s own error; the verification is not guaranteed and, measured on engine 0.4.2, its 3D intersection check differed by 2.9e-6 mm '
       'against a tolerance of 1.07e-6 mm (G10 FAIL, job REJECTED). The geometry itself is correct (31-digit reference); the angle is refused with its own code.')
_as = lambda b: _asm_layout(150, 100, b, 0)
a_in = round(_angle_at(_as, 0.02, 1.0), 6)
a_ob = round(_angle_at(_as, 179.9, 179.9999), 6)
for tag, b in (('acute-inside', a_in), ('obtuse-inside', a_ob)):
    case(f'B-hwb-1.1-{tag}', 'closed_form', f'Header with branch R=150 r=100 beta={b!r}: just inside the angle the verification is guaranteed to confirm (ratio 0.9)', _as(b),
         f'ratio F/(agree + 3 bound) = {branch_conditioning(_as(b)):.4f} < 1 over the opening, branch and joint checks (closed form, TOLERANCE_POLICY.md section 7): '
         'must be accepted. Plate layout from the 3D construction of the reference; saddle heights from the closed form.', hp=True, quantities={'branch': _branch_q(150, 100, b)})
refuse('E-hwb-1.1-acute-outside', 'Header with branch R=150 r=100 beta=0.009: the verification cannot confirm the pattern', _as(0.009), ['beta_deg'],
       f'ratio F/(agree + 3 bound) = {branch_conditioning(_as(0.009)):.1f} (joint checks): not guaranteed; measured on engine 0.4.2, the two 3D checks of the joint '
       'differed by about 2 times their tolerance (G10 FAIL, job REJECTED). Refused with branch_angle_not_verifiable since engine 0.4.3.')
case('B-hwb-1.1-obtuse-in-regime', 'closed_form', 'Header with branch R=150 r=100 beta=179.99: outside the guaranteed range (ratio 1.11) but confirmed by the verification', _as(179.99),
     f'ratio F/(agree + 3 bound) = {branch_conditioning(_as(179.99)):.3f} > 1: the verification is not guaranteed, but its measured differences are about 1/20 of its '
     'tolerances, so it confirms the pattern: the angle must not be refused for the bound alone (no over-refusal). The parts carry eps_num_governs: near 180 deg the rounding '
     'of beta itself dominates eps_num. Saddle heights from the closed form.', hp=True, quantities={'branch': _branch_q(150, 100, 179.99)})
# the nearest representable angles to the excluded ends: nextafter(0, 1) and nextafter(180, 0) make a developed branch of 1e17 mm and more
for nm, b in (('nextafter-0', 5e-324), ('nextafter-180', 179.99999999999997)):
    refuse(f'E-br-beta-{nm}', f'branch: beta = {b!r} deg (the representable value next to the excluded end of (0, 180))', stub(150, 100, b, 0, 0.995e8), ['beta', '1e8'],
           'The open interval (0, 180) is the whole angle condition, but the developed branch of the nearest representable angle is longer than 1e8 mm (limit of the build).')
# chord-error cap: the saddle of a branch (beta 90, delta 0, R = 150 K, r = 100 K) and the large end of an eccentric reducer (200, 100, 300, 100) K are
# drawn with at most 46 080 segments; the largest chord error grows linearly with K and reaches 0.01 mm at K = 48 100 (branch) and 38 800 (eccentric)
for tag, K in (('inside', 45000), ('outside', 51000)):
    R_, r_ = 150 * K, 100 * K
    if tag == 'inside':
        case('B-br-chord-cap-inside', 'closed_form', f'Branch stub R={R_} r={r_} beta=90: chord error 9.4e-3 mm with 46 080 segments (just inside the 0.01 mm cap)', stub(R_, r_, 90, 0, 7e6),
             f'The chord error of the saddle with 46 080 uniform segments is 2.0787e-7 K = {2.0787e-7 * K:.3g} mm (reference polyline, 6.5 % under the 0.01 mm limit): accepted.',
             quantities={'branch': {'s_at_0': q(math.sqrt(R_ ** 2 - r_ ** 2), rel=1e-11), 's_at_90': q(R_, rel=1e-11), 'developed_width': q(2 * pi * r_)}})
    else:
        refuse('E-br-chord-cap-outside', f'Branch stub R={R_} r={r_} beta=90: chord error {2.0787e-7 * K:.3g} mm with 46 080 segments (just outside the 0.01 mm cap)', stub(R_, r_, 90, 0, 8e6), ['chord'],
               f'The chord error of the saddle with 46 080 uniform segments is {2.0787e-7 * K:.3g} mm (6 % over the 0.01 mm limit): refused (B2-005 point e, the cap is kept).')
for tag, K in (('inside', 36000), ('outside', 41000)):
    req = {'family': 'eccentric_reducer', 'r1': 200 * K, 'r2': 100 * K, 'h': 300 * K, 'e': 100 * K}
    sag = 2.5783e-7 * K
    if tag == 'inside':
        case('B-ecc-chord-cap-inside', 'closed_form', f'Eccentric reducer (200, 100, 300, 100) x {K}: chord error {sag:.3g} mm with 46 080 segments (just inside the 0.01 mm cap)', req,
             f'The chord error of the large end with 46 080 uniform segments is 2.5783e-7 K = {sag:.3g} mm (reference polyline, 7 % under the 0.01 mm limit): accepted.')
    else:
        refuse('E-ecc-chord-cap-outside', f'Eccentric reducer (200, 100, 300, 100) x {K}: chord error {sag:.3g} mm with 46 080 segments (just outside the 0.01 mm cap)', req, ['chord'],
               f'The chord error of the large end with 46 080 uniform segments is {sag:.3g} mm (5.7 % over the 0.01 mm limit): refused.')
# small branch: opening tends to an ellipse with semi-axes r/sin(beta) (axial) and r (circumferential)
for k, beta in enumerate((90, 45)):
    R, r = 1000, 1
    sb = math.sin(math.radians(beta))
    ell = f(4 * (r / sb) * mp.ellipe(1 - sb * sb))
    rq = hwb(R, r, beta, 0, 2200, 50, 1100 if beta == 90 else 1500)
    cx = 50 + R / math.tan(math.radians(beta))
    case(f'A-SMALL-BRANCH-{beta}', 'limit', f'Branch radius -> small (r/R = 1e-3), beta = {beta}: opening tends to an ellipse', rq,
         f'For r << R the header is locally a plane cut obliquely by the branch: an ellipse with semi-axes r/sin(beta) = {r / sb:.9g} along the axis and r = 1 around it. '
         f'Area -> pi r^2/sin(beta); perimeter -> 4 a E(1 - b^2/a^2). Curvature corrections are O((r/R)^2) = 1e-6 relative; tolerance 2e-4 relative. '
         f'The station is where the branch axis meets the header axis (work point), so the opening centre on the surface is at station + R/tan(beta) = {cx:.9g}, on the generator facing the branch (y = pi R).',
         measures={'header': {'hole_area': [pi * r * r / sb, 2e-4 * pi / sb], 'hole_perimeter': [ell, 2e-4 * ell], 'hole_w': [2 * r / sb, 2e-4], 'hole_h': [2 * r, 2e-4], 'hole_cy': [pi * R, 1e-6], 'hole_cx': [cx, 2e-3]}})

# =========================================================== limits / degenerate
base_cone = {'family': 'cone_frustum', 'r1': 200, 'r2': 100, 'h': 300}
for k, e in enumerate((0.0, 1e-9, 1e-6, 1e-3)):
    c9 = hp.eccentric(200, 100, 300, e)
    case(f'L-ECC-TO-CONE-{k}', 'limit', f'Eccentricity -> 0 (e = {e:g}) reduces to the concentric cone', {'family': 'eccentric_reducer', 'r1': 200, 'r2': 100, 'h': 300, 'e': e},
         'With e = 0 the oblique cone is the right cone CONE-01: developed angle = sector angle 2*pi/sqrt(10), area = 30000*pi*sqrt(10); for small e the quantities are continuous '
         '(values from the mpmath tier at this e).  The relation compares the pattern with the cone_frustum pattern of the same radii and height.',
         quantities={'reducer': {'developed_angle': q(c9['developed_angle'], rel=1e-11), 'rho_at_0': q(c9['rho_at_0']), 'rho_at_180': q(c9['rho_at_180']), 'area': q(c9['area'], rel=1e-9)}},
         relation={'type': 'equivalent_outline', 'other': base_cone, 'tol_rel': max(5e-5, 20 * e / 200)})
case('L-ECC-FLAT-SIDE', 'limit', 'Flat-sided reducer e = r1 - r2 (ECC-01): seam ruling parallel to the axis', {'family': 'eccentric_reducer', 'r1': 200, 'r2': 100, 'h': 300, 'e': 100},
     'With e = r1 - r2 the ruling at theta = 0 is parallel to the axis, so the seam has length h = 300 exactly and rho(0) = 600 (apex at height 600 above the large end).',
     quantities={'reducer': {'rho_at_0': q(600), 'rho_at_180': q(math.sqrt(400 ** 2 + 600 ** 2))}}, measures={'reducer': {'len_seam_a': q(300, rel=1e-12), 'len_seam_b': q(300, rel=1e-12)}})

# =========================================================== (c) high precision
for k, (r1, r2, h) in enumerate([(200, 100, 300), (300, 50, 100), (1000, 999, 50), (0.75, 0.125, 2.5), (123456.789, 98765.4321, 55555.5), (200, 199, 300)]):
    c = hp.right_cone(r1, r2, h)
    case(f'HP-CONE-{k + 1}', 'high_precision', f'Right cone r1={r1} r2={r2} h={h}: mpmath values', {'family': 'cone_frustum', 'r1': r1, 'r2': r2, 'h': h},
         'mpmath, 40 digits, from the 3D geometry: apex by similar triangles on a ruling; L = |C1 - apex|; sector angle = 2*pi*r1/L1 (rim length preserved) cross-checked against the '
         'arc length of the spherical image of the rulings; area = 3D surface integral, cross-checked against the sector area.  Values stored to 17 digits.',
         quantities={'reducer': {n: q(c[n], rel=2e-12) for n in ('slant_length', 'sin_half_angle', 'apex_radius_large', 'apex_radius_small', 'sector_angle', 'area', 'chord_large_arc')}},
         measures={'reducer': {'outline_area': q(c['area'], rel=1e-9), 'len_large_end': q(2 * mp.pi * mp.mpf(r1), rel=1e-11), 'len_small_end': q(2 * mp.pi * mp.mpf(r2), rel=1e-11), 'len_seam_a': q(c['slant_length'], rel=1e-11)}},
         hp_strings={n: mp.nstr(c[n], 20) for n in ('sector_angle', 'area', 'apex_radius_large')})
for k, (r1, r2, h, e) in enumerate([(200, 100, 300, 100), (200, 100, 300, 0.001), (200, 100, 300, 350), (300, 50, 100, 250), (500, 480, 60, 20), (10, 9, 500, 1), (1, 0.5, 0.01, 0.5), (2500, 100, 400, 2000)]):
    c = hp.eccentric(r1, r2, h, e)
    lens = {'len_large_end': [f(2 * mp.pi * r1), POLY * 2 * pi * r1], 'len_small_end': [f(2 * mp.pi * r2), POLY * 2 * pi * r2], 'len_seam_a': q(c['seam_length'], rel=1e-11), 'len_seam_b': q(c['seam_length'], rel=1e-11),
            'outline_area': [f(c['area']), 3 * POLY * f(c['area'])]}
    case(f'HP-ECC-{k + 1}', 'high_precision', f'Eccentric reducer r1={r1} r2={r2} h={h} e={e}: mpmath values', {'family': 'eccentric_reducer', 'r1': r1, 'r2': r2, 'h': h, 'e': e},
         'mpmath, 40 digits: developed angle = arc length of the spherical image of the rulings (tanh-sinh quadrature); area = 3D surface integral of |S_theta x S_lambda| '
         '(agrees with the polar integral to all digits).  The apex-free ODE route (reference/hp.py eccentric_apexfree_chord) reproduces the 0-180 deg chord of case 1 to 20 digits.',
         quantities={'reducer': {'developed_angle': q(c['developed_angle'], rel=2e-11), 'rho_at_0': q(c['rho_at_0'], rel=2e-12), 'rho_at_180': q(c['rho_at_180'], rel=2e-12), 'area': q(c['area'], rel=1e-9)}},
         measures={'reducer': lens}, hp_strings={n: mp.nstr(c[n], 20) for n in ('developed_angle', 'area', 'psi_180')})
for k, (R, r, beta, delta) in enumerate([(150, 100, 90, 0), (150, 100, 60, 0), (150, 80, 90, 40), (150, 100, 60, 20), (500, 120, 35, -200), (150, 149, 90, 0), (300, 100, 120, 150), (1000, 10, 15, 0)]):
    s_end = f(RF.Joint3D(R, r, beta, delta).s_of_phi(np.linspace(0, 2 * np.pi, 2001)).max()) + 0.5 * R
    j = RF.Joint3D(R, r, beta, delta)
    x, _ = j.header_coords(j.curve3d(np.linspace(0, 2 * np.pi, 20001)))
    L = math.ceil((x.max() - x.min()) + R)
    st = math.ceil(-x.min() + 0.4 * R)
    rq = hwb(R, r, beta, delta, L, st, round(s_end, 3))
    qq, mm = assembly_expect(rq, with_hp=True)
    case(f'HP-JOINT-{k + 1}', 'high_precision', f'Header R={R} with branch r={r}, beta={beta}, delta={delta}: mpmath integrals', rq,
         'Layout from the float reference tier (3D placement, bisection on each ruling); opening perimeter and area, saddle developed length and branch pattern area from mpmath '
         '(25 digits; ray/cylinder intersection in vector form, numerical differentiation inside tanh-sinh quadrature).  The developed opening perimeter, the developed saddle length '
         'and the 3D curve length agree with each other when contact and development radii coincide (isometry).  The product measures 720-segment polylines, hence the relative allowance.',
         quantities=qq, measures=mm)

# =========================================================== (d) piping assemblies across the supported range
n = 0
for R in (150.0,):
    for beta in (12, 30, 45, 60, 75, 90, 105, 135, 168):
        for ratio in (0.05, 0.3, 0.6, 0.9, 0.99):
            for dfrac in (0.0, 0.5, -0.5, 0.97):
                if dfrac == 0.97 and ratio in (0.05, 0.99):
                    continue
                r = R * ratio
                delta = round(dfrac * (R - r), 9)
                j = RF.Joint3D(R, r, beta, delta)
                ph = np.linspace(0, 2 * np.pi, 20001)
                s = j.s_of_phi(ph)
                x, _ = j.header_coords(j.branch_point(ph, s))
                L = float(math.ceil(x.max() - x.min() + 2 * R))
                st = float(math.ceil(-x.min() + (0.3 + 0.2 * (n % 5)) * R))
                rq = hwb(R, r, beta, delta, L, st, float(math.ceil(s.max() + 0.25 * R)), sref=('outside', 'inside', 'neutral', 'unknown')[n % 4])
                qq, mm = assembly_expect(rq)
                n += 1
                case(f'D-ASM-{n:03d}', 'assembly', f'Header R={R:g} / branch r={r:g}, beta={beta}, delta={delta:g}', rq,
                     'Layout expectation from the float reference tier: header and branch cylinders placed in 3D, intersection by bisection on each branch ruling, '
                     'opening extents on the plate in the frame x = station, y = arc from the seam opposite the branch; saddle heights on the branch.', quantities=qq, measures=mm)
# development radius different from contact radius, stations near the plate ends, large and small sizes
for k, (R, Rd, r, rd, beta, delta, scale, sref) in enumerate([(150, 145, 100, 96, 60, 20, 1, 'outside'), (150, 155, 60, 63, 40, -30, 1, 'inside'), (1500, 1490, 400, 394, 75, 300, 1, 'outside'), (150, 150, 100, 100, 60, 20, 0.01, 'outside'), (150, 150, 100, 100, 60, 20, 100, 'unknown')]):
    R, Rd, r, rd, delta = (v * scale for v in (R, Rd, r, rd, delta))
    j = RF.Joint3D(R, r, beta, delta)
    ph = np.linspace(0, 2 * np.pi, 20001)
    s = j.s_of_phi(ph)
    x, _ = j.header_coords(j.branch_point(ph, s))
    for pos in ('near_start', 'near_end'):
        L = float(round(x.max() - x.min() + 3 * R, 6))
        st = float(round(-x.min() + 0.01 * R, 6)) if pos == 'near_start' else float(round(L - x.max() - 0.01 * R, 6))
        rq = hwb(R, r, beta, delta, L, st, float(round(s.max() + 0.25 * R, 6)), R_dev=Rd, r_dev=rd, sref=sref)
        qq, mm = assembly_expect(rq)
        case(f'D-ASM-DEV-{k + 1}-{pos}', 'assembly', f'R={R:g} (dev {Rd:g}), r={r:g} (dev {rd:g}), beta={beta}, delta={delta:g}, opening {pos.replace("_", " ")} of the plate', rq,
             'As the D-ASM cases; development radii differ from contact radii (angles from the contact surfaces, arc lengths on the development radius); opening 0.01 R from a plate end.',
             quantities=qq, measures=mm)

# =========================================================== (b) symmetry / invariance
asm = hwb(150, 100, 60, 20, 1200, 500, 400)
case('S-MIRROR-DELTA', 'symmetry', 'Mirror: delta -> -delta', asm,
     'Reflecting the assembly in the plane containing the header axis and the branch direction changes the sign of delta only. On the plate the opening is reflected about the '
     'line y = W/2 (the generator facing the branch); on the branch the saddle is reflected about phi = 90 deg (u -> W/2 - u, cyclically).',
     relation={'type': 'mirror_delta'})
case('S-MIRROR-BETA', 'symmetry', 'Mirror: beta -> 180 - beta', asm,
     'Reflecting the assembly in the plane through the station perpendicular to the header axis turns beta into 180 - beta. On the plate the opening is reflected about x = station; '
     'on the branch the saddle is reflected u -> W - u.', relation={'type': 'mirror_beta'})
case('S-STATION-SHIFT', 'symmetry', 'Station shift translates the opening only', asm,
     'Moving the branch 137.5 mm along the header translates the opening by (137.5, 0); the plate outline and the whole branch pattern are unchanged.', relation={'type': 'station_shift', 'd': 137.5})
case('S-STUB-EQUALS-ASSEMBLY-BRANCH', 'symmetry', 'Branch stub alone equals the branch part of the assembly', asm,
     'The branch pattern depends only on (R, r, r_dev, beta, delta, s_end): the stand-alone branch_stub result must be identical to the branch part of header_with_branch.', relation={'type': 'stub_equals_branch'})
bases = {'cyl': {'family': 'cylinder_shell', 'r_dev': 500, 'length': 1000}, 'cone': {'family': 'cone_frustum', 'r1': 200, 'r2': 100, 'h': 300},
         'ecc': {'family': 'eccentric_reducer', 'r1': 200, 'r2': 100, 'h': 300, 'e': 100}, 'stub': stub(150, 100, 60, 20, 400), 'asm': asm}
for name, b in bases.items():
    for kk in (2.0, 0.5, 1024.0, 25.4, 1e-3, 1e3, 1 / 3):
        case(f'S-SCALE-{name}-{kk:.6g}', 'symmetry', f'Uniform scale x{kk:.6g} of {b["family"]}', b,
             'Scaling every length by k scales every pattern coordinate by k (lengths x k, areas x k^2); angles unchanged. For k a power of two the result should be '
             'identical to rounding; otherwise agreement to ~1e-12 relative.', relation={'type': 'scale', 'k': kk})

# =========================================================== (e) refusals
valid = {'cylinder_shell': ({'family': 'cylinder_shell', 'r_dev': 500, 'length': 1000}, ['r_dev', 'length']),
         'cone_frustum': (base_cone, ['r1', 'r2', 'h']),
         'eccentric_reducer': ({'family': 'eccentric_reducer', 'r1': 200, 'r2': 100, 'h': 300, 'e': 100}, ['r1', 'r2', 'h']),
         'branch_stub': (stub(150, 100, 60, 20, 400), ['header_radius', 'branch_radius', 'branch_dev_radius', 's_end']),
         'header_with_branch': (asm, ['header_radius', 'header_dev_radius', 'header_length', 'branch_radius', 'branch_dev_radius', 's_end'])}
for fam, (base, pos) in valid.items():
    for p in pos:
        for label, v in (('zero', 0.0), ('negative', -abs(base[p])), ('negative-zero', -0.0)):
            refuse(f'E-{fam}-{p}-{label}', f'{fam}: {p} = {v}', dict(base, **{p: v}), [p, 'radius', '> 0'], f'{p} must be a finite positive length; {v} describes no part.')
    for p in [k for k in base if k not in ('family', 'surface_reference')]:
        for label, tok in (('NaN', 'NaN'), ('Infinity', 'Infinity'), ('1e999', '1e999'), ('string', '"12"'), ('null', 'null')):
            body = json.dumps(base).replace(f'"{p}": {json.dumps(base[p])}', f'"{p}": {tok}')
            assert tok in body
            refuse(f'E-{fam}-{p}-{label}', f'{fam}: {p} = {tok}', None, [], f'{tok} is not a finite JSON number: the request must be rejected at the boundary (HTTP 400), never computed.', kind='malformed', raw_body=body)
        refuse(f'E-{fam}-{p}-missing', f'{fam}: {p} missing', {k: v for k, v in base.items() if k != p}, [p], 'A missing parameter must not be defaulted.', kind='malformed')
    for p in [k for k in base if k not in ('family', 'surface_reference')]:
        refuse(f'E-{fam}-{p}-1e308', f'{fam}: {p} = 1e308', dict(base, **{p: 1e308}), [],
               'A finite but absurd magnitude: products of lengths overflow binary64. The only acceptable outcomes are a refusal or a finite, correct pattern; a pattern containing '
               'non-finite numbers is a defect. Expected here: refusal.')
refuse('E-family-unknown', 'unknown family', {'family': 'teapot'}, [], 'Unknown part family.', kind='malformed')
refuse('E-cone-r1-eq-r2', 'cone: r1 = r2', dict(base_cone, r2=200), ['r1', 'r2', 'cylinder'], 'Equal radii is a cylinder, not a cone (apex at infinity).')
refuse('E-cone-r1-lt-r2', 'cone: r1 < r2', dict(base_cone, r1=100, r2=200), ['r1', 'r2'], 'The family is defined with the large end first (Spec 02 4.2: r1 > r2).')
# B2-005 (engine 0.4.2): CONE-02 (200, 199.9, 300) has sin(alpha) = 3.3e-4 and an apex radius of 6e5 mm; it is ACCEPTED (case A-CONE-SLENDER below).
# A cone is refused only by its apex radius r1/sin(alpha) > 1e8 mm, or by a seam gap l2 (2 pi - Phi) <= 1e-9 mm at the small end.
refuse('E-cone-near-cyl', 'cone: near-cylindrical, apex radius above 1e8 mm (CONE-02 scaled)', {'family': 'cone_frustum', 'r1': 1e6, 'r2': 1e6 - 1, 'h': 1e6}, ['apex', 'radius'], 'sin(alpha) = 1e-6, apex radius r1/sin(alpha) = 1e12 mm exceeds the 1e8 mm developed-size limit of the build.')
refuse('E-cone-seam-gap', 'cone: seam gap below the closure resolution', {'family': 'cone_frustum', 'r1': 300, 'r2': 50, 'h': 1e-5}, ['seam', 'closure'], '1 - sin(alpha) = h^2 / (2 * 250^2) = 8e-16: the seam gap l2 (2 pi - Phi) = 50 * 2 pi * 8e-16 = 2.5e-13 mm is below the 1e-9 mm closure resolution of the profile check.')
refuse('E-ecc-r1-eq-r2', 'eccentric: r1 = r2 (oblique cylinder)', {'family': 'eccentric_reducer', 'r1': 100, 'r2': 100, 'h': 100, 'e': 100}, ['r1', 'r2', 'cylinder'], 'OBLCYL-01 geometry: declared not implemented.')
refuse('E-ecc-e-negative', 'eccentric: e < 0', {'family': 'eccentric_reducer', 'r1': 200, 'r2': 100, 'h': 300, 'e': -100}, ['e', 'offset'], 'The offset is a magnitude; its direction is fixed by the seam datum.')
refuse('E-ecc-near-cyl', 'eccentric: apex radius above 1e8 mm', {'family': 'eccentric_reducer', 'r1': 1e6, 'r2': 1e6 - 1, 'h': 1e6, 'e': 50}, ['apex', 'radius'], 'The apex lies about 1e12 mm from the large end: above the 1e8 mm developed-size limit of the build.')
refuse('E-ecc-seam-gap', 'eccentric: seam gap below the closure resolution', {'family': 'eccentric_reducer', 'r1': 300, 'r2': 50, 'h': 1e-5, 'e': 0}, ['seam', 'closure'], 'Nearly flat: the angle defect 2 pi - Psi is about 5e-15, so the seam gap at the small end is far below the 1e-9 mm closure resolution.')
st = stub(150, 100, 60, 0, 400)
refuse('E-br-equal-tee', 'branch: r = R (BR-05 equal tee)', dict(st, branch_radius=150, branch_dev_radius=150, beta_deg=90), ['delta', 'tangent', 'equal'], 'Equal-diameter tee: cornered saddle, declared not implemented.')
refuse('E-br-r-gt-R', 'branch: r > R', dict(st, branch_radius=200, branch_dev_radius=200), ['delta', 'radius'], 'Branch larger than the header: no full saddle exists.')
refuse('E-br-partial', 'branch: |delta| + r > R (BR-02)', stub(150, 100, 90, 60, 400), ['delta'], 'Partial intersection: different topology.')
refuse('E-br-partial-neg', 'branch: |delta| + r > R, delta negative', stub(150, 100, 90, -60, 400), ['delta'], 'Partial intersection on the other side.')
refuse('E-br-tangent', 'branch: |delta| + r = R exactly', stub(150, 100, 90, 50, 400), ['delta', 'tangent'], 'Tangent rulings: declared not implemented.')
for b in (0, 180, -30, 200, 360, 540 + 60):
    refuse(f'E-br-beta-{b}', f'branch: beta = {b} deg', dict(st, beta_deg=b), ['beta', 'angle'], 'Angle between axes outside the open interval (0, 180) degrees (B2-005: the former 10..170 deg limit is gone, 5 and 175 deg are accepted, see A-BR-GRAZE-*). 600 deg is not an angle between two lines.')
refuse('E-br-beta-size', 'branch: developed branch longer than 1e8 mm', dict(st, beta_deg=1e-6, s_end=1e8), ['beta', 'angle', '1e8'], 'beta = 1e-6 deg: s_max = (R + r cos(beta))/sin(beta) = 1.4e10 mm exceeds the 1e8 mm developed-size limit.')
refuse('E-br-s_end-inside', 'branch: square end inside the saddle', dict(st, s_end=100), ['s_end', 'saddle'], 's_end below the highest saddle point: the stub would have no closed square end.')
refuse('E-asm-off-start', 'assembly: opening off the plate at station 0 end', dict(asm, branch_station=20), ['station', 'opening', 'shell'], 'Station 20: the opening (centre at station + R/tan(beta) = 106.6, reaching 30 mm before it) would cross the plate end x = 0.')
refuse('E-asm-off-end', 'assembly: opening off the plate at the far end', dict(asm, branch_station=1150), ['station', 'opening', 'shell'], 'The opening would cross the plate end x = L.')
refuse('E-asm-station-negative', 'assembly: negative station', dict(asm, branch_station=-600), ['station', 'opening'], 'Branch not on the plate at all.')
refuse('E-asm-station-beyond', 'assembly: station beyond the header', dict(asm, branch_station=5000), ['station', 'opening'], 'Branch not on the plate at all.')
refuse('E-asm-plate-too-short', 'assembly: plate shorter than the opening', dict(asm, header_length=150, branch_station=75), ['station', 'opening', 'length'], 'Opening longer than the plate.')
refuse('E-asm-sref-invalid', 'assembly: undeclared surface reference', dict(asm, surface_reference='whatever'), ['surface_reference'], 'The surface reference must be one of the declared values.')
refuse('E-asm-equal-tee', 'assembly: equal tee', dict(asm, branch_radius=150, branch_dev_radius=150, beta_deg=90, delta=0), ['delta', 'branch'], 'Equal tee not implemented.')
refuse('E-asm-partial', 'assembly: |delta| + r > R', dict(asm, delta=60), ['delta', 'branch'], 'Partial intersection.')
refuse('E-asm-beta-0', 'assembly: beta = 0', dict(asm, beta_deg=0), ['beta', 'angle', 'branch'], 'Parallel axes.')
refuse('E-unknown-field', 'unknown request field', dict(base_cone, thickness=12), ['unknown', 'thickness'], 'A field the family does not have must not be ignored silently (contract since engine 0.2.0).', kind='malformed')
for sref, Rd, rd in (('outside', 155, 100), ('outside', 150, 104), ('inside', 145, 100), ('inside', 150, 97), ('neutral', 149, 100), ('neutral', 150, 100.5)):
    refuse(f'E-asm-sref-{sref}-{Rd}-{rd}', f'assembly: development radii impossible for contact radii declared {sref} (R 150 / dev {Rd}, r 100 / dev {rd})', dict(asm, header_dev_radius=Rd, branch_dev_radius=rd, surface_reference=sref),
           ['surface_reference', 'inconsistent'], 'Spec 02 6.1: the development surface lies inside the wall (r_inside <= r_dev <= r_outside). A development radius beyond the declared contact surface means a negative wall thickness.')
for fam, (base, pos) in valid.items():
    refuse(f'E-{fam}-{pos[0]}-2e8', f'{fam}: {pos[0]} = 2e8 mm', dict(base, **{pos[0]: 2e8}), ['magnitude', 'exceeds'], 'Declared implementation limit of the build: magnitudes above 1e8 mm are refused.')
    refuse(f'E-{fam}-{pos[-1]}-5e-7', f'{fam}: {pos[-1]} = 5e-7 mm', dict(base, **{pos[-1]: 5e-7}), ['below', 'smallest'], 'Declared implementation limit of the build: positive dimensions below 1e-6 mm are refused.')
NOTES = {'not_constructible': [{'id': 'E-asm-opening-crosses-seam', 'reason': 'The product fixes the longitudinal seam diametrically opposite the branch and accepts no seam position input; '
                                'the opening spans |omega| < 90 deg for every supported joint, so an opening crossing the seam cannot be requested. The validator instead measures the '
                                'opening-to-seam clearance on every accepted assembly (checks/compare.py, opening_seam_clearance).'}]}

for c in CASES:
    with open(os.path.join(OUT, c['id'] + '.json'), 'w') as fh:
        json.dump(c, fh, indent=1)
with open(os.path.join(HERE, 'INDEX.json'), 'w') as fh:
    groups = {}
    for c in CASES:
        groups.setdefault(c['group'], []).append(c['id'])
    json.dump({'count': len(CASES), 'groups': {g: len(v) for g, v in groups.items()}, 'ids': groups, 'notes': NOTES}, fh, indent=1)
print(len(CASES), {g: len(v) for g, v in groups.items()})
