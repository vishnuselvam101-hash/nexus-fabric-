#!/usr/bin/env python3
"""Runs the whole independent validation against a NEXUS FABRIC server.

  python3 validation/run_all.py --url http://127.0.0.1:8802 --user U --password P
  python3 validation/run_all.py --spawn /path/to/nf-server            # private server on a temp data dir, port 8802

Stages: 0 reference self-test, seeded faults, DXF-only mating (task 1.5) · 1 corpus · 2 seeded random sweep · 3 synthetic workload · 4 numerical stability.
Prints a summary, writes validation/results/latest.json, exits non-zero on any mismatch or defect finding.
"""
import argparse
import glob
import json
import math
import multiprocessing as mp_
import os
import random
import sys
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import numpy as np
import shapely

from checks.client import Client, SpawnedServer, USER, PASSWORD, SETUP_TOKEN_ENV, DxfError
from checks.product import Job
from checks.compare import compare_job, compare_scaled, rel_tol, scale_request
from checks.oracle import classify
from checks import hpcheck

PARTS = {'cylinder_shell': ['shell'], 'cone_frustum': ['reducer'], 'eccentric_reducer': ['reducer'], 'branch_stub': ['branch'], 'header_with_branch': ['header', 'branch']}
# the length keys of the scale relation are defined once, in checks/compare.py (LENGTH_KEYS)
# findings that are defects of the product (exit non-zero); the others are observations
DEFECT_CODES = {'MARKING_SIDE_UNDECLARED', 'JSON_ARC_UNLABELLED', 'FLAT_SEAM_CLEARANCE', 'VIEW_SIDE_CONFLICT', 'EPS_REP_EXCEEDED', 'EPS_NUM_EXCEEDED', 'DXF_EXTENTS_WRONG', 'NON_FINITE_OUTPUT', 'REFUSAL_NOT_CLEAN', 'PRODUCT_CHECK_NOT_PASS', 'SCALE_NOT_INVARIANT', 'REFUSAL_KIND'}

_client = None


def _init(url, user, password, project):
    global _client
    _client = Client(url)
    _client.login(user, password)
    _client.project = project


def strip(o):
    if isinstance(o, dict):
        return {k: strip(v) for k, v in o.items() if k not in ('R', 't')}
    if isinstance(o, (list, tuple)):
        return [strip(v) for v in o]
    if isinstance(o, (np.floating, np.integer)):
        return float(o)
    if isinstance(o, np.ndarray):
        return o.tolist()
    if isinstance(o, float) and not math.isfinite(o):
        return repr(o)
    return o


def finite_job(job):
    for p in job.parts.values():
        for e in p.ents:
            if not np.all(np.isfinite(e.pts)):
                return False
        for v in list(p.q.values()) + [p.eps_num, p.eps_rep]:
            if v is None or not math.isfinite(v):
                return False
    return True


def submit(request=None, raw_body=None):
    return Job(_client, _client.calculate(request, raw_body))


# ------------------------------------------------------------------ expectations
def check_expected(case, job, issues):
    for kind, getter in (('quantities', lambda p: p.q), ('measures', lambda p: p.measures())):
        for part, exp in (case.get(kind) or {}).items():
            if part not in job.parts:
                issues.append(f'expected part {part} missing')
                continue
            got = getter(job.parts[part])
            for name, (val, tol) in exp.items():
                if name not in got:
                    issues.append(f'{kind}: {part}.{name} not reported')
                elif not abs(got[name] - val) <= tol:
                    issues.append(f'{kind}: {part}.{name} = {got[name]!r}, expected {val!r} +/- {tol:.3g} (off by {got[name] - val:.3e})')


def _haus(a, b):
    la, lb = shapely.LineString(a), shapely.LineString(b)
    return max(float(shapely.distance(shapely.points(a), lb).max()), float(shapely.distance(shapely.points(b), la).max()))


