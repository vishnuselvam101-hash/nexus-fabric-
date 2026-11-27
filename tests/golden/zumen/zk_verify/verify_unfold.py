import sys
import numpy as np
sys.stdout.reconfigure(encoding='utf-8')

N = 20000
th = np.linspace(0, 2*np.pi, N+1)

def plen(x, y):
    return np.sum(np.hypot(np.diff(x), np.diff(y)))

def plen3(P):
    return np.sum(np.linalg.norm(np.diff(P, axis=0), axis=1))

def shoelace(x, y):
    return 0.5*abs(np.dot(x[:-1], y[1:]) - np.dot(x[1:], y[:-1]))

print("=== A. T字管（枝は本管軸に直角, 偏心 e） 板厚ゼロで確認 ===")
def tee(R, r, e, H=None):
    z = e + r*np.sin(th)
    y = np.sqrt(R**2 - z**2)
    x = r*np.cos(th)
    P = np.stack([x, y, z], 1)
    # 枝の展開: u = r*theta, h = y (本管軸からの高さ)
    u = r*th
    # 本管の穴の展開: 軸方向 x, 周方向 s = R*asin(z/R)
    s = R*np.arcsin(z/R)
    return dict(P=P, u=u, h=y, hx=x, hs=s)

R = 1.0
d = tee(R, 1.0, 0.0)
L3 = plen3(d['P']); Lb = plen(d['u'], d['h']); Lm = plen(d['hx'], d['hs'])
a, b = R*np.sqrt(2), R
ram = np.pi*(3*(a+b) - np.sqrt((3*a+b)*(a+3*b)))
print(f"同径T R=1: 3D交線長={L3:.5f} 枝展開の切り口長={Lb:.5f} 本管穴の展開の周長={Lm:.5f} 楕円(a=√2,b=1)周長={ram:.5f}")
print(f"  枝展開の幅(=円周) = {d['u'][-1]:.5f} (2π={2*np.pi:.5f})")
print(f"  本管穴の展開面積 = {shoelace(d['hx'], d['hs']):.5f} (理論 4R^2 = 4)")
print(f"  切り口の深さ(最大-最小) = {d['h'].max()-d['h'].min():.5f} (理論 R=1)")

for (Rr, rr, ee) in [(1.0, 0.6, 0.0), (1.0, 0.6, 0.3), (1.0, 0.3, 0.7)]:
    d = tee(Rr, rr, ee)
    print(f"R={Rr} r={rr} e={ee}: 3D={plen3(d['P']):.5f} 枝={plen(d['u'], d['h']):.5f} 穴={plen(d['hx'], d['hs']):.5f}")

# 接線出口（AWWA C208 Figure 3）: e = R - r, d = 2r
Do = 2.0; dd = 0.8
R = Do/2; r = dd/2; e = R - r
alpha = np.arccos((0.5*Do - dd)/(0.5*Do))
y_near = np.sqrt(R**2 - (e - r)**2)
print(f"接線出口 Do=2,d=0.8: 手前側の交点高さ={y_near:.5f}  C208の 0.5*Do*sin(α)={0.5*Do*np.sin(alpha):.5f}")
d = tee(R, r, e)
print(f"  奥側(θ=90°)の交点高さ={np.sqrt(max(R**2-(e+r)**2,0)):.5f} (接線なので0)")

print()
print("=== B. 斜め枝（ラテラル, 角度 β, 偏心 e） ===")
def lateral(R, r, e, beta):
    z = e + r*np.sin(th)
    y = np.sqrt(R**2 - z**2)
    t = (y - r*np.cos(th)*np.cos(beta))/np.sin(beta)
    x = t*np.cos(beta) - r*np.cos(th)*np.sin(beta)
    P = np.stack([x, y, z], 1)
    return dict(P=P, u=r*th, t=t, hx=x, hs=R*np.arcsin(z/R))

d90 = lateral(1.0, 0.6, 0.2, np.pi/2); dT = tee(1.0, 0.6, 0.2)
print(f"β=90° と T字管の差(最大) = {np.max(np.abs(d90['t']-dT['h'])):.2e}")
for beta_deg in [45, 60]:
    d = lateral(1.0, 1.0, 0.0, np.radians(beta_deg))
    P = d['P']
    res = []
    for mask in [np.cos(th) >= 0, np.cos(th) < 0]:
        Q = P[mask]; c = Q.mean(0)
        sv = np.linalg.svd(Q - c, compute_uv=False)
        res.append(sv[-1]/np.sqrt(len(Q)))
    print(f"同径ラテラル β={beta_deg}°: 3D={plen3(P):.5f} 枝={plen(d['u'], d['t']):.5f} 穴={plen(d['hx'], d['hs']):.5f}  交線が平面か(半分ずつの平面当てはめ残差)={res[0]:.1e},{res[1]:.1e}")
