"""Numerical-stability cases: near-tangent branches, shallow angles, extreme aspect ratios, extreme magnitudes.
Each case is checked like a corpus case (reference comparison with dense sampling) and, where marked `hp`,
the product's coordinates are compared with the mpmath tier to measure the TRUE numerical error against the
stated eps_num.  Extreme magnitudes are tested through exact power-of-two scaling of a verified base case:
the scaled job must either be refused or be exactly the scaled pattern, with every number finite."""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from checks.oracle import classify
import numpy as np
from reference import families as RF

EXP = {'valid': 'accept', 'invalid': 'refuse', 'unsupported': 'refuse', 'edge': 'either'}


def _asm(R, r, beta, delta, Rd=None, rd=None):
    j = RF.Joint3D(R, r, beta, delta)
    ph = np.linspace(0, 2 * np.pi, 40001)
    s = j.s_of_phi(ph)
    if np.isnan(s).any():
        return {'family': 'header_with_branch', 'header_radius': R, 'header_dev_radius': Rd or R, 'header_length': 10 * R, 'branch_station': 5 * R, 'branch_radius': r, 'branch_dev_radius': rd or r,
                'beta_deg': beta, 'delta': delta, 's_end': 20 * R, 'surface_reference': 'outside'}
    x, _ = j.header_coords(j.branch_point(ph, s))
    return {'family': 'header_with_branch', 'header_radius': R, 'header_dev_radius': Rd or R, 'header_length': float(math.ceil(x.max() - x.min() + 2 * R)), 'branch_station': float(math.ceil(-x.min() + R)),
            'branch_radius': r, 'branch_dev_radius': rd or r, 'beta_deg': beta, 'delta': delta, 's_end': float(math.ceil(np.nanmax(s) + R)), 'surface_reference': 'outside'}