def check_relation(case, job, issues, findings, info):
    rel = case['relation']
    req = case['request']
    t = rel['type']
    if t == 'scale':
        k = rel['k']
        other = scale_request(req, k)
        j2 = submit(other)
        info['scaled_outcome'] = 'refused: ' + j2.refusal_text()[:200] if j2.refused else 'accepted'
        if j2.refused:
            if not rel.get('refusal_ok'):
                issues.append(f'scaled request (x{k}) refused: {j2.refusal_text()}')
            return
        if not finite_job(j2):
            findings.append({'code': 'NON_FINITE_OUTPUT', 'detail': f'request scaled by {k:.3g} was accepted and returned non-finite numbers'})
            return
        bad = [c['name'] for c in j2.checks() if c['verdict'] != 'PASS']
        info['scaled_state'] = j2.state
        # geometric scale relation (checks/compare.py compare_scaled): entity types must agree; vertex counts may differ;
        # polylines agree with k times the base within the stated chord-error limits; the scaled pattern itself must
        # pass the exact reference comparison
        type_issues, devs, sc_findings, sc_info = compare_scaled(job, j2, k, other)
        info.update(sc_info)
        issues.extend(type_issues)
        if devs and rel.get('refusal_ok'):
            findings.append({'code': 'SCALE_NOT_INVARIANT', 'detail': f'request scaled by {k:.3g} was accepted but the pattern is not the scaled pattern ({"; ".join(devs[:3])}); product state {j2.state}, checks not PASS: {bad}'})
        else:
            issues.extend(devs)
        findings.extend(sc_findings)
    elif t in ('mirror_delta', 'mirror_beta', 'station_shift'):
        other = dict(req)
        if t == 'mirror_delta':
            other['delta'] = -req['delta']
        elif t == 'mirror_beta':
            other['beta_deg'] = 180 - req['beta_deg']
        else:
            other['branch_station'] = req['branch_station'] + rel['d']
        j2 = submit(other)
        if j2.refused:
            issues.append(f'{t}: transformed request refused: {j2.refusal_text()}')
            return
        h1, h2 = job.parts['header'], j2.parts['header']
        b1, b2 = job.parts['branch'], j2.parts['branch']
        W, Wb = 2 * math.pi * req['header_dev_radius'], 2 * math.pi * req['branch_dev_radius']
        hole = h1.curve('branch_opening').pts.copy()
        cut = b1.curve('saddle_cut').pts.copy()
        if t == 'mirror_delta':
            hole[:, 1] = W - hole[:, 1]
            cut[:, 0] = np.mod(Wb / 2 - cut[:, 0], Wb)
        elif t == 'mirror_beta':
            hole[:, 0] = 2 * req['branch_station'] - hole[:, 0]
            cut[:, 0] = Wb - cut[:, 0]
        else:
            hole[:, 0] += rel['d']
        tol = 1e-9 * max(h1.extent(), b1.extent())
        cut2 = b2.curve('saddle_cut')
        dh = _haus(hole, h2.curve('branch_opening').pts)
        db = float(cut2.dist(cut).max())
        info['relation_dev'] = {'opening': dh, 'saddle': db}
        if dh > tol:
            issues.append(f'{t}: opening is not the transformed opening (Hausdorff {dh:.3e} mm)')
        if db > tol:
            issues.append(f'{t}: saddle is not the transformed saddle (deviation {db:.3e} mm)')
        for n in ('seam_a', 'top', 'seam_b', 'bottom'):
            if np.abs(h1.curve(n).pts - h2.curve(n).pts).max() > 0:
                issues.append(f'{t}: plate outline changed')
        if t == 'station_shift' and any(np.abs(e1.pts - e2.pts).max() > 0 for e1, e2 in zip(b1.ents, b2.ents)):
            issues.append('station_shift: branch pattern changed')
    elif t == 'stub_equals_branch':
        other = {'family': 'branch_stub', **{k: req[k] for k in ('header_radius', 'branch_radius', 'branch_dev_radius', 'beta_deg', 'delta', 's_end')}}
        j2 = submit(other)
        if j2.refused:
            issues.append('stub refused')
            return
        # the cut geometry (ENTITIES up to the first TEXT annotation); the annotation of an assembly part also states the joint (engine 0.4.1)
        def cut(t):
            i = t.index('  2\nENTITIES\n')
            k = t.find('  0\nTEXT\n', i)
            return t[i:k if k >= 0 else len(t)]
        b1, b2 = job.parts['branch'], j2.parts['branch']
        if b1.from_file and b1.dxf_text is not None and b2.from_file and b2.dxf_text is not None:
            if cut(b1.dxf_text) != cut(b2.dxf_text):
                issues.append('branch_stub DXF differs from the branch part of the assembly')
        else:
            # a withheld branch has no file: the byte comparison is NOT APPLICABLE (recorded, never a pass);
            # the drawn entities are compared instead (the JSON vertices of a withheld part are exact)
            missing = [n for n, b in (('assembly branch', b1), ('stub', b2)) if not (b.from_file and b.dxf_text is not None)]
            info['stub_byte_compare'] = 'NOT APPLICABLE: DXF withheld for ' + ' and '.join(missing)
            if len(b1.ents) != len(b2.ents) or any(e1.kind != e2.kind or e1.layer != e2.layer or e1.pts.shape != e2.pts.shape or np.abs(e1.pts - e2.pts).max() > 0 for e1, e2 in zip(b1.ents, b2.ents)):
                issues.append('branch_stub pattern differs from the branch part of the assembly (entity kinds, layers and points compared; DXF byte comparison NOT APPLICABLE)')
    elif t == 'equivalent_outline':
        j2 = submit(rel['other'])
        if j2.refused:
            issues.append('comparison request refused')
            return
        p1, p2 = list(job.parts.values())[0], list(j2.parts.values())[0]
        r1 = np.concatenate([c.dense(721) for c in p1.outline])
        r2 = np.concatenate([c.dense(721) for c in p2.outline])
        d = max(float(shapely.distance(shapely.points(r1), shapely.LineString(np.concatenate([c.dense(2881) for c in p2.outline]))).max()),
                float(shapely.distance(shapely.points(r2), shapely.LineString(np.concatenate([c.dense(2881) for c in p1.outline]))).max()))
        sc = max(p1.extent(), p2.extent())
        info['relation_dev'] = {'outline_hausdorff': d, 'relative': d / sc}
        if d > rel['tol_rel'] * sc:
            issues.append(f'outline differs from the equivalent part by {d:.3e} mm ({d / sc:.2e} relative, allowed {rel["tol_rel"]:.1e})')


