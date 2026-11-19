#!/usr/bin/env python3
"""Generator of large synthetic fabrication workloads for the supported families.

Produces a pipe-spool style parts list: line sizes from a table of common pipe outside diameters, headers with
branches (one job per branch: the product supports ONE opening per plate), straight shells, concentric and
eccentric reducers, loose branch stubs, repeated components (same request several times) and mirrored
components (delta -> -delta or beta -> 180 - beta, tagged "mirror_of").  A chosen fraction of deliberately
invalid parts can be mixed in.  Every record carries the expected outcome from the independent oracle
(checks/oracle.py): "accept", "refuse" or "either" (boundary).

Output: JSON Lines, one part per line:
  {"part_id", "spool", "kind", "qty", "mirror_of", "repeat_of", "expected", "why", "request"}
`request` is the body for POST /api/projects/{id}/calculations.

  python3 validation/workload_gen.py --parts 2000 --seed 7 --invalid 0.05 --out workload.jsonl
  from workload_gen import generate;  parts = generate(parts=500, seed=1)
"""
import argparse
import json
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from checks.oracle import classify

PIPE_OD = [60.3, 88.9, 114.3, 168.3, 219.1, 273.0, 323.9, 355.6, 406.4, 457.0, 508.0, 610.0, 762.0, 914.0, 1219.0, 1626.0, 2032.0]
WALL = [3.91, 5.49, 6.02, 7.11, 8.18, 9.27, 9.53, 9.53, 9.53, 9.53, 9.53, 9.53, 12.7, 12.7, 12.7, 16.0, 16.0]
ANGLES = [90, 90, 90, 90, 60, 45, 45, 30, 75, 120, 135, 150]


def _expected(req):
    cls, why = classify(req)
    return {'valid': 'accept', 'invalid': 'refuse', 'unsupported': 'refuse', 'edge': 'either'}[cls], why


def _header_branch(rng, hi, bi, sref):
    od, t = PIPE_OD[hi], WALL[hi]
    bod, bt = PIPE_OD[bi], WALL[bi]
    R = od / 2
    # A real set-on joint puts the branch INSIDE surface on the header OUTSIDE surface.  The product has one
    # surface_reference for all radii, so that mix can only be declared 'unknown'; the other parts use the
    # outside surfaces of both pipes, declared 'outside'.
    if sref == 'unknown':
        r = bod / 2 - bt
    else:
        r, sref = bod / 2, 'outside'
    beta = rng.choice(ANGLES)
    slack = R - r
    delta = 0.0 if rng.random() < 0.6 else round(rng.uniform(-0.8, 0.8) * slack, 1)
    sb = math.sin(math.radians(beta))
    reach = (R + r) / sb + r / max(math.tan(math.radians(min(beta, 180 - beta))), 0.2)
    L = round(rng.uniform(2.5, 6.0) * reach + 4 * R, 0)
    station = round(rng.uniform(reach + 0.2 * R, L - reach - 0.2 * R), 0)
    s_end = round((R + r * abs(math.cos(math.radians(beta)))) / sb + rng.uniform(0.5, 3.0) * bod, 0)
    return {'family': 'header_with_branch', 'header_radius': R, 'header_dev_radius': round(R - t / 2, 3), 'header_length': L, 'branch_station': station,
            'branch_radius': round(r, 3), 'branch_dev_radius': round(bod / 2 - bt / 2, 3), 'beta_deg': beta, 'delta': delta, 's_end': s_end, 'surface_reference': sref}


def _mirror(req, rng):
    m = dict(req)
    if req.get('delta', 0) != 0 and rng.random() < 0.5:
        m['delta'] = -req['delta']
    else:
        m['beta_deg'] = 180 - req['beta_deg']
    return m


def _break(req, rng):
    """Turn a valid request into one that must be refused."""
    b = dict(req)
    fam = req['family']
    keys = [k for k in req if k not in ('family', 'surface_reference', 'delta', 'beta_deg', 'branch_station', 'e')]
    mode = rng.choice(['zero', 'negative', 'topology'])
    if mode == 'zero':
        b[rng.choice(keys)] = 0.0
    elif mode == 'negative':
        k = rng.choice(keys)
        b[k] = -abs(req[k])
    elif fam in ('branch_stub', 'header_with_branch'):
        what = rng.choice(['big_branch', 'offset', 'angle', 'short'] + (['station'] if fam == 'header_with_branch' else []))
        if what == 'big_branch':
            b['branch_radius'] = b['branch_dev_radius'] = req['header_radius'] * rng.choice([1.0, 1.2])
        elif what == 'offset':
            b['delta'] = req['header_radius']
        elif what == 'angle':
            b['beta_deg'] = rng.choice([0, 180, 3, 177, -45, 270])
        elif what == 'short':
            b['s_end'] = 0.25 * req['header_radius']
        else:
            b['branch_station'] = rng.choice([-req['header_length'], 0.0, req['header_length'], 3 * req['header_length']])
    elif fam in ('cone_frustum', 'eccentric_reducer'):
        b['r1'], b['r2'] = (req['r2'], req['r1']) if rng.random() < 0.5 else (req['r1'], req['r1'])
    else:
        b['length'] = -req['length']
    return b