d = lateral(1.0, 0.5, 0.25, np.radians(45))
print(f"異径偏心ラテラル R=1 r=0.5 e=0.25 β=45°: 3D={plen3(d['P']):.5f} 枝={plen(d['u'], d['t']):.5f} 穴={plen(d['hx'], d['hs']):.5f}")

print()
print("=== C. 板厚: set-on の枝の切り口（例: 本管 外径1016 t12, 枝 外径610 t9, e=0） ===")
Ro = 1016/2; ri = 610/2 - 9; ro = 610/2; rm = (ri+ro)/2
def ycut_seton(Ro, ri, ro, e, n=201):
    rho = np.linspace(ri, ro, n)
    z = e + np.outer(np.sin(th), rho)
    return np.sqrt(Ro**2 - np.min(np.abs(z), axis=1)**2)
y_in = np.sqrt(Ro**2 - (ri*np.sin(th))**2)
y_out = np.sqrt(Ro**2 - (ro*np.sin(th))**2)
y_set = ycut_seton(Ro, ri, ro, 0.0)
print(f"θ=90°(わき): 内面基準の高さ={y_in[N//4]:.2f} 外面基準={y_out[N//4]:.2f} 差(外側のすき=開先)={y_in[N//4]-y_out[N//4]:.2f} mm")
print(f"set-on 規則(壁厚内の最高点)と内面基準の差(e=0, 最大) = {np.max(np.abs(y_set-y_in)):.3f} mm")
e = 150.0
y_set = ycut_seton(Ro, ri, ro, e)
y_in = np.sqrt(Ro**2 - (e + ri*np.sin(th))**2)
k = np.argmax(np.abs(y_set - y_in))
print(f"偏心 e=150 のとき 内面基準と壁厚内最高点の差(最大) = {np.max(np.abs(y_set-y_in)):.2f} mm (θ={np.degrees(th[k]):.1f}°)")
print(f"枝の展開幅: 板厚中心 2π·rm = {2*np.pi*rm:.1f} mm / 内面 2π·ri = {2*np.pi*ri:.1f} / 外面 2π·ro = {2*np.pi*ro:.1f}")

print()
print("=== D. 同心円すいを n 枚割り（板厚中心で） ===")
def cone_pieces(D1, D2, H, n):
    R1, R2 = D1/2, D2/2  # R1 > R2
    sl = np.hypot(H, R1 - R2)
    L1 = R1*sl/(R1 - R2); L2 = L1 - sl
    Theta = 2*np.pi*R1/L1
    return L1, L2, Theta, Theta/n
for n in [1, 2, 3, 4]:
    L1, L2, Th, Tp = cone_pieces(1200, 800, 600, n)
    print(f"n={n}: 外弧半径 L1={L1:.2f} 内弧半径 L2={L2:.2f} 全体角={np.degrees(Th):.3f}° 1枚の角={np.degrees(Tp):.3f}°  1枚の外弧長={L1*Tp:.2f} (π·D1/n={np.pi*1200/n:.2f}) 内弧長={L2*Tp:.2f} (π·D2/n={np.pi*800/n:.2f})")
t = 12.0; Di1 = 1188.0; Di2 = 788.0; H = 600.0
alpha = np.arctan((Di1 - Di2)/2/H)
print(f"厚板の円すい: 半頂角={np.degrees(alpha):.2f}°, 端面での板厚中心径 = 内径 + t/cosα = {Di1 + t/np.cos(alpha):.2f} (円筒と同じ考えの 内径+t = {Di1+t:.2f})")
print(f"  板の端を板面に直角に切ると、内外の端の軸方向のずれ = t·sinα = {t*np.sin(alpha):.2f} mm")