def check_refusal(case, job, findings):
    """A refusal must be clean: no pattern, no DXF, never APPROVED, and it should say why."""
    exp = case.get('refusal') or {}
    fam = (case.get('request') or {}).get('family')
    if job.http in (200, 422):
        if job.http == 200 or job.state != 'REFUSED':
            findings.append({'code': 'REFUSAL_KIND', 'detail': f'refusal answered HTTP {job.http} with state {job.state}; the contract is HTTP 422 with state REFUSED'})
        if job.state == 'APPROVED':
            findings.append({'code': 'REFUSAL_NOT_CLEAN', 'detail': 'refused job is in state APPROVED'})
        for part in PARTS.get(fam, []):
            try:
                st, _ = _client.dxf(job.id, part)
            except DxfError as e:
                st = e.status
            # only 404 (no file) holds: a 200 serves a DXF, and a 409 (withheld) is still a file the refused job answered for
            if st != 404:
                findings.append({'code': 'REFUSAL_NOT_CLEAN', 'detail': f'DXF request for part {part} of a refused job answered HTTP {st}, expected 404'})
        if not job.outcome:
            findings.append({'code': 'REFUSAL_NOT_CLEAN', 'detail': 'refused job has no stated outcome'})
    if exp.get('kind') == 'malformed' and job.http != 400:
        findings.append({'code': 'REFUSAL_KIND', 'detail': 'request that is not valid input was stored as a job instead of being rejected with HTTP 400'})
    if exp.get('kind') == 'engineering' and job.http != 422:
        findings.append({'code': 'REFUSAL_KIND_NOTE', 'detail': f'engineering refusal expected as a recorded job, got HTTP {job.http}'})
    m = exp.get('mentions') or []
    text = job.refusal_text().lower()
    if m and not any(w.lower() in text for w in m):
        findings.append({'code': 'REFUSAL_REASON_UNCLEAR', 'detail': f'refusal text does not mention any of {m}: "{job.refusal_text()[:160]}"'})


