#!/usr/bin/env python3
"""How large is the error if a chiral branch pattern is marked on the wrong face of the plate?

Uses the reference only.  A branch stub rolled from the mirrored pattern has saddle heights s(-phi).  The fitter
may still rotate the stub about its axis (angle c) and slide it along the axis (d); the residual root gap is
    min over c, d of  max over phi | s(c - phi) + d - s(phi) | .
Zero when the joint is mirror-symmetric (delta = 0 or beta = 90 deg)."""
import numpy as np
from reference import families as RF


def misfit(R, r, beta, delta, n=3600):
    j = RF.Joint3D(R, r, beta, delta)
    phi = np.arange(n) * 2 * np.pi / n
    s = j.s_of_phi(phi)
    sm = s[(-np.arange(n)) % n]                    # s(-phi)
    best = np.inf
    for k in range(n):                             # rotation by c = k * 2 pi / n
        d = np.roll(sm, k) - s
        best = min(best, 0.5 * (d.max() - d.min()))  # optimal axial shift centres the residual
    return float(best)


if __name__ == '__main__':
    print(f'{"R":>6}{"r":>7}{"beta":>6}{"delta":>7}   residual root gap of a mirrored stub (mm)')
    for R, r, beta, delta in [(150, 100, 60, 20), (150, 100, 60, 5), (150, 100, 60, 1), (150, 100, 85, 20), (150, 100, 45, 40), (150, 75, 45, 60), (150, 30, 30, 100), (150, 100, 90, 20), (150, 100, 60, 0),
                              (406.4, 150, 45, 100), (1000, 300, 60, 350)]:
        print(f'{R:>6}{r:>7}{beta:>6}{delta:>7}   {misfit(R, r, beta, delta):.3f}')
