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
from checks.product import Job
from checks.compare import cmp_header, cmp_branch, compare_job

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
    a = ap.parse_args()
    cl = Client(a.url).connect(a.user, a.password)
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

    faults = [('Defect 002: outline with circumference along x, opening unchanged', defect_002), ('opening mirrored about the plate centre line', hole_mirrored),
              ('opening shifted 0.05 mm along the axis', hole_shifted), ('opening measured from the wrong plate end', hole_wrong_station_origin),
              ('opening moved half a turn (onto the seam side)', hole_on_seam_side), ('saddle heights scaled by 1.0005', saddle_wrong_radius), ('branch seam moved a quarter turn', saddle_seam_shifted)]
    base = compare_job(REQ, fresh())
    ok = not base['issues']
    print(('ok  ' if ok else 'FAIL') + ' unmodified result has no geometry issue')
    for name, plant in faults:
        j = fresh()
        plant(j)
        issues = []                      # geometry comparison only (the DXF/JSON round trip is not what is tested here)
        hr, _, _ = cmp_header(REQ, j.parts['header'], issues)
        cmp_branch(REQ, j.parts['branch'], issues)
        if plant is hole_mirrored and not issues and hr.get('views_matching') == ['inside']:
            # a mirrored part is a legitimate drawing of the same part seen from the other face; because the product
            # declares no marking side this cannot be called wrong, only reported: the detected side must flip
            issues.append('reported as drawn from the INSIDE (was outside) - detectable only as a change of side, since no marking side is declared')
        caught = bool(issues)
        ok &= caught
        print(('ok  ' if caught else 'MISS') + f' {name}: ' + (issues[0][:170] if caught else 'not detected'))
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
