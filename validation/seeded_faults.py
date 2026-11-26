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
from checks.compare import cmp_header, cmp_branch, cmp_cone, cmp_eccentric, compare_job, seam_resolution_check, dxf_roundtrip, declared_side, compare_scaled, scale_request, rel_tol, unexpected_holds
from reference import families as RF
from stability import _asm

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


class UnbuiltPart(RuntimeError):
    """The job could not build a part a seeded fault needs (Job.unbuilt): one explicit FAIL line, never a crash."""


def _guarded(label, stage, cl):
    """A stage that cannot run to its end is one explicit FAIL line: the run goes on and no pass is reported for it."""
    try:
        return stage(cl)
    except Exception as ex:
        return False, [f'FAIL {label}: {type(ex).__name__}: {ex}']


def run(cl):
    ok, lines = _guarded('header and DXF faults', header_faults, cl)
    for label, stage in (('near-tangent and scale faults', near_tangent_and_scale), ('seam faults', seam_faults),
                         ('regime faults', regime_faults), ('verdict faults', verdict_faults)):
        ok_s, lines_s = _guarded(label, stage, cl)
        ok &= ok_s
        lines += lines_s
    lines.append('SUMMARY: verdict ' + ('PASS' if ok else 'FAIL'))
    return ok, lines


def header_faults(cl):
    lines = []
    W, L = 2 * math.pi * 150, 1200.0

    def fresh():
        j = Job(cl, cl.calculate(REQ))
        if j.unbuilt:
            raise UnbuiltPart(f'the result has a part that cannot be built: {j.unbuilt}')
        return j

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
    if text is None:
        # a withheld header has no file to tamper with: a clear FAIL, never an exception (and never a pass)
        ok = False
        lines.append('FAIL DXF tampering: the header DXF is withheld, no file to plant the faults into')
        tamper = []
    else:
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


def verdict_faults(cl):
    """Task 1.2 (review; plan rule A9): the product's own verdicts are split in checks/compare.py. Planted on a correct
    result's checks: a FAIL must raise PRODUCT_CHECK_NOT_PASS (a defect); an INCONCLUSIVE must raise
    PRODUCT_CHECK_INCONCLUSIVE and must NOT raise the defect; a verdict that is not one of the three must still raise
    PRODUCT_CHECK_NOT_PASS. The unmodified result must raise neither code. Derivation: checks/compare.py."""
    out, ok = [], True

    def fresh():
        return Job(cl, cl.calculate(REQ))

    def codes(job):
        return {f['code'] for f in compare_job(REQ, job)['findings']}

    base = codes(fresh())
    clean = not ({'PRODUCT_CHECK_NOT_PASS', 'PRODUCT_CHECK_INCONCLUSIVE'} & base)
    ok &= clean
    out.append(('ok  ' if clean else 'FAIL') + ' unmodified result raises no product-check finding' + ('' if clean else f': {sorted(base)}'))
    for name, verdict, want, forbid in [('product check FAIL', 'FAIL', 'PRODUCT_CHECK_NOT_PASS', 'PRODUCT_CHECK_INCONCLUSIVE'),
                                        ('product check INCONCLUSIVE', 'INCONCLUSIVE', 'PRODUCT_CHECK_INCONCLUSIVE', 'PRODUCT_CHECK_NOT_PASS'),
                                        ('product check with an unknown verdict', 'MAYBE', 'PRODUCT_CHECK_NOT_PASS', 'PRODUCT_CHECK_INCONCLUSIVE')]:
        j = fresh()
        checks = j.result.get('checks') or []
        if not checks:
            ok = False
            out.append(f'MISS {name}: the result has no product checks to plant on')
            continue
        checks[0]['verdict'] = verdict
        got = codes(j)
        caught = want in got and forbid not in got
        ok &= caught
        out.append(('ok  ' if caught else 'MISS') + f' {name} ({checks[0]["name"]}): ' + (f'{want} raised' if caught else f'not detected: {sorted(got)}'))
    return ok, out


