import sys
import numpy as np
sys.stdout.reconfigure(encoding='utf-8')
rng = np.random.default_rng(0)

def cone_cyl_points(A, cen, s, half_scale=1.0):
    dA = np.linalg.norm(A - cen); ax = (cen - A)/dA; half = np.arcsin(s/dA)*half_scale
    e1 = np.cross(ax, [0, 0, 1.0]); e1 /= np.linalg.norm(e1); e2 = np.cross(ax, e1)
    pts = []
    for ang in np.linspace(0, 2*np.pi, 1441)[:-1]:
        g = np.cos(half)*ax + np.sin(half)*(np.cos(ang)*e1 + np.sin(ang)*e2)
        a2 = g[1]**2 + g[2]**2; b2 = 2*(A[1]*g[1] + A[2]*g[2]); c2 = A[1]**2 + A[2]**2 - 1.0
        disc = b2*b2 - 4*a2*c2
        if disc < 0 or a2 < 1e-12:
            continue
        for tt in [(-b2 - np.sqrt(disc))/(2*a2), (-b2 + np.sqrt(disc))/(2*a2)]:
            if tt > 0:
                pts.append(A + tt*g)
    return np.array(pts)

def two_plane_fit(P, iters=60, tries=40):
    best = 1e9
    for _ in range(tries):
        lab = rng.integers(0, 2, len(P))
        for _ in range(iters):
            planes = []
            for k in range(2):
                Q = P[lab == k]
                if len(Q) < 3:
                    Q = P[rng.choice(len(P), 3, replace=False)]
                c = Q.mean(0); _, _, vt = np.linalg.svd(Q - c); planes.append((c, vt[-1]))
            dist = np.stack([np.abs((P - c) @ n) for c, n in planes], 1)
            lab = np.argmin(dist, 1)
        best = min(best, dist.min(1).max())
    return best

# 本管: 半径1の円筒（軸 x）。球: 中心0 半径1。枝: 頂点Aの円すい
A = np.array([2.0, 2.2, 0.0])
P = cone_cyl_points(A, np.zeros(3), 1.0)
print(f"球に接する円すい×円筒: 交点 {len(P)} 点, 2平面当てはめの最大距離 = {two_plane_fit(P):.2e}")
P = cone_cyl_points(A, np.array([0, 0, 0.0]), 1.0, half_scale=0.85)
print(f"半頂角を0.85倍（球に接しない）: 2平面当てはめの最大距離 = {two_plane_fit(P):.2e}")