print()
print("=== E. 偏心円すい（片落管）の展開: 頂点からの母線と単位球上の弧長 ===")
def oblique_cone(R1, R2, H, off, n=3600):
    # 大径 R1 の円 z=0 中心(0,0)、小径 R2 の円 z=H 中心(off,0)。同じ方位の点を結ぶ母線は頂点を通る
    phi = np.linspace(0, 2*np.pi, n+1)
    k = R2/R1
    A = np.array([off, 0, H])/(1-k)  # 相似の中心
    P1 = np.stack([R1*np.cos(phi), R1*np.sin(phi), 0*phi], 1)
    P2 = np.stack([off + R2*np.cos(phi), R2*np.sin(phi), H + 0*phi], 1)
    chk = np.cross(P1 - A, P2 - A)
    U = (P1 - A); d1 = np.linalg.norm(U, axis=1); U = U/d1[:, None]
    d2 = np.linalg.norm(P2 - A, axis=1)
    psi = np.concatenate([[0], np.cumsum(np.arccos(np.clip(np.sum(U[1:]*U[:-1], 1), -1, 1)))])
    B = np.stack([d1*np.cos(psi), d1*np.sin(psi)], 1)
    T = np.stack([d2*np.cos(psi), d2*np.sin(psi)], 1)
    return phi, psi, B, T, np.max(np.linalg.norm(chk, axis=1))