def regime_faults(cl):
    """Engine 0.4.3, work plan task 1.1: faults planted in results INSIDE the regime where the product's verification is not
    guaranteed (checks/oracle.py branch_conditioning >= 1): a stub at 0.003 deg (ratio 39) and a header with branch at
    179.99 deg (ratio 1.11). The product accepts both (its verification confirms them); the validator must still catch a
    wrong pattern there. (That the product's own verification rejects such a pattern instead of refusing the angle as
    branch_angle_not_verifiable is tested in crates/uem, test task_1_1_a_wrong_pattern_inside_the_regime_is_rejected_not_refused.)"""
    lines, ok = [], True
    stub_req = {'family': 'branch_stub', 'header_radius': 150, 'branch_radius': 100, 'branch_dev_radius': 100, 'beta_deg': 0.003, 'delta': 0, 's_end': 5.2e6}
    asm_req = _asm(150.0, 100.0, 179.99, 0.0)

    def shift(part, curve, dx, dy):
        def plant(j):
            c = j.parts[part].curve(curve)
            setpts(c, c.pts + [dx, dy])
        return plant

    def scale_y(k):
        def plant(j):
            c = j.parts['branch'].curve('saddle_cut')
            setpts(c, np.stack([c.pts[:, 0], c.pts[:, 1] * k], 1))
        return plant

    plan = [(stub_req, 'stub 0.003 deg: saddle cut 0.05 mm too high', shift('branch', 'saddle_cut', 0.0, 0.05)),
            (stub_req, 'stub 0.003 deg: saddle heights scaled by 1 + 1e-7', scale_y(1 + 1e-7)),
            (asm_req, 'header with branch 179.99 deg: opening shifted 0.05 mm along the axis', shift('header', 'branch_opening', 0.05, 0.0)),
            (asm_req, 'header with branch 179.99 deg: saddle cut 0.05 mm too high', shift('branch', 'saddle_cut', 0.0, 0.05))]
    for req in (stub_req, asm_req):
        j = Job(cl, cl.calculate(req))
        base = compare_job(req, j) if j.accepted else {'issues': ['not accepted: ' + j.refusal_text()[:150]]}
        good = j.accepted and not base['issues']
        ok &= good
        lines.append(('ok  ' if good else 'FAIL') + f" task 1.1 regime, {req['family']} beta {req['beta_deg']}: unmodified result accepted, no geometry issue" + ('' if good else ': ' + base['issues'][0][:160]))
    for req, name, plant in plan:
        j = Job(cl, cl.calculate(req))
        if not j.accepted:
            ok = False
            lines.append(f'MISS task 1.1 regime, {name}: could not plant (result not accepted)')
            continue
        plant(j)
        issues = []
        if 'header' in j.parts:
            cmp_header(req, j.parts['header'], issues)
        cmp_branch(req, j.parts['branch'], issues)
        ok &= bool(issues)
        lines.append(('ok  ' if issues else 'MISS') + f' task 1.1 regime, {name}: ' + (issues[0][:150] if issues else 'not detected'))
    return ok, lines


