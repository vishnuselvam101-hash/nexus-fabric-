#!/usr/bin/env python3
"""Tests of the validator itself: take a correct product result, plant a known fault in the geometry
(as the validator sees it), and require the comparison to report it.  Fault 1 is Defect 002 exactly:
the plate outline drawn with the circumference along x while the opening keeps the axis along x.

  python3 validation/seeded_faults.py --url http://127.0.0.1:8802 [--user U --password P]
Exit status 1 if any planted fault goes unnoticed."""
import argparse
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import shapely
from checks.client import Client, USER, PASSWORD
from checks.product import Job, Part
from checks.compare import cmp_header, cmp_branch, compare_job, dxf_roundtrip, declared_side

REQ = {'family': 'header_with_branch', 'header_radius': 150, 'header_dev_radius': 150, 'header_length': 1200, 'branch_station': 500, 'branch_radius': 100,
       'branch_dev_radius': 100, 'beta_deg': 60, 'delta': 20, 's_end': 400, 'surface_reference': 'outside'}


def setpts(c, pts):
    c.pts = np.asarray(pts, float)
    c._ls = shapely.LineString(c.pts)


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


def run(cl):
    lines = []
    W, L = 2 * math.pi * 150, 1200.0

    def fresh():
        return Job(cl, cl.calculate(REQ))

    def defect_002(j):
        h = j.parts['header']
        for n, (p, q) in {'seam_a': ((0, 0), (W, 0)), 'top': ((W, 0), (W, L)), 'seam_b': ((W, L), (0, L)), 'bottom': ((0, L), (0, 0))}.items():
            setpts(h.curve(n), [p, q])

    def hole_mirrored(j):
        c = j.parts['header'].curve('branch_opening')
        setpts(c, np.stack([c.pts[:, 0], W - c.pts[:, 1]], 1))

    def hole_shifted(j):
        c = j.parts['header'].curve('branch_opening')
        setpts(c, c.pts + [0.05, 0.0])

    def hole_wrong_station_origin(j):
        c = j.parts['header'].curve('branch_opening')
        setpts(c, np.stack([L - c.pts[:, 0], c.pts[:, 1]], 1))

    def hole_on_seam_side(j):
        c = j.parts['header'].curve('branch_opening')
        setpts(c, np.stack([c.pts[:, 0], np.mod(c.pts[:, 1] + W / 2, W)], 1))

    def saddle_wrong_radius(j):
        c = j.parts['branch'].curve('saddle_cut')
        setpts(c, np.stack([c.pts[:, 0], c.pts[:, 1] * 1.0005], 1))

    def saddle_seam_shifted(j):
        c = j.parts['branch'].curve('saddle_cut')
        p = c.pts[:-1]
        k = len(p) // 4
        v = np.roll(p[:, 1], k)
        setpts(c, np.stack([c.pts[:, 0], np.append(v, v[0])], 1))

    def saddle_mirrored(j):
        c = j.parts['branch'].curve('saddle_cut')
        Wb = 2 * math.pi * 100
        setpts(c, np.stack([Wb - c.pts[:, 0], c.pts[:, 1]], 1)[::-1])

    faults = [('Defect D1: branch saddle mirrored (drawn from the other face) under the same declared side', saddle_mirrored), ('Defect 002: outline with circumference along x, opening unchanged', defect_002), ('opening mirrored about the plate centre line', hole_mirrored),
              ('opening shifted 0.05 mm along the axis', hole_shifted), ('opening measured from the wrong plate end', hole_wrong_station_origin),
              ('opening moved half a turn (onto the seam side)', hole_on_seam_side), ('saddle heights scaled by 1.0005', saddle_wrong_radius), ('branch seam moved a quarter turn', saddle_seam_shifted)]
    base = compare_job(REQ, fresh())
    ok = not base['issues']
    lines.append(('ok  ' if ok else 'FAIL') + ' unmodified result has no geometry issue' + ('' if ok else ': ' + base['issues'][0][:200]))
    for name, plant in faults:
        j = fresh()
        plant(j)
        issues = []                      # geometry comparison only (the DXF/JSON round trip is not what is tested here)
        hr, _, _ = cmp_header(REQ, j.parts['header'], issues)
        br, _, _ = cmp_branch(REQ, j.parts['branch'], issues)
        declared_side(j.parts['branch'], br, issues, [], True)
        # a mirrored part is a correct drawing of the same part seen from the other face: it is wrong only against
        # the marking side the result declares
        declared_side(j.parts['header'], hr, issues, [], True)
        caught = bool(issues)
        ok &= caught
        lines.append(('ok  ' if caught else 'MISS') + f' {name}: ' + (issues[0][:170] if caught else 'not detected'))
    # ---- tampering with the DXF file itself: the round-trip check must fail whenever CUT geometry changes,
    #      and must not be distracted by the non-cutting NF_NOT_RELEASED marking
    j = fresh()
    h = j.parts['header']
    text = h.dxf_text
    v = h.curve('branch_opening').pts[5]
    old = repr(float(v[0]))
    extra = lambda layer: text.replace('  0\nENDSEC\n  0\nEOF', f'  0\nLINE\n  8\n{layer}\n 10\n1.0\n 20\n1.0\n 11\n50.0\n 21\n60.0\n  0\nENDSEC\n  0\nEOF')
    tamper = [('one vertex of the opening moved by 1e-6 mm in the DXF', text.replace(old, repr(float(v[0]) + 1e-6), 1) if old in text else None),
              ('extra LINE added on CUT_INNER', extra('CUT_INNER')), ('extra LINE hidden on the NF_NOT_RELEASED layer', extra('NF_NOT_RELEASED')), ('extra LINE on a new layer', extra('SCRIBE')),
              ('opening moved to the CUT_OUTER layer', text.replace('CUT_INNER', 'CUT_OUTER'))]
    for name, t in tamper:
        if t is None or t == text:
            ok = False
            lines.append(f'MISS {name}: could not plant the fault (DXF text layout changed)')
            continue
        issues = []
        try:
            dxf_roundtrip(Part(h.view, t), issues)
        except Exception as ex:
            issues.append(f'reader failed: {ex}')
        ok &= bool(issues)
        lines.append(('ok  ' if issues else 'MISS') + f' DXF: {name}: ' + (issues[0][:150] if issues else 'not detected'))
    return ok, lines


if __name__ == '__main__':
    main()