# ------------------------------------------------------------------ one case
def run_case(case):
    out = {'id': case['id'], 'group': case['group'], 'title': case.get('title', ''), 'request': case.get('request'), 'expect': case['expect'], 'issues': [], 'findings': [], 'info': {}}
    t0 = time.time()
    try:
        job = submit(case.get('request'), case.get('raw_body'))
        out['http'], out['job_id'], out['outcome'], out['state'], out['engine'] = job.http, job.id, job.refusal_text() if job.refused else None, job.state, job.engine
        if job.http in (401, 403, 404, 421, 429, 500, 502, 503):
            out['class'] = 'error'
            out['issues'].append(f'server answered HTTP {job.http}: {job.error}')
            return out
        if job.accepted and not finite_job(job):
            out['findings'].append({'code': 'NON_FINITE_OUTPUT', 'detail': 'accepted job contains non-finite numbers in geometry, quantities or error estimates'})
        if case.get('g11'):
            g11 = next((g['result'] for g in job.result.get('gates', []) if g['id'] == 'G11'), None)
            if g11 != case['g11']:
                out['issues'].append(f'profile gate G11 is {g11}, expected {case["g11"]} (seam gap against its resolution)')
            if job.state == 'APPROVED':
                out['issues'].append('state APPROVED is unreachable')
        exp = case['expect']
        if exp == 'refuse':
            if job.accepted:
                out['class'] = 'wrongly_accepted'
                out['issues'].append('request that must be refused produced a pattern')
            else:
                out['class'] = 'refused_correctly'
                if job.self_rejected:
                    out['findings'].append({'code': 'REFUSED_ONLY_BY_GATE', 'detail': 'input was not refused: a pattern was computed and stored (with null/non-finite numbers where they overflowed), then rejected by the profile gate; files withheld'})
                else:
                    check_refusal(case, job, out['findings'])
            return out
        if job.refused:
            regime = 'task 1.1' in str((case.get('oracle') or ['', ''])[-1])
            if exp == 'either' and regime and (job.self_rejected or job.refusal_code != 'branch_angle_not_verifiable'):
                # task 1.1: in the regime of the verification's conditioning only that refusal is allowed; a computed
                # pattern REJECTED by the product's own gates, or a refusal with another code, is not
                out['class'] = 'wrongly_refused'
                out['issues'].append(f'in the task 1.1 regime only branch_angle_not_verifiable may refuse: {job.refusal_text()[:200]}')
                if job.self_rejected:
                    out['findings'].append({'code': 'REFUSED_ONLY_BY_GATE', 'detail': 'pattern computed, then REJECTED by the product\'s own gates (G10) in the task 1.1 regime instead of refused with branch_angle_not_verifiable'})
            elif exp == 'either':
                out['class'] = 'refused_correctly'
                if not job.self_rejected:
                    check_refusal(case, job, out['findings'])
            else:
                out['class'] = 'wrongly_refused'
                out['issues'].append(f'valid request refused: {job.refusal_text()[:200]}')
            return out
        if case.get('compare', True) and finite_job(job):
            rep = compare_job(case['request'], job, ndense=case.get('ndense', 6001))
            out['issues'] += rep['issues']
            out['findings'] += rep['findings']
            out['info']['compare'] = {k: v for k, v in rep.items() if k not in ('issues', 'findings')}
        check_expected(case, job, out['issues'])
        if case.get('relation'):
            check_relation(case, job, out['issues'], out['findings'], out['info'])
        if case.get('hp'):
            te = hpcheck.true_errors(case['request'], job)
            out['info']['true_numeric_error'] = te
            for part, v in te.items():
                if v['true_error'] > v['eps_num'] * 1.0 and v['true_error'] > 0:
                    out['findings'].append({'code': 'EPS_NUM_EXCEEDED', 'detail': f"{part}: true numerical error {v['true_error']:.3e} mm exceeds stated eps_num {v['eps_num']:.3e} mm (x{v['ratio']:.1f})"})
        out['class'] = 'agree' if not out['issues'] else 'disagree'
    except Exception as ex:
        out['class'] = 'error'
        out['issues'].append(f'{type(ex).__name__}: {ex} | {traceback.format_exc(limit=3)}')
    finally:
        out['seconds'] = round(time.time() - t0, 3)
        out['info'] = strip(out['info'])
    return out