def seam_faults(cl):
    """Engine 0.4.2 Q2(b): plant faults in the seam-resolution quantities and the G11 verdict of a correct result and require
    checks.compare.seam_resolution_check to report them (it had never been shown to fire).
      reducer, e = 0, seam gap 3e-3 mm (below 2 eps_rep): the engine says G11 INCONCLUSIVE; plant G11 PASS (an unresolved seam passed);
      the same reducer with the reported seam_gap scaled x100 (a gap above the resolution that the DXF does not have);
      right cone, seam gap 2.5e-7 mm: plant the pre-fix state of a large cone (eps_num of a 1e7 mm part, resolution left at the bare
      1e-9 mm) - the derived floor max(1e-9, 2 eps_num) must be demanded of the product."""
    lines, ok = [], True
    red = {'family': 'eccentric_reducer', 'r1': 300, 'r2': 50, 'h': 1.0925510388562152, 'e': 0}
    cone = {'family': 'cone_frustum', 'r1': 300, 'r2': 50, 'h': 0.01}

    def seam_issues(req, plant):
        j = Job(cl, cl.calculate(req))
        if 'reducer' not in j.parts:
            # the reducer cannot be built from the result (e.g. withheld with an arc, whose exact geometry the JSON does
            # not state): the seam faults cannot be planted, so the case is an explicit FAIL, never a pass
            return None, [f'the reducer cannot be built from the result: {j.unbuilt.get("reducer", j.refusal_text()[:120] or "no part")}']
        if 'reducer' in j.withheld:
            # The reducer file is withheld (its cut-geometry checks are INCONCLUSIVE by design).  These seam faults
            # plant into the JSON/report, not the file, so they need only the geometry report and seam_resolution_check,
            # neither of which reads the file.  Build that report straight from the family comparator on the part the
            # Job constructed from the JSON curves, so the faults stay applicable without the served DXF.
            issues = []
            cmp = cmp_eccentric if req['family'] == 'eccentric_reducer' else cmp_cone
            pr, _ = cmp(req, j.parts['reducer'], issues)
            issues += unexpected_holds(j)     # the withheld control is an unexpected hold unless the case declares it
        else:
            pre = compare_job(req, j)
            pr = pre['parts']['reducer']
            issues = pre['issues']
        if issues:
            return None, issues
        plant(j)
        out = []
        seam_resolution_check(req['family'], 'reducer', pr, j, out)
        return True, out

    def g11_pass(j):
        for g in j.result['gates']:
            if g['id'] == 'G11':
                g['result'] = 'PASS'

    def gap_x100(j):
        for p in j.result['patterns']:
            p['quantities']['seam_gap'] *= 100

    def cone_old_floor(j):
        for p in j.result['patterns']:
            p['eps_num'] = 3.0e-6        # an apex radius near 1e8 mm: 2 eps_num = 6e-6 mm, above the 2.5e-7 mm seam gap

    def cone_old_floor_g11_pass(j):
        cone_old_floor(j)
        g11_pass(j)
    # Every one of these faults plants into the JSON/report (the G11 gate verdict, the reported seam_gap, eps_num),
    # never into the DXF file, so each stays applicable while the eccentric reducer file is withheld: seam_issues
    # builds the geometry from the JSON curves.  None needs the released reducer file, so none is moved to another
    # case and none is not applicable: no fault is dropped.
    for name, req, plant in (('reducer: seam gap below 2 eps_rep reported with G11 PASS', red, g11_pass),
                             ('reducer: reported seam_gap 100 x the real one', red, gap_x100),
                             ('cone: resolution left at the bare 1e-9 mm where 2 eps_num is far above it', cone, cone_old_floor),
                             ('cone: seam gap below max(1e-9, 2 eps_num) reported with G11 PASS', cone, cone_old_floor_g11_pass)):
        base, issues = seam_issues(req, lambda j: None)
        if base is None:
            ok = False
            lines.append(f'MISS {name}: unmodified result already has issues: {issues[0][:150]}')
            continue
        if issues:
            ok = False
            lines.append(f'MISS {name}: unmodified result has a seam issue: {issues[0][:150]}')
            continue
        _, issues = seam_issues(req, plant)
        ok &= bool(issues)
        lines.append(('ok  ' if issues else 'MISS') + f' seam: {name}: ' + (issues[0][:170] if issues else 'not detected'))
    return ok, lines


def _scale_entities(job, f, kinds=('line', 'arc', 'polyline')):
    """Plant a geometry fault: the entities of the given kinds multiplied by f (drawn points and arc parameters alike)."""
    for part in job.parts.values():
        for e in part.ents:
            if e.kind not in kinds:
                continue
            if e.kind == 'arc':
                e.c = tuple(np.asarray(e.c, float) * f)
                e.r = e.r * f
                e.pts = e.arc_points(2)
            else:
                e.pts = e.pts * f
                e._ls = shapely.LineString(e.pts) if len(e.pts) > 1 else None