def cases():
    cs = []

    def add(tag, title, req, hp=True, **kw):
        cls, why = classify(req)
        c = {'id': f'N-{tag}', 'group': 'stability', 'title': title, 'request': req, 'expect': EXP[cls], 'oracle': [cls, why], 'hp': hp, 'ndense': 60001}
        c.update(kw)
        cs.append(c)

    R = 150.0
    for beta in (90, 45):
        for g in (1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 2e-6, 1.1e-6, 1e-6, 5e-7):
            add(f'tangent-r-b{beta}-g{g:g}', f'near-tangent: r = R(1-{g:g}), delta = 0, beta = {beta}', _asm(R, R * (1 - g), beta, 0.0))
        for g in (1e-2, 1e-4, 2e-6):
            add(f'tangent-d-b{beta}-g{g:g}', f'near-tangent: delta = R - r - {g:g} R, r = R/2, beta = {beta}', _asm(R, R / 2, beta, R / 2 - g * R))
            add(f'tangent-dneg-b{beta}-g{g:g}', f'near-tangent: delta = -(R - r - {g:g} R), r = R/2, beta = {beta}', _asm(R, R / 2, beta, -(R / 2 - g * R)))
    for beta in (9.0, 9.78, 9.8, 10.0, 11.0, 15.0, 165.0, 170.0, 170.2, 170.3):
        for ratio in (0.3, 0.9):
            add(f'shallow-b{beta:g}-r{ratio:g}', f'shallow angle beta = {beta}, r/R = {ratio}', _asm(R, R * ratio, beta, 0.2 * R * (1 - ratio)))
    # extreme aspect ratios
    add('aspect-cyl-thin', 'cylinder r = 0.01, L = 1e6', {'family': 'cylinder_shell', 'r_dev': 0.01, 'length': 1e6})
    add('aspect-cyl-flat', 'cylinder r = 1e6, L = 0.01', {'family': 'cylinder_shell', 'r_dev': 1e6, 'length': 0.01})
    for h in (100.0, 1.0, 1e-2, 1e-4, 1e-8):
        add(f'aspect-cone-flat-h{h:g}', f'nearly flat cone r1=300 r2=50 h={h:g} (sector -> full turn)', {'family': 'cone_frustum', 'r1': 300, 'r2': 50, 'h': h})
        add(f'aspect-ecc-flat-h{h:g}', f'nearly flat eccentric reducer r1=300 r2=50 h={h:g} e=100', {'family': 'eccentric_reducer', 'r1': 300, 'r2': 50, 'h': h, 'e': 100})
    for sa in (1.5e-3, 1.01e-3, 1.0001e-3, 0.9999e-3):
        h = 1.0 / sa * math.sqrt(1 - sa * sa)
        add(f'aspect-cone-slender-sa{sa:g}', f'near-cylindrical cone, sin(alpha) = {sa:g} (declared limit 1e-3)', {'family': 'cone_frustum', 'r1': 200.0, 'r2': 199.0, 'h': h})
        add(f'aspect-ecc-slender-sa{sa:g}', f'near-cylindrical eccentric reducer, sin(alpha) = {sa:g}', {'family': 'eccentric_reducer', 'r1': 200.0, 'r2': 199.0, 'h': h, 'e': 1.0})
    for e in (1e3, 1e4, 1e6):
        add(f'aspect-ecc-oblique-e{e:g}', f'very oblique reducer r1=200 r2=100 h=300 e={e:g}', {'family': 'eccentric_reducer', 'r1': 200, 'r2': 100, 'h': 300, 'e': e})
    add('aspect-ecc-tiny-r2', 'reducer with tiny small end r2 = 1e-6', {'family': 'eccentric_reducer', 'r1': 200, 'r2': 1e-6, 'h': 300, 'e': 100})
    add('aspect-cone-tiny-r2', 'cone with tiny small end r2 = 1e-9 (apex nearly in the pattern)', {'family': 'cone_frustum', 'r1': 200, 'r2': 1e-9, 'h': 300})
    add('aspect-branch-tiny', 'tiny branch r/R = 1e-6', _asm(1000.0, 1e-3, 60, 0.0))
    add('aspect-branch-long', 'very long stub s_end = 1e6 R', {'family': 'branch_stub', 'header_radius': 150, 'branch_radius': 100, 'branch_dev_radius': 100, 'beta_deg': 60, 'delta': 20, 's_end': 1.5e8})
    a = _asm(150.0, 100.0, 60, 20.0)
    add('aspect-header-long', 'very long header L = 1e7 R', dict(a, header_length=1.5e9, branch_station=7.5e8))
    add('devradius-mismatch', 'development radii impossible for the declared surface (R_dev = 10 R, declared outside)', dict(a, header_dev_radius=1500.0, branch_dev_radius=10.0), hp=False)
    add('devradius-mismatch-unknown', 'development radii far from contact radii with surface reference unknown (R_dev = 10 R, r_dev = 0.1 r): accepted without plausibility check', dict(a, header_dev_radius=1500.0, branch_dev_radius=10.0, surface_reference='unknown'), hp=False)
    # extreme magnitudes through exact scaling
    bases = {'cyl': {'family': 'cylinder_shell', 'r_dev': 500, 'length': 1000}, 'cone': {'family': 'cone_frustum', 'r1': 200, 'r2': 100, 'h': 300},
             'ecc': {'family': 'eccentric_reducer', 'r1': 200, 'r2': 100, 'h': 300, 'e': 100}, 'asm': a}
    for name, b in bases.items():
        for n in (-1040, -200, -60, -30, -26, -22, -16, -8, 8, 14, 16, 18, 30, 200, 505, 1010):
            inside = -26 <= n <= 16      # every dimension of the scaled part stays within the declared 1e-6 .. 1e8 mm
            cs.append({'id': f'N-magnitude-{name}-2^{n}', 'group': 'stability', 'title': f'{b["family"]} scaled by 2^{n} (~{2.0 ** n:.1e})' + ('' if inside else ' - outside the declared range'), 'request': b, 'expect': 'accept', 'compare': False, 'hp': False,
                       'relation': {'type': 'scale', 'k': 2.0 ** n, 'refusal_ok': not inside}})
    return cs


if __name__ == '__main__':
    for c in cases():
        print(c['id'], c['expect'], c.get('oracle'))