# ------------------------------------------------------------------ case sources
QUICK_ALWAYS = ('A-TEE-90', 'A-CONE-HALFDISC', 'A-CYL-01', 'HP-JOINT-4', 'HP-ECC-1', 'HP-CONE-1', 'S-MIRROR-DELTA', 'S-MIRROR-BETA', 'S-STATION-SHIFT', 'D-ASM-DEV-1-near_start')


def corpus_cases(quick):
    cs = [json.load(open(f)) for f in sorted(glob.glob(os.path.join(HERE, 'corpus', 'cases', '*.json')))]
    for c in cs:
        if c['group'] == 'high_precision':
            c['hp'] = True
    if quick:
        # CI subset: every closed form, fixed frame / view-side / mirror cases, and a thin slice of everything else
        step = {'closed_form': 1, 'limit': 2, 'high_precision': 6, 'assembly': 9, 'symmetry': 8, 'refusal': 6}
        keep, n = [], {}
        for c in cs:
            i = n.get(c['group'], 0)
            n[c['group']] = i + 1
            if c['id'] in QUICK_ALWAYS or i % step.get(c['group'], 6) == 0:
                keep.append(c)
        cs = keep
    return cs


def random_cases(seed, n):
    rng = random.Random(seed)
    lu = lambda a, b: math.exp(rng.uniform(math.log(a), math.log(b)))
    cs = []

    def add(fam, req):
        cls, why = classify(req)
        cs.append({'id': f'R-{fam}-{len(cs):04d}', 'group': 'random_sweep', 'title': f'seeded random {fam} ({cls}{": " + why if why else ""})', 'request': req,
                   'expect': {'valid': 'accept', 'invalid': 'refuse', 'unsupported': 'refuse', 'edge': 'either'}[cls], 'oracle': [cls, why]})

    for _ in range(n):
        add('cylinder_shell', {'family': 'cylinder_shell', 'r_dev': lu(1e-2, 1e5), 'length': lu(1e-2, 1e5) * rng.choice([1, 1, 1, 1, -1])})
    for _ in range(n):
        r1 = lu(1e-1, 1e4)
        r2 = r1 * rng.choice([rng.uniform(0.02, 0.98), rng.uniform(0.9, 0.99999), rng.uniform(0.3, 1.2)])
        add('cone_frustum', {'family': 'cone_frustum', 'r1': r1, 'r2': r2, 'h': r1 * lu(1e-3, 1e2)})
    for _ in range(n):
        r1 = lu(1e-1, 1e4)
        r2 = r1 * rng.choice([rng.uniform(0.02, 0.98), rng.uniform(0.02, 0.98), rng.uniform(0.3, 1.1)])
        add('eccentric_reducer', {'family': 'eccentric_reducer', 'r1': r1, 'r2': r2, 'h': r1 * lu(1e-3, 1e2), 'e': r1 * rng.choice([0, rng.uniform(0, 1), lu(1e-3, 30), -0.1])})

    def joint():
        R = lu(1, 1e4)
        r = R * rng.choice([rng.uniform(0.02, 0.95), rng.uniform(0.9, 0.9999), rng.uniform(0.5, 1.3)])
        beta = rng.choice([rng.uniform(10, 170), rng.uniform(10, 170), rng.uniform(-20, 200)])
        delta = rng.choice([0.0, rng.uniform(-1, 1) * max(R - r, 0), rng.uniform(-1.2, 1.2) * R])
        return R, r, beta, delta

    for _ in range(n):
        R, r, beta, delta = joint()
        sb = max(abs(math.sin(math.radians(beta))), 0.05)
        add('branch_stub', {'family': 'branch_stub', 'header_radius': R, 'branch_radius': r, 'branch_dev_radius': r * rng.uniform(0.9, 1.1), 'beta_deg': beta, 'delta': delta,
                            's_end': (R + r) / sb * rng.choice([1.3, 2.0, 0.6])})
    for _ in range(n):
        R, r, beta, delta = joint()
        sb = max(abs(math.sin(math.radians(beta))), 0.05)
        reach = (R + 2 * r) / sb
        L = reach * rng.uniform(2.5, 8)
        add('header_with_branch', {'family': 'header_with_branch', 'header_radius': R, 'header_dev_radius': R * rng.uniform(0.9, 1.1), 'header_length': L,
                                   'branch_station': rng.choice([rng.uniform(0.35, 0.65), rng.uniform(0.35, 0.65), rng.uniform(-0.2, 1.2)]) * L, 'branch_radius': r, 'branch_dev_radius': r * rng.uniform(0.9, 1.1),
                                   'beta_deg': beta, 'delta': delta, 's_end': (R + r) / sb * rng.choice([1.3, 2.0, 0.6]), 'surface_reference': rng.choice(['outside', 'inside', 'neutral', 'unknown'])})
    return cs