def generate(parts=1000, seed=1, invalid=0.0, repeat=0.15, mirror=0.10):
    rng = random.Random(seed)
    out = []
    spool = 0

    def emit(kind, req, sp, qty=1, mirror_of=None, repeat_of=None):
        exp, why = _expected(req)
        rec = {'part_id': f'P{len(out) + 1:06d}', 'spool': f'SP-{sp:04d}', 'kind': kind, 'qty': qty, 'mirror_of': mirror_of, 'repeat_of': repeat_of, 'expected': exp, 'why': why, 'request': req}
        out.append(rec)
        return rec

    while len(out) < parts:
        spool += 1
        hi = rng.randrange(3, len(PIPE_OD))
        od, t = PIPE_OD[hi], WALL[hi]
        sref = rng.choice(['outside', 'unknown'])
        new = []
        for _ in range(rng.randint(1, 4)):                                   # branches on this header
            bi = rng.randrange(0, hi)
            new.append(emit('header+branch', _header_branch(rng, hi, bi, sref), spool))
        for _ in range(rng.randint(0, 3)):                                   # straight shells
            new.append(emit('shell', {'family': 'cylinder_shell', 'r_dev': round(od / 2 - t / 2, 3), 'length': float(rng.choice([500, 1000, 1500, 2000, 3000, 6000]))}, spool, qty=rng.randint(1, 6)))
        if rng.random() < 0.7:                                               # reducer to a smaller line
            si = rng.randrange(max(0, hi - 4), hi)
            r1, r2 = round(od / 2 - t / 2, 3), round(PIPE_OD[si] / 2 - WALL[si] / 2, 3)
            h = float(round(rng.uniform(1.0, 4.0) * (r1 - r2) + 50, 0))
            if rng.random() < 0.5:
                new.append(emit('concentric reducer', {'family': 'cone_frustum', 'r1': r1, 'r2': r2, 'h': h}, spool))
            else:
                e = round(r1 - r2, 3) if rng.random() < 0.7 else round(rng.uniform(0, 1.5) * (r1 - r2), 3)
                new.append(emit('eccentric reducer', {'family': 'eccentric_reducer', 'r1': r1, 'r2': r2, 'h': h, 'e': e}, spool))
        if rng.random() < 0.4:                                               # loose stub (nozzle)
            bi = rng.randrange(0, hi)
            hb = _header_branch(rng, hi, bi, sref)
            new.append(emit('branch stub', {'family': 'branch_stub', **{k: hb[k] for k in ('header_radius', 'branch_radius', 'branch_dev_radius', 'beta_deg', 'delta', 's_end')}}, spool, qty=rng.randint(1, 4)))
        for rec in list(new):
            if rec['request']['family'] in ('branch_stub', 'header_with_branch') and rng.random() < mirror:
                emit(rec['kind'] + ' (mirrored)', _mirror(rec['request'], rng), spool, mirror_of=rec['part_id'])
            if rng.random() < repeat:
                emit(rec['kind'] + ' (repeat)', dict(rec['request']), spool, repeat_of=rec['part_id'])
            if rng.random() < invalid:
                emit(rec['kind'] + ' (invalid)', _break(rec['request'], rng), spool)
    return out[:parts]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--parts', type=int, default=1000)
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--invalid', type=float, default=0.0, help='fraction of parts followed by a deliberately invalid variant')
    ap.add_argument('--repeat', type=float, default=0.15)
    ap.add_argument('--mirror', type=float, default=0.10)
    ap.add_argument('--out', default='-')
    a = ap.parse_args()
    recs = generate(a.parts, a.seed, a.invalid, a.repeat, a.mirror)
    fh = sys.stdout if a.out == '-' else open(a.out, 'w')
    for r in recs:
        fh.write(json.dumps(r) + '\n')
    if a.out != '-':
        fh.close()
        from collections import Counter
        print(f'{len(recs)} parts in {len({r["spool"] for r in recs})} spools ->', a.out, dict(Counter(r['request']['family'] for r in recs)), dict(Counter(r['expected'] for r in recs)), file=sys.stderr)


if __name__ == '__main__':
    main()