phi, psi, B, T, colin = oblique_cone(600, 400, 700, 200)
print(f"同じ方位の点を結ぶ線が頂点を通るか(外積の最大) = {colin:.2e}")
print(f"展開後の大径側の弧長 = {plen(B[:,0], B[:,1]):.3f} (π·1200 = {np.pi*1200:.3f})")
print(f"展開後の小径側の弧長 = {plen(T[:,0], T[:,1]):.3f} (π·800 = {np.pi*800:.3f})")
print(f"全体の展開角 = {np.degrees(psi[-1]):.3f}°")
phi0, psi0, B0, T0, _ = oblique_cone(600, 400, 700, 0.0)
sl = np.hypot(700, 200); L1 = 600*sl/200
print(f"偏心0なら 展開角 = {np.degrees(psi0[-1]):.4f}° と 2π·R1/L1 = {np.degrees(2*np.pi*600/L1):.4f}° が一致")
for n in [3, 4]:
    idx = [j*(len(phi)-1)//n for j in range(n+1)]
    idx[-1] = len(phi)-1
    angs = [np.degrees(psi[idx[j+1]] - psi[idx[j]]) for j in range(n)]
    print(f"{n}つ割（継ぎ目を方位で等分）: 各片の展開角 = {[round(float(a),3) for a in angs]}  ← 偏心だと片ごとに形が違う")

print()
print("=== F. 円すいに直角に立つ枝（枝軸は円すい軸に直角） ===")
def branch_on_cone(Ra, Rb, Lc, xb, r, e):
    # 円すい軸 x, x=0 で半径 Ra, x=Lc で半径 Rb
    k = (Rb - Ra)/Lc
    x = xb + r*np.cos(th); z = e + r*np.sin(th)
    rho = Ra + k*x
    y = np.sqrt(rho**2 - z**2)
    P = np.stack([x, y, z], 1)
    # 円すいの展開（頂点基準）
    xa = -Ra/k if k != 0 else None
    alpha = np.arctan(abs(k))
    sdist = np.sqrt((x - xa)**2 + rho**2)
    phi = np.arctan2(z, y)
    psi = phi*np.sin(alpha)
    hx, hy = sdist*np.cos(psi), sdist*np.sin(psi)
    return P, r*th, y, hx, hy
P, u, h, hx, hy = branch_on_cone(1000, 600, 1500, 750, 300, 0.0)
print(f"3D交線長={plen3(P):.3f} 枝の展開の切り口長={plen(u, h):.3f} 円すいの穴(展開後)の周長={plen(hx, hy):.3f}")
P, u, h, hx, hy = branch_on_cone(1000, 999.999, 1500, 750, 300, 0.0)
dT = tee(1000, 300, 0.0)
print(f"テーパをほぼ0にすると T字管の式との差(最大) = {np.max(np.abs(h - dT['h'])):.2e}")

print()
print("=== G. 共通の球に接する2つの円すい/円筒の交線は平面になるか（Y字管・分岐の設計の考え方） ===")
def cone_surface_pts(apex, axis, half, sphere_c, rho_s, n=400, m=400):
    pass
# 球: 中心0 半径 s。本管: 半径 s の円筒(軸 x)。枝: 球に接する円すい（頂点 A, 軸は A→0）
s = 1.0
A = np.array([2.0, 2.2, 0.0])
dA = np.linalg.norm(A); ax = -A/dA; half = np.arcsin(s/dA)
# 円すい面と円筒面 y^2+z^2=s^2 の交線を、円すいの母線ごとに解く
pts = []
e1 = np.cross(ax, [0, 0, 1.0]); e1 /= np.linalg.norm(e1); e2 = np.cross(ax, e1)
for ang in np.linspace(0, 2*np.pi, 721)[:-1]:
    g = np.cos(half)*ax + np.sin(half)*(np.cos(ang)*e1 + np.sin(ang)*e2)
    # A + t g が y^2+z^2=s^2
    a2 = g[1]**2 + g[2]**2; b2 = 2*(A[1]*g[1] + A[2]*g[2]); c2 = A[1]**2 + A[2]**2 - s**2
    disc = b2*b2 - 4*a2*c2
    if disc < 0 or a2 < 1e-12:
        continue
    for tt in [(-b2 - np.sqrt(disc))/(2*a2), (-b2 + np.sqrt(disc))/(2*a2)]:
        if tt > 0:
            pts.append(A + tt*g)
pts = np.array(pts)
# 2枚の平面に分かれるはず。x 符号で分けて当てはめ
for name, msk in [("近い側", np.linalg.norm(pts - A, axis=1) < np.median(np.linalg.norm(pts - A, axis=1)))]:
    pass
from itertools import product
best = []
dist = np.linalg.norm(pts - A, axis=1)
# 母線ごとに近い交点(1つめ)だけ集める
near = []
for ang in np.linspace(0, 2*np.pi, 721)[:-1]:
    g = np.cos(half)*ax + np.sin(half)*(np.cos(ang)*e1 + np.sin(ang)*e2)
    a2 = g[1]**2 + g[2]**2; b2 = 2*(A[1]*g[1] + A[2]*g[2]); c2 = A[1]**2 + A[2]**2 - s**2
    disc = b2*b2 - 4*a2*c2
    if disc < 0 or a2 < 1e-12:
        continue
    tt = (-b2 - np.sqrt(disc))/(2*a2)
    if tt > 0:
        near.append(A + tt*g)
near = np.array(near)
c = near.mean(0); sv = np.linalg.svd(near - c, compute_uv=False)
print(f"円筒(半径1)と、同じ球に接する円すい: 近い側の交線 {len(near)}点 の平面当てはめ残差 = {sv[-1]/np.sqrt(len(near)):.2e} (0なら平面)")
A2 = A + np.array([0.0, 0.0, 0.3])  # 球に接しない円すい（軸を少しずらす）
ax2 = -(A2 - np.array([0, 0, 0.2])); ax2 /= np.linalg.norm(ax2)
e1 = np.cross(ax2, [0, 0, 1.0]); e1 /= np.linalg.norm(e1); e2 = np.cross(ax2, e1)
near = []
for ang in np.linspace(0, 2*np.pi, 721)[:-1]:
    g = np.cos(half)*ax2 + np.sin(half)*(np.cos(ang)*e1 + np.sin(ang)*e2)
    a2 = g[1]**2 + g[2]**2; b2 = 2*(A2[1]*g[1] + A2[2]*g[2]); c2 = A2[1]**2 + A2[2]**2 - s**2
    disc = b2*b2 - 4*a2*c2
    if disc < 0 or a2 < 1e-12:
        continue
    tt = (-b2 - np.sqrt(disc))/(2*a2)
    if tt > 0:
        near.append(A2 + tt*g)
near = np.array(near)
c = near.mean(0); sv = np.linalg.svd(near - c, compute_uv=False)
print(f"比較: 球に接しない円すい: 平面当てはめ残差 = {sv[-1]/np.sqrt(len(near)):.2e}")

print()
print("=== H. 補強板（カラー）: 平面視で円形の当て板を本管外面に沿わせる ===")
Ro = 1016/2; tp = 12.0; Rpm = Ro + tp/2
ro_b = 610/2; Dp = 1000.0  # 当て板の平面視の外径
def pad(Rsurf, Rdev, rho, e=0.0):
    z = e + rho*np.sin(th); x = rho*np.cos(th)
    phi = np.arcsin(z/Rsurf)
    return x, Rdev*phi
xi, si = pad(Ro, Rpm, ro_b)      # 内側の穴: 枝外面が本管外面に当たる線（角度）を、板厚中心の長さで
xo, so = pad(Ro + tp, Rpm, Dp/2)  # 外周: 平面視の円を当て板の外面で切る（推論の置き方の一例）
print(f"当て板の展開: 周方向の全長(外周) = {so.max()-so.min():.1f} mm（平面視の幅 {Dp:.0f} mm より長い）  軸方向の全長 = {xo.max()-xo.min():.1f} mm")
print(f"  内側の穴: 周方向 {si.max()-si.min():.1f} mm / 軸方向 {xi.max()-xi.min():.1f} mm（平面視では {2*ro_b:.0f} mm の円）")
print(f"  当て板の面積(展開) = {shoelace(xo, so) - shoelace(xi, si):.0f} mm^2")