def near_tangent_and_scale(cl):
    """Seeded faults for the two exact tests: the header opening of a near-tangent joint (N-tangent-d-b90-g2e-06, where
    the old nearest-point search failed) and the scale relation at k = 25.4 (the scaled pattern is checked exactly).
    Same line format as run().  Returns (ok, lines)."""
    out, ok = [], True

    def report(name, caught, msg):
        nonlocal ok
        ok &= bool(caught)
        out.append(('ok  ' if caught else 'MISS') + f' {name}: ' + (msg[:170] if caught else 'not detected'))

    NT = _asm(150.0, 75.0, 90, 75.0 - 2e-6 * 150.0)           # the request of case N-tangent-d-b90-g2e-06
    wt = 2 * math.pi * NT['header_dev_radius']
    ctrl = Job(cl, cl.calculate(NT))
    h = ctrl.parts['header']
    tol = rel_tol(max(h.extent(), wt, NT['header_length'], NT['header_radius']))      # the scale used by cmp_header
    issues = []
    hr, _, ev = cmp_header(NT, h, issues)
    br, _, _ = cmp_branch(NT, ctrl.parts['branch'], issues)
    declared_side(ctrl.parts['branch'], br, issues, [], True)
    declared_side(h, hr, issues, [], True)
    issues += unexpected_holds(ctrl)          # the unmodified control is not held by the AMOEBA digest
    m = [e for e in ev if e['match'] and e['natural']]
    ok &= (not issues) and bool(m)
    out.append(('ok  ' if (not issues and m) else 'FAIL') + ' control: unmodified near-tangent joint has no geometry issue' + ('' if (not issues and m) else ': ' + (issues[0][:170] if issues else 'no frame mapping')))
    if not m:
        return ok, out
    R0, t0 = m[0]['R'], m[0]['t']                          # the frame the comparison finds for the unmodified part

    def opening(j):
        return j.parts['header'].curve('branch_opening')

    def off_curve(j):
        c = opening(j)
        p = c.pts.copy()
        k = len(p) // 3
        tv = p[k + 1] - p[k - 1]
        p[k] = p[k] + 10 * tol * np.array([-tv[1], tv[0]]) / np.linalg.norm(tv)     # one vertex moved normal to the curve
        setpts(c, p)

    def mirrored(j):
        c = opening(j)
        setpts(c, np.stack([c.pts[:, 0], wt - c.pts[:, 1]], 1))

    def shifted(j):
        c = opening(j)
        setpts(c, c.pts + [10 * tol, 0.0])

    def last5(j):
        c = opening(j)
        p = c.pts
        s = np.concatenate([[0.0], np.cumsum(np.linalg.norm(np.diff(p, axis=0), axis=1))])
        setpts(c, p[s <= 0.95 * s[-1]])

    def wrong_radius(j):
        c = opening(j)
        rw = RF.header_with_opening(NT['header_radius'], NT['header_dev_radius'], NT['header_length'], NT['branch_station'],
                                    NT['branch_radius'] * (1 - 1e-4), NT['beta_deg'], NT['delta'])     # smaller: the near-tangent branch still meets the header
        cw = rw.curves['opening']
        setpts(c, cw.f(np.linspace(cw.t0, cw.t1, len(c.pts))) @ R0.T + t0)      # same vertex count, radius off by 1e-4

    def judge(j):
        iss = []
        try:
            hr2, _, _ = cmp_header(NT, j.parts['header'], iss)
            declared_side(j.parts['header'], hr2, iss, [], True)
        except Exception as ex:          # as in compare_job: a comparison that cannot be completed is a failure, never a pass
            iss.append(f'comparison aborted: {type(ex).__name__}: {ex}')
        return iss

    for name, plant in (('one opening vertex moved off the curve by 10 x tol', off_curve), ('opening mirrored about the plate centre line', mirrored),
                        ('opening shifted along the station by 10 x tol', shifted), ('last 5 % of the opening missing', last5),
                        ('correct vertex count, branch radius 1e-4 relative too small', wrong_radius)):
        j = Job(cl, cl.calculate(NT))
        plant(j)
        iss = judge(j)
        report(f'near-tangent N-tangent-d-b90-g2e-06: {name}', bool(iss), iss[0] if iss else '')

    K = 25.4
    other = scale_request(REQ, K)
    base = Job(cl, cl.calculate(REQ))
    t_i, devs, _, _ = compare_scaled(base, Job(cl, cl.calculate(other)), K, other, dxf=False)
    t_i += unexpected_holds(base)             # the unmodified base is not held by the AMOEBA digest
    ok &= not (t_i or devs)
    out.append(('ok  ' if not (t_i or devs) else 'FAIL') + f' control: unmodified pattern at scale {K} passes the scale relation' + ('' if not (t_i or devs) else ': ' + (t_i + devs)[0][:170]))
    j2 = Job(cl, cl.calculate(other))
    _scale_entities(j2, 1 + 1e-6)
    t_i, devs, _, info = compare_scaled(base, j2, K, other, dxf=False)
    # a part missing from the scaled job is not a detection of the planted fault: it is reported on its own
    real = [s for s in t_i if s not in info['scale_missing_parts']] + devs
    report(f'scale relation: pattern at scale {K} multiplied by {K} x (1 + 1e-6)', bool(real), real[0] if real else '')
    if info['scale_missing_parts']:
        ok = False
        out.append(f'FAIL scale relation: {info["scale_missing_parts"][0]} (a missing part is not a detected fault)')
    # polylines only (lines and arcs exact): the chord-error test cannot see a relative 1e-6 change of the polylines, so
    # this fault must be caught by the exact reference comparison of the scaled pattern
    j3 = Job(cl, cl.calculate(other))
    _scale_entities(j3, 1 + 1e-6, kinds=('polyline',))
    t_i, devs, _, info = compare_scaled(base, j3, K, other, dxf=False)
    exact_msgs = [s for s in devs if 'exact reference comparison' in s]
    report(f'scale relation: polylines only multiplied by {K} x (1 + 1e-6), caught by the exact reference comparison', bool(exact_msgs), exact_msgs[0] if exact_msgs else '')
    if info['scale_missing_parts']:
        ok = False
        out.append(f'FAIL scale relation: {info["scale_missing_parts"][0]} (a missing part is not a detected fault)')
    return ok, out


if __name__ == '__main__':
    main()