def workload_cases(n, seed):
    from workload_gen import generate
    cs = []
    for i, rec in enumerate(generate(parts=n, seed=seed, invalid=0.05)):
        cs.append({'id': f'W-{rec["part_id"]}', 'group': 'workload', 'title': f'{rec["spool"]} {rec["kind"]}', 'request': rec['request'], 'expect': rec['expected'], 'compare': (i % 4 == 0)})
    return cs


def stability_cases():
    from stability import cases
    return cases()


# ------------------------------------------------------------------ reference self-test
def selftest():
    """The reference must reproduce itself by three methods and the published reference values before it judges anything."""
    import mpmath as mp
    from reference import families as RF, hp, ruled
    res = []

    def chk(name, got, want, tol):
        ok = abs(float(got) - float(want)) <= tol
        res.append({'name': name, 'got': float(got), 'want': float(want), 'tol': tol, 'ok': bool(ok)})

    c = RF.cone_frustum(200, 100, 300)
    h = hp.right_cone(200, 100, 300)
    chk('cone: float developer sector angle vs mpmath', c.info['psi_total'], h['sector_angle'], 1e-12)
    chk('cone: mpmath rim-length route vs spherical-image route', h['sector_angle'], h['sector_angle_spherical'], 1e-30)
    chk('cone: 3D surface integral vs sector area', h['area'], h['area_sector'], 1e-20)
    chk('cone: published CONE-01 sector angle', h['sector_angle'], 1.986917653159, 1e-12)
    e = RF.eccentric_reducer(200, 100, 300, 100)
    he = hp.eccentric(200, 100, 300, 100)
    chk('eccentric: float apex-free developer vs mpmath apex route (developed angle)', e.info['psi_total'], he['developed_angle'], 1e-11)
    a = e.curves['large'].sample(3)
    chord = np.linalg.norm(a[0] - a[1])
    chk('eccentric: float chord 0-180 vs mpmath apex-free Taylor ODE', chord, hp.eccentric_apexfree_chord(200, 100, 300, 100), 2e-10)
    chk('eccentric: published ECC-01 chord', chord, 602.7695543939, 1e-9)
    chk('eccentric: published ECC-01 area', he['area'], 304679.8420026, 1e-6)
    chk('eccentric: surface integral vs polar integral', he['area'], he['area_polar'], 1e-25)
    tri = []
    for n in (400, 800, 1600):
        tt = np.linspace(0, 2 * np.pi, n + 1)
        p2, q2 = ruled.unroll_strip(e.c1.p(tt), e.c2.p(tt))
        tri.append(np.linalg.norm(p2[n // 2] - p2[0]))
    rich = tri[2] + (tri[2] - tri[1]) / 3
    chk('eccentric: triangulation unrolling (Richardson limit) vs developer', rich, chord, 2e-6)
    chk('eccentric: triangulation converges at second order (ratio of successive errors ~4)', (tri[1] - tri[0]) / (tri[2] - tri[1]), 4.0, 0.1)
    j = RF.Joint3D(150, 100, 60, 0)
    s = j.s_of_phi(np.radians([0, 45, 90, 135, 180, 270]))
    for got, want in zip(s, [129.0994448736, 111.9276941188, 115.4700538379, 111.9276941188, 129.0994448736, 230.9401076759]):
        chk('joint: published BR-03 table', got, want, 2e-10)
    j = RF.Joint3D(150, 80, 90, 40)
    for got, want in zip(j.s_of_phi(np.radians([0, 45, 90, 135, 180, 270])), [90.0, 114.7802970915, 144.5683229480, 149.0821364201, 144.5683229480, 144.5683229480]):
        chk('joint: published BR-04 table', got, want, 2e-10)
    jh = hp.Joint(150, 100, 60, 20)
    jf = RF.Joint3D(150, 100, 60, 20)
    ph = [0.1, 1.3, 2.9, 4.4, 6.0]
    for p, sf in zip(ph, jf.s_of_phi(np.array(ph))):
        chk('joint: float bisection vs mpmath vector quadratic', sf, jh.s(p), 1e-11)
    X = jf.curve3d(np.linspace(0, 2 * np.pi, 97))
    chk('joint: intersection points on header (point-to-axis distance)', np.abs(jf.dist_header_axis(X) - 150).max(), 0, 1e-10)
    chk('joint: intersection points on branch (point-to-axis distance)', np.abs(jf.dist_branch_axis(X) - 100).max(), 0, 1e-10)
    return res


# ------------------------------------------------------------------ main
def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--url', default='http://127.0.0.1:8802')
    ap.add_argument('--spawn', help='path of an nf-server binary to start privately (temp data dir)')
    ap.add_argument('--port', type=int, default=8802)
    ap.add_argument('--user', default=USER)
    ap.add_argument('--password', default=PASSWORD)
    ap.add_argument('--seed', type=int, default=20261119)
    ap.add_argument('--random', type=int, default=60, help='random cases per family')
    ap.add_argument('--workload', type=int, default=400, help='synthetic workload parts')
    ap.add_argument('--workers', type=int, default=3)
    ap.add_argument('--quick', action='store_true', help='CI mode: a few minutes; keeps the Defect-002 detector, the view-side check and every closed-form case')
    ap.add_argument('--setup-token', default=os.environ.get(SETUP_TOKEN_ENV), help=f'one-time first-run setup token of the server (or env {SETUP_TOKEN_ENV}); only needed while the server has no administrator')
    ap.add_argument('--only', help='comma-separated group names')
    ap.add_argument('--out', default=os.path.join(HERE, 'results', 'latest.json'))
    a = ap.parse_args()
    server = None
    if a.spawn:
        server = SpawnedServer(a.spawn, a.port)
        a.url = server.url
        a.setup_token = server.token
    t0 = time.time()
    # The spawned server is stopped however this block ends: an exception in the self-test, the planted-fault
    # stage or the corpus run must not leave it running (task 1.8).
    try:
        st = selftest()
        print(f'[0] reference self-test: {sum(r["ok"] for r in st)}/{len(st)} ok')
        for r in st:
            if not r['ok']:
                print('    FAIL', r)
        client = Client(a.url).connect(a.user, a.password, a.setup_token)
        # the validator validates itself against the live build: planted faults (Defect 002 among them) must be reported
        import seeded_faults
        sf_ok, sf_lines = seeded_faults.run(client)
        print(f'[0] seeded faults (incl. Defect 002 and DXF tampering): {"all detected" if sf_ok else "NOT ALL DETECTED"}')
        for ln in sf_lines:
            if not ln.startswith('ok'):
                print('    ' + ln)
        # work plan task 1.5: the two DXF files of a one-curve joint must mate from the files alone (fold-back derived
        # from Spec 02 section 4.4, correspondence searched); the correct joint must mate and every planted fault must FAIL
        from checks import mating
        mt_ok, mt_lines = mating.run(client)
        print(f'[0] DXF-only mating (task 1.5): {"mates, all faults detected" if mt_ok else "FAIL"}')
        for ln in mt_lines:
            if not ln.startswith('ok'):
                print('    ' + ln)
        cases = corpus_cases(a.quick)
        cases += random_cases(a.seed, 5 if a.quick else a.random)
        cases += workload_cases(24 if a.quick else a.workload, a.seed)
        stab = stability_cases()
        cases += [c for i, c in enumerate(stab) if i % 12 == 0 or c['id'] in ('N-tangent-r-b45-g1.1e-06', 'N-tangent-d-b90-g2e-06', 'N-aspect-cone-flat-h0.01')] if a.quick else stab
        if a.only:
            cases = [c for c in cases if c['group'] in a.only.split(',')]
        print(f'[1-4] {len(cases)} cases against {a.url} (health reports engine {client.engine}), project {client.project}')
        with mp_.Pool(a.workers, initializer=_init, initargs=(a.url, a.user, a.password, client.project)) as pool:
            results = []
            for i, r in enumerate(pool.imap(run_case, cases, chunksize=1)):
                results.append(r)
                if (i + 1) % 100 == 0:
                    print(f'    {i + 1}/{len(cases)}  ({time.time() - t0:.0f} s)', flush=True)
        restarts = server.restarts if server else 0
    finally:
        if server:
            server.stop()
            try:
                server.proc.wait(timeout=10)
            except Exception:
                server.proc.kill()
    # ---- summary
    classes = ['agree', 'disagree', 'refused_correctly', 'wrongly_accepted', 'wrongly_refused', 'error']
    groups = {}
    for r in results:
        g = groups.setdefault(r['group'], {c: 0 for c in classes} | {'run': 0, 'with_defect_findings': 0})
        g['run'] += 1
        g[r['class']] += 1
        if any(f['code'] in DEFECT_CODES for f in r['findings']):
            g['with_defect_findings'] += 1
    total = {k: sum(g[k] for g in groups.values()) for k in ['run'] + classes + ['with_defect_findings']}
    fcount = {}
    for r in results:
        for f in r['findings']:
            d = fcount.setdefault(f['code'], {'count': 0, 'example': None})
            d['count'] += 1
            d['example'] = d['example'] or {'case': r['id'], 'request': r['request'], 'detail': f['detail']}
    hdr = f'{"group":<16}{"run":>6}{"agree":>7}{"disagr":>7}{"ref.ok":>7}{"w.acc":>6}{"w.ref":>6}{"error":>6}{"defect-findings":>17}'
    print('\n' + hdr + '\n' + '-' * len(hdr))
    for name, g in list(groups.items()) + [('TOTAL', total)]:
        print(f'{name:<16}{g["run"]:>6}{g["agree"]:>7}{g["disagree"]:>7}{g["refused_correctly"]:>7}{g["wrongly_accepted"]:>6}{g["wrongly_refused"]:>6}{g["error"]:>6}{g["with_defect_findings"]:>17}')
    print('\nfindings by code:')
    for code, d in sorted(fcount.items()):
        print(f'  {code:<24}{d["count"]:>5}  {"DEFECT" if code in DEFECT_CODES else "note  "}  e.g. {d["example"]["case"]}: {d["example"]["detail"][:150]}')
    bad = [r for r in results if r['class'] in ('disagree', 'wrongly_accepted', 'wrongly_refused', 'error')]
    if bad:
        print(f'\nmismatches ({len(bad)}):')
        for r in bad[:60]:
            print(f'  [{r["class"]}] {r["id"]}: {r["issues"][0][:230] if r["issues"] else ""}')
            print(f'      request: {json.dumps(r["request"])[:300]}')
    selftest_ok = all(r['ok'] for r in st) and sf_ok and mt_ok
    engines = sorted({r.get('engine') for r in results if r.get('engine')})
    ok = selftest_ok and not bad and not any(c in DEFECT_CODES for c in fcount)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, 'w') as fh:
        json.dump({'when': time.strftime('%Y-%m-%dT%H:%M:%S%z'), 'url': a.url, 'engine': engines, 'mode': 'quick' if a.quick else 'full', 'seeded_faults': sf_lines, 'mating': mt_lines, 'seed': a.seed, 'elapsed_s': round(time.time() - t0, 1), 'verdict': 'PASS' if ok else 'FAIL', 'server_restarts_during_run': restarts,
                   'selftest': st, 'summary': {'groups': groups, 'total': total}, 'findings_by_code': fcount, 'results': results}, fh, indent=1, default=str)
    print(f'\n{"PASS" if ok else "FAIL"}  ({time.time() - t0:.0f} s)  ->  {a.out}')
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
