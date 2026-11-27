import sys
import numpy as np
sys.stdout.reconfigure(encoding='utf-8')

def cone_M(A, a, gamma):
    a = a/np.linalg.norm(a)
    S = np.outer(a, a) - np.cos(gamma)**2*np.eye(3)
    M = np.zeros((4, 4)); M[:3, :3] = S; M[:3, 3] = -S @ A; M[3, :3] = -S @ A; M[3, 3] = A @ S @ A
    return M

def cyl_M(R):
    M = np.diag([0.0, 1.0, 1.0, -R*R]); return M

def min_rank_in_pencil(M1, M2):
    xs = np.linspace(-3, 3, 9)
    ys = [np.linalg.det(M1 - x*M2) for x in xs]
    coef = np.polyfit(xs, ys, 4)
    w = np.roots(coef)
    out = []
    for lam in w:
        if not np.isfinite(lam) or abs(lam.imag) > 1e-9:
            continue
        sv = np.linalg.svd(M1 - lam.real*M2, compute_uv=False)
        out.append((lam.real, sv/sv[0]))
    return out

A = np.array([2.0, 2.2, 0.0]); a = -A
g_t = np.arcsin(1.0/np.linalg.norm(A))
for name, g in [("球に接する円すい", g_t), ("半頂角0.85倍", 0.85*g_t)]:
    res = min_rank_in_pencil(cone_M(A, a, g), cyl_M(1.0))
    best = min(res, key=lambda t: t[1][2])
    print(f"{name}: 束 M1-λM2 の中で3番目の特異値(相対)の最小 = {best[1][2]:.2e} (≈0 なら2平面の組＝交線は平面の曲線2本)")
# 同じ球に接する2つの円すい（Y字の枝どうし）
A1 = np.array([2.5, 1.5, 0.0]); A2 = np.array([2.5, -1.5, 0.0])
g1 = np.arcsin(1/np.linalg.norm(A1)); g2 = np.arcsin(1/np.linalg.norm(A2))
res = min_rank_in_pencil(cone_M(A1, -A1, g1), cone_M(A2, -A2, g2))
best = min(res, key=lambda t: t[1][2])
print(f"同じ球に接する円すい2本: 3番目の特異値の最小 = {best[1][2]:.2e}")
A2b = A2 + np.array([0.0, 0.0, 0.4])
g2b = np.arcsin(1/np.linalg.norm(A2b))
res = min_rank_in_pencil(cone_M(A1, -A1, g1), cone_M(A2b, -A2, g2b))
best = min(res, key=lambda t: t[1][2])
print(f"片方の軸を球の中心から外した場合: 3番目の特異値の最小 = {best[1][2]:.2e}")
