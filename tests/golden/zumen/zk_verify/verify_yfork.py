# 二股（Y字管）の展開の期待値を、画面の計算（shared.js）と別に出す。python verify_yfork.py
# 座標: 交点（3本の管の中心線が会う点）を原点。本管は -x の向きに伸びる。x は本管の軸をまっすぐ延ばした向き。
# 枝1 は +y の側へ β1、枝2 は -y の側へ β2 だけ x から振れる（Y の面は xy）。z は Y の面に直角（股の点の向き）。
# どの管も「共通の球」（中心は交点、半径 Rs＝本管の外半径）に接する円筒か円すい（ψ＝0 が円筒）として扱う。
# 片 i の母線は、管の中心線に沿う位置 s（交点から）と、周りの角 θ（0＝相手1の側の Y の面の線、+90°＝+z の股の点）で表す。
# 周方向の長さは板厚の基準（既定 中心）、切り口の位置は振れ幅の面（既定 外面）で決める。えび曲げの片と同じ決まり。
import math, sys
sys.stdout.reconfigure(encoding="utf-8")
sin, cos, tan, asin, acos, atan2, sqrt, pi = math.sin, math.cos, math.tan, math.asin, math.acos, math.atan2, math.sqrt, math.pi
rad, deg = math.radians, math.degrees
RHO = 7.85e-6
NG = []


def dot(a, b): return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
def add(a, b): return (a[0] + b[0], a[1] + b[1], a[2] + b[2])
def mul(a, k): return (a[0] * k, a[1] * k, a[2] * k)
def sub(a, b): return (a[0] - b[0], a[1] - b[1], a[2] - b[2])
def nrm(a): return sqrt(dot(a, a))
def unit(a): return mul(a, 1 / nrm(a))


def check(ok, name, info=""):
    if not ok:
        NG.append(name)
        print(f"  NG {name} {info}")


class Fork:
    # Rs: 共通の球の半径（本管の外半径）。b1・b2: 枝の振れ（度）。psi1・psi2: 枝の円すいの片側の角（度。0＝円筒）
    def __init__(self, Rs, b1, b2, psi1=0.0, psi2=0.0):
        self.Rs = Rs
        self.u = [(-1.0, 0.0, 0.0), (cos(rad(b1)), sin(rad(b1)), 0.0), (cos(rad(b2)), -sin(rad(b2)), 0.0)]
        self.psi = [0.0, rad(psi1), rad(psi2)]
        self.nb = [(1, 2), (0, 2), (0, 1)]  # 相手1・相手2（本管→枝1・枝2、枝→本管・もう片方の枝）
        self.e2 = (0.0, 0.0, 1.0)
        self.e1 = []
        for i in range(3):
            # Y の面の中で中心線に直角な向き。相手1 の側を正にとる（相手1 がちょうど反対の向きなら、相手2 と反対の側）
            u = self.u[i]
            e = unit((-u[1], u[0], 0.0))
            a1, a2 = dot(e, self.u[self.nb[i][0]]), dot(e, self.u[self.nb[i][1]])
            if a1 < -1e-12 or (abs(a1) <= 1e-12 and a2 > 0):
                e = mul(e, -1)
            self.e1.append(e)

    def g(self, j, P):
        # 球からの「離れ」: g_i ≥ g_j の所が片 i の残る側（円筒どうしなら 2 本の中心線の二等分の面）
        return (dot(P, self.u[j]) - self.Rs * sin(self.psi[j])) / cos(self.psi[j])

    def rho(self, i, s, ao=0.0):
        # 片 i の、中心線に沿って s の所の半径（ao は外面から内へずらす量）
        return (self.Rs - s * sin(self.psi[i]) - ao) / cos(self.psi[i])

    def point(self, i, s, th, ao=0.0):
        w = add(mul(self.e1[i], cos(th)), mul(self.e2, sin(th)))
        return add(mul(self.u[i], s), mul(w, self.rho(i, s, ao)))

    def scut_j(self, i, j, th, ao=0.0):
        # g_i − g_j は母線に沿って s の1次式。0 になる s
        f = lambda s: self.g(i, self.point(i, s, th, ao)) - self.g(j, self.point(i, s, th, ao))
        f0, f1 = f(0.0), f(1.0)
        A = f1 - f0
        return -f0 / A, A

    def scut(self, i, th, ao=0.0):
        best, who = -1e18, None
        for j in self.nb[i]:
            s, A = self.scut_j(i, j, th, ao)
            if A <= 0:
                raise ValueError("母線が相手から抜けない")
            if s > best:
                best, who = s, j
        return best, who

    def apex(self, i, o=0.0):
        # 円すいの頂点（中心線に沿って）。板厚の基準の面は外面から o 内側
        return (self.Rs - o) / sin(self.psi[i])

    def crotch(self):
        # 3 つの g が等しい線（z に平行）と本管の面の交わり
        # -x = (x c1 + y s1 - k1)/C1, -x = (x c2 - y s2 - k2)/C2
        (c1, s1, _), (c2, m2, _) = self.u[1], self.u[2]
        C1, C2 = cos(self.psi[1]), cos(self.psi[2])
        k1, k2 = self.Rs * sin(self.psi[1]), self.Rs * sin(self.psi[2])
        a11, a12, r1 = c1 + C1, s1, k1
        a21, a22, r2 = c2 + C2, m2, k2
        det = a11 * a22 - a12 * a21
        x = (r1 * a22 - a12 * r2) / det
        y = (a11 * r2 - r1 * a21) / det
        return x, y, sqrt(max(self.Rs ** 2 - y * y, 0.0))


def piece(F, i, s_end, t, o, ao, ths=-pi / 4, M=3600):
    # 片 i の展開: 板の横 u は継ぎ目（θ=ths）から周の長さ、縦 v は切り口から管端（または円すいの小さい口）まで母線に沿って
    psi = F.psi[i]
    cut = []
    for k in range(M + 1):
        th = ths + 2 * pi * k / M
        cut.append((th, F.scut(i, th, ao)[0]))
    if psi == 0:
        rb = F.Rs - o
        top = [(rb * (th - ths), s_end - s) for th, s in cut]
        poly = [(0.0, 0.0), (2 * pi * rb, 0.0)] + top[::-1]
        circ = 2 * pi * rb
    else:
        sab = F.apex(i, o)
        lb = lambda s: (sab - s) / cos(psi)
        pol = lambda th, s: (lb(s) * sin((th - ths) * sin(psi)), lb(s) * cos((th - ths) * sin(psi)))
        top = [pol(th, s) for th, s in cut]
        small = [pol(th, s_end) for th, s in cut]
        poly = small + top[::-1]
        circ = 2 * pi * F.rho(i, s_end, o)
    n = len(poly)
    area = abs(sum(poly[k][0] * poly[(k + 1) % n][1] - poly[(k + 1) % n][0] * poly[k][1] for k in range(n))) / 2
    xs, ys = [p[0] for p in poly], [p[1] for p in poly]
    lens = [(s_end - s) / cos(psi) for th, s in cut]
    marks = [(s_end - F.scut(i, rad(22.5 * k), ao)[0]) / cos(psi) for k in range(16)]
    return {"circ": circ, "area": area, "kg": area * t * RHO, "w": max(xs) - min(xs), "h": max(ys) - min(ys), "short": min(lens), "long": max(lens),
            "marks": marks, "poly": poly, "cut": cut}


def switch_angles(F, i, ao=0.0):
    # 片 i の切り口で、切る相手が入れ替わる角（股の点）。2 つ
    j1, j2 = F.nb[i]
    f = lambda th: F.scut_j(i, j1, th, ao)[0] - F.scut_j(i, j2, th, ao)[0]
    out, N0 = [], 3600
    for k in range(N0):
        a, b = 2 * pi * k / N0, 2 * pi * (k + 1) / N0
        if f(a) * f(b) < 0:
            for _ in range(80):
                m = (a + b) / 2
                if f(a) * f(m) <= 0:
                    b = m
                else:
                    a = m
            out.append((a + b) / 2)
    return out


def seam_len(F, i, j, ao=0.0, N=20000):
    # 片 i の切り口のうち、相手 j で切られる所の 3D の長さ（股の点から股の点まで）
    a, b = switch_angles(F, i, ao)
    if F.scut(i, (a + b) / 2, ao)[1] != j:
        a, b = b, a + 2 * pi
    L, prev = 0.0, None
    for k in range(N + 1):
        th = a + (b - a) * k / N
        P = F.point(i, F.scut_j(i, j, th, ao)[0], th, ao)
        if prev is not None:
            L += nrm(sub(P, prev))
        prev = P
    return L


def ellipse_half(a, b, N=200000):
    # 半径 a・b の楕円の半周の長さ（数値積分）
    return sum(sqrt((a * sin(pi * (k + 0.5) / N)) ** 2 + (b * cos(pi * (k + 0.5) / N)) ** 2) for k in range(N)) * pi / N


def surf_dist(F, j, P, ao=0.0):
    # 点 P と相手 j の面（中心線からの距離が半径）の差。円すいは中心線に沿う位置で半径が変わる
    s = dot(P, F.u[j])
    r = nrm(sub(P, mul(F.u[j], s)))
    return r - F.rho(j, s, ao)


def show(name, v, d=2):
    print(f"  {name}: {v:.{d}f}")


# ---------------- 確かめ ----------------
print("== 確かめ1: 同じ径の二股。切り口は平面 2 枚。半周ずつ えび曲げの切り口（片側の角 γ）と同じ ==")
R = 508.0
for (b1, b2) in [(45, 45), (30, 30), (30, 45), (15, 60), (60, 60)]:
    F = Fork(R, b1, b2)
    D = b1 + b2
    # 片ごとの (相手1 の側の係数, 相手2 の側の係数)。振れ幅 = R × 係数 × |cosθ|
    K = {0: (tan(rad(b1) / 2), tan(rad(b2) / 2)), 1: (tan(rad(b1) / 2), 1 / tan(rad(D) / 2)), 2: (tan(rad(b2) / 2), 1 / tan(rad(D) / 2))}
    err = 0.0
    for i in range(3):
        for k in range(720):
            th = 2 * pi * k / 720
            s = F.scut(i, th)[0]
            ex = R * K[i][0] * cos(th) if cos(th) >= 0 else -R * K[i][1] * cos(th)
            err = max(err, abs(s - ex))
    print(f"  β1={b1}° β2={b2}°（挟む角 {D}°）: 式との差 {err:.1e}。係数 本管 {K[0][0]:.5f}/{K[0][1]:.5f}・枝1 {K[1][0]:.5f}/{K[1][1]:.5f}・枝2 {K[2][0]:.5f}/{K[2][1]:.5f}")
    check(err < 1e-9, "同径の式", f"{b1},{b2}")
print("  えび曲げの片の振れ幅 k＝R·tanγ（shared.js bendPiece）と比べると、本管の枝1 の側は γ=β1/2、枝の本管の側は γ=β/2、枝どうしの側は γ=90°−(β1+β2)/2")

print("== 確かめ2: 3D の総当たり（同じ径・板厚0）。交点から伸びる3本の半分の円筒の中に入る所を、母線ごとに二分法で探す ==")


def kept_eq(F, i, P):
    for j in range(3):
        if j == i:
            continue
        sj = dot(P, F.u[j])
        if sj > 0 and nrm(sub(P, mul(F.u[j], sj))) < F.Rs - 1e-12:
            return False
    return True


worst = 0.0
for (b1, b2) in [(45, 45), (30, 45), (15, 60), (70, 20)]:
    F = Fork(R, b1, b2)
    for i in range(3):
        for k in range(97):
            th = 2 * pi * (k + 0.37) / 97
            grid = [3 * R * q / 600 for q in range(601)]
            flags = [kept_eq(F, i, F.point(i, s, th)) for s in grid]
            ch = [q for q in range(600) if flags[q] != flags[q + 1]]
            check(len(ch) <= 1 and flags[-1], "総当たり 1 回だけ変わる", f"{b1},{b2},{i},{k},{ch}")
            if not ch:
                check(F.scut(i, th)[0] < 3 * R / 600, "総当たりで切り口が見つからない", f"{b1},{b2},{i},{k}")
                continue
            lo, hi = grid[ch[0]], grid[ch[0] + 1]
            for _ in range(60):
                mid = (lo + hi) / 2
                if kept_eq(F, i, F.point(i, mid, th)):
                    hi = mid
                else:
                    lo = mid
            worst = max(worst, abs(hi - F.scut(i, th)[0]))
show("一番大きい差 mm", worst, 6)
check(worst < 1e-6, "総当たり")

print("== 確かめ3: 継ぎ目の長さ（3D）。両側の片から測った長さと、平面で切った円筒の楕円の半周が同じ ==")
for (b1, b2) in [(45, 45), (30, 45)]:
    F = Fork(R, b1, b2)
    D = b1 + b2
    for (i, j, a) in [(0, 1, 180 - b1), (0, 2, 180 - b2), (1, 2, D)]:
        # 管 i と j の中心線のなす角 a。切る面と中心線のなす角は 90°−a/2 なので、楕円の長い半径は R/sin(a/2)
        l1, l2, le = seam_len(F, i, j), seam_len(F, j, i), ellipse_half(R, R / sin(rad(a) / 2))
        print(f"  β1={b1} β2={b2} 片{i}と片{j}: {l1:.3f} / {l2:.3f} / 楕円の半周 {le:.3f}")
        check(abs(l1 - le) < 1e-3 and abs(l2 - le) < 1e-3, "継ぎ目の長さ", f"{b1},{b2},{i},{j}")

print("== 確かめ4: 極限で今の形と一致 ==")
# (a) 枝2 をまっすぐ（β2=0）: 枝1 は同径のラテラル（e=0・板厚0・角 α=β1）の枝の切り口。spec_lateral.md の wall_tau
F = Fork(R, 45, 0)
wall_tau = lambda Rc, rho, th, e, a: (sqrt(Rc * Rc - (e + rho * sin(th)) ** 2) + rho * cos(th) * cos(a)) / sin(a)
err = max(abs(F.scut(1, th)[0] - wall_tau(R, R, th + pi, 0, rad(45))) for th in [2 * pi * k / 720 for k in range(720)])
show("(a) β2=0 の枝1 とラテラル 45° の枝の切り口の差", err, 9)
check(err < 1e-8, "ラテラルの極限")
# 本管（-x 側）と枝2（+x 側）を合わせると、ラテラルの本管の穴（板厚0・spec_lateral §4.2 の足あと）になる
err = 0.0
for k in range(1, 180):
    phi = -pi / 2 + pi * k / 180
    lo = -F.scut(0, phi)[0]
    hi = F.scut(2, pi - phi)[0]
    c, w = R * cos(phi) / tan(rad(45)), sqrt(R * R - (R * sin(phi)) ** 2) / sin(rad(45))
    err = max(err, abs(lo - (c - w)), abs(hi - (c + w)))
show("(a) 本管＋枝2 の切り口と、ラテラルの本管の穴の縁の差", err, 9)
check(err < 1e-8, "ラテラルの穴の極限")
# (b) β1=β2=90: 本管は同径の T字管の枝（teeY・e=0・板厚0: √(R²−(R sinθ)²)）
F = Fork(R, 90, 90)
err = max(abs(F.scut(0, th)[0] - sqrt(max(R * R - (R * sin(th)) ** 2, 0))) for th in [2 * pi * k / 720 for k in range(720)])
show("(b) β1=β2=90 の本管と、同径の T字管の枝の切り口の差", err, 9)
check(err < 1e-8, "T字管の極限")
# (c) 円すいの角 ψ を 0 に近づけると、同じ径の二股に続く
Fe, Fc = Fork(R, 30, 30), Fork(R, 30, 30, 1e-4, 1e-4)
err = max(abs(Fe.scut(i, th)[0] - Fc.scut(i, th)[0]) for i in range(3) for th in [2 * pi * k / 360 for k in range(360)])
show("(c) ψ=0.0001° の円すいの二股と同じ径の二股の差", err, 6)
check(err < 0.01, "円すいの極限")

print("== 確かめ5: 面積。展開図の面積＝2π·r·H − 2·r·R·(係数1＋係数2)（同じ径・板厚0） ==")
F = Fork(R, 30, 45)
for (i, H) in [(0, 500.0), (1, 1500.0), (2, 1200.0)]:
    p = piece(F, i, H, 0, 0.0, 0.0)
    K1 = [tan(rad(30) / 2), tan(rad(30) / 2), tan(rad(45) / 2)][i]
    K2 = [tan(rad(45) / 2), 1 / tan(rad(75) / 2), 1 / tan(rad(75) / 2)][i]
    ex = 2 * pi * R * H - 2 * R * R * (K1 + K2)
    print(f"  片{i}: 展開図 {p['area']:.1f}・式 {ex:.1f}")
    check(abs(p["area"] - ex) < 1.0, "面積", str(i))

print("== 確かめ6: AWWA C208 の二股（Wye）の寸法の幾何の部分（余裕 f を除く） ==")
for Dlt in (90, 60, 30):
    F = Fork(R, Dlt / 2, Dlt / 2)
    mT = max(F.scut(0, th)[0] for th in [2 * pi * k / 3600 for k in range(3600)])
    mB = max(F.scut(1, th)[0] for th in [2 * pi * k / 3600 for k in range(3600)])
    print(f"  挟む角 {Dlt}°: 本管の切り口の一番深い所 {mT:.2f}（F の (Do/2)·tan(Δ/4) {R*tan(rad(Dlt)/4):.2f}）・枝の股の側 {mB:.2f}（Hy の 0.5Do/tan(Δ/2) {R/tan(rad(Dlt)/2):.2f}）")
    check(abs(mT - R * tan(rad(Dlt) / 4)) < 0.01 and abs(mB - R / tan(rad(Dlt) / 2)) < 0.01, "AWWA", str(Dlt))
fDo = lambda Do: 0.143 * Do + 127
for (Do, Dlt) in [(1016, 90), (1625.6, 60)]:
    print(f"  AWWA の最小の長さ Do={Do}・Δ={Dlt}°: F={Do/2*tan(rad(Dlt)/4)+fDo(Do):.1f}・Hy={fDo(Do)/sin(rad(Dlt)/2)+0.5*Do/tan(rad(Dlt)/2):.1f}（f={fDo(Do):.1f}）")

print("== 確かめ7: 枝の径が本管より細い形を円筒だけで作れるか ==")
# 枝の円筒の点は Y の面から |z| ≤ r。本管の面の |z| > r の所（股の点のまわり）はどの枝にも届かず、閉じない
for (Do, d) in [(1016, 610), (1016, 914.4), (1016, 1016)]:
    print(f"  本管 {Do}・枝 {d}: 本管の股の点のまわりで枝が届かない高さ {(Do - d) / 2:.1f}（上下に。0 なら閉じる）")

print("== 確かめ8: 円すいの二股（共通の球）。切り口が相手の面の上にあるか・つなぐ面で 2 つの面の切り口が同じか ==")


def psi_from(Rs, r, sr):
    # 交点から円すいの小さい口までの長さ sr と、小さい口の外半径 r から、円すいの片側の角
    return deg(asin(Rs / sqrt(sr * sr + r * r)) - atan2(r, sr))


def inside(F, j, P):
    s = dot(P, F.u[j])
    rr = F.rho(j, s)
    return rr > 0 and nrm(sub(P, mul(F.u[j], s))) < rr


import random
random.seed(1)
for (Do, d, b1, b2, sr) in [(1016, 610, 30, 30, 900), (1016, 610, 30, 45, 900), (1625.6, 1117.6, 25, 25, 1500), (610, 1016, 30, 30, 900)]:
    Rs, r = Do / 2, d / 2
    ps = psi_from(Rs, r, sr)
    F = Fork(Rs, b1, b2, ps, ps)
    worst_on = 0.0
    for i in range(3):
        for k in range(360):
            th = 2 * pi * (k + 0.5) / 360
            s, who = F.scut(i, th)
            worst_on = max(worst_on, abs(surf_dist(F, who, F.point(i, s, th))))
    # つなぐ面（g_i＝g_j で、残りの g より大きい所）の上の点では、片 i の面の内と片 j の面の内が同じ（つなぐ面で 2 つの面の切り口が重なる）
    bad, tot = 0, 0
    for (i, j) in [(0, 1), (0, 2), (1, 2)]:
        k3, cnt = 3 - i - j, 0
        while cnt < 20000:
            P = (random.uniform(-2 * Rs, 2 * Rs), random.uniform(-2 * Rs, 2 * Rs), random.uniform(-1.2 * max(Rs, r), 1.2 * max(Rs, r)))
            f0 = F.g(i, P) - F.g(j, P)
            f1 = F.g(i, add(P, F.u[i])) - F.g(j, add(P, F.u[i]))
            P = add(P, mul(F.u[i], -f0 / (f1 - f0)))
            if F.g(i, P) < F.g(k3, P):
                continue
            if min(abs(surf_dist(F, i, P)), abs(surf_dist(F, j, P))) < 1e-6:
                continue
            cnt += 1
            if inside(F, i, P) != inside(F, j, P):
                bad += 1
        tot += cnt
    xc, yc, zc = F.crotch()
    on = max(abs(surf_dist(F, j, (xc, yc, zc))) for j in range(3))
    print(f"  {Do}・{d}・β {b1}/{b2}・円すいの小さい口まで {sr}: ψ={ps:.4f}°・切り口が相手の面から離れる一番大きい量 {worst_on:.2e}・つなぐ面の点 {tot} のうち内外が食い違う点 {bad}・股の点 ({xc:.2f}, {yc:.2f}, ±{zc:.2f}) が3つの面から離れる量 {on:.1e}")
    check(worst_on < 1e-6 and bad == 0 and on < 1e-6, "円すいの二股", f"{Do},{d},{b1},{b2}")
    check(abs(F.rho(1, sr) - r) < 1e-9, "円すいの小さい口")
    for (i, j) in [(0, 1), (0, 2), (1, 2)]:
        l1, l2 = seam_len(F, i, j), seam_len(F, j, i)
        print(f"    継ぎ目 片{i}と片{j} の長さ（両側から）: {l1:.4f} / {l2:.4f}")
        check(abs(l1 - l2) < 1e-3, "円すいの継ぎ目の長さ", f"{i},{j}")
# 閉じた式との照らし（対称・ψ<β）: 円すいの外側の線が本管のわきの線に当たる所、円すいの股の側の線が二等分の面に当たる所
Rs, r, b, sr = 508.0, 305.0, 30.0, 900.0
ps = psi_from(Rs, r, sr)
F = Fork(Rs, b, b, ps, ps)
s_out = Rs * sin(rad(b + ps) / 2) / cos(rad(b - ps) / 2)
s_in = Rs * cos(rad(b)) / sin(rad(b + ps))
print(f"  外側の母線 {F.scut(1, 0)[0]:.4f}（式 R·sin((β+ψ)/2)/cos((β−ψ)/2) = {s_out:.4f}）・股の側の母線 {F.scut(1, pi)[0]:.4f}（式 R·cosβ/sin(β+ψ) = {s_in:.4f}）")
check(abs(F.scut(1, 0)[0] - s_out) < 1e-9 and abs(F.scut(1, pi)[0] - s_in) < 1e-9, "円すいの閉じた式")

print("== 確かめ8b: 切り口の位置の閉じた式（spec §4.2）と、1次式を解いた値が同じか。円すいの角の限り ψ < (β1+β2)/2 ==")


def scut_closed(F, i, j, th, ao=0.0):
    pi_, pj = F.psi[i], F.psi[j]
    c, a = dot(F.u[i], F.u[j]), dot(F.e1[i], F.u[j])
    A = 1 / cos(pi_) - c / cos(pj) + a * cos(th) * tan(pi_) / cos(pj)
    B = F.Rs * sin(pi_) / cos(pi_) + (F.Rs - ao) * a * cos(th) / (cos(pi_) * cos(pj)) - F.Rs * sin(pj) / cos(pj)
    return B / A, A


err = 0.0
for (b1, b2, ps, ao) in [(45, 45, 0, 0), (30, 45, 0, 6), (30, 30, 13.5946, 0), (30, 45, 13.5946, 6), (30, 30, -12.2775, 0)]:
    F = Fork(508, b1, b2, ps, ps)
    for i in range(3):
        for j in F.nb[i]:
            for k in range(72):
                th = 2 * pi * k / 72
                err = max(err, abs(scut_closed(F, i, j, th, ao)[0] - F.scut_j(i, j, th, ao)[0]))
show("閉じた式との一番大きい差", err, 9)
check(err < 1e-6, "閉じた式")
for (b1, b2) in [(30, 30), (30, 45)]:
    lim = (b1 + b2) / 2
    for ps in (lim - 0.01, lim + 0.01):
        F = Fork(508, b1, b2, ps, ps)
        ok = True
        try:
            for i in range(3):
                for k in range(720):
                    F.scut(i, 2 * pi * k / 720)
        except ValueError:
            ok = False
        print(f"  β {b1}/{b2}・ψ={ps:.2f}°: {'出せる' if ok else '出せない（枝の円すいどうしの母線が相手から抜けない）'}")
        check(ok == (ps < lim), "円すいの角の限り", f"{b1},{b2},{ps}")

print("== 確かめ9: 円すいの片の展開が形を保つか（3D の切り口の長さ＝展開図の切り口の長さ、3D の面積＝展開図の面積。板厚0） ==")
for (Do, d, b1, b2, sr) in [(1016, 610, 30, 30, 900), (1016, 610, 30, 45, 900)]:
    Rs, r = Do / 2, d / 2
    ps = psi_from(Rs, r, sr)
    F = Fork(Rs, b1, b2, ps, ps)
    for i in (1, 2):
        p = piece(F, i, sr, 0, 0.0, 0.0, M=20000)
        l2 = sum(math.dist(p["poly"][k], p["poly"][k + 1]) for k in range(20001, 2 * 20001 - 1))
        l3 = 0.0
        for k in range(20000):
            P0 = F.point(i, p["cut"][k][1], p["cut"][k][0])
            P1 = F.point(i, p["cut"][k + 1][1], p["cut"][k + 1][0])
            l3 += nrm(sub(P1, P0))
        # 3D の面積: 切り口から小さい口までを細かい三角に割って足す
        A3, NT, NS = 0.0, 1000, 60
        for k in range(NT):
            tha, thb = p["cut"][k * (20000 // NT)][0], p["cut"][(k + 1) * (20000 // NT)][0]
            sa, sb = p["cut"][k * (20000 // NT)][1], p["cut"][(k + 1) * (20000 // NT)][1]
            for q in range(NS):
                fa, fb = q / NS, (q + 1) / NS
                P00 = F.point(i, sa + (sr - sa) * fa, tha); P01 = F.point(i, sa + (sr - sa) * fb, tha)
                P10 = F.point(i, sb + (sr - sb) * fa, thb); P11 = F.point(i, sb + (sr - sb) * fb, thb)
                for (A_, B_, C_) in ((P00, P10, P11), (P00, P11, P01)):
                    cx = sub(B_, A_); cy = sub(C_, A_)
                    cr = (cx[1] * cy[2] - cx[2] * cy[1], cx[2] * cy[0] - cx[0] * cy[2], cx[0] * cy[1] - cx[1] * cy[0])
                    A3 += nrm(cr) / 2
        print(f"  {Do}・{d}・β {b1}/{b2} 円すい{i}: 切り口 3D {l3:.3f}・展開図 {l2:.3f}／面積 3D {A3:.0f}・展開図 {p['area']:.0f}")
        check(abs(l3 - l2) < 0.05 and abs(A3 - p["area"]) / p["area"] < 2e-4, "円すいの展開", f"{i}")

print("== 確かめ10: 板厚の決まりの効き方（同じ径・Do 1016・板厚 12・挟む角 90°） ==")
F = Fork(508, 45, 45)
for (nm, o, ao) in [("周 中心・振れ幅 外径（既定）", 6, 0), ("周 外・振れ幅 外径", 0, 0), ("周 中心・振れ幅 中心", 6, 6), ("周 内・振れ幅 外径", 12, 0)]:
    p = piece(F, 1, 900, 12, o, ao)
    print(f"  {nm}: 枝の板の幅 {p['circ']:.2f}・本管の側の振れ {(900 - p['marks'][0]):.2f}・股の側の振れ {(900 - p['marks'][8]):.2f}・面積 {p['area']:.0f}")


print("== 確かめ11: AWWA C208 Table A.1 の Wye 90°（std_yfork_draft.json）と、式を in. の整数に切り上げた値 ==")
import json, os
jp = os.path.join(os.path.dirname(os.path.abspath(__file__)), "std_yfork_draft.json")
if os.path.exists(jp):
    rows = json.load(open(jp, encoding="utf-8"))["rows"]
    bad = []
    for q in rows:
        Do = q["od_in"]
        f = 0.143 * Do + 5
        Fc, Hc = math.ceil(Do / 2 * tan(rad(22.5)) + f - 1e-9), math.ceil(f / sin(rad(45)) + 0.5 * Do / tan(rad(45)) - 1e-9)
        if (Fc, Hc) != (q["F_in"], q["Hy_in"]) or round(q["F_in"] * 25.4) != q["F_mm"] or round(q["Hy_in"] * 25.4) != q["Hy_mm"]:
            bad.append((q["D_in"], Fc, Hc, q["F_in"], q["Hy_in"]))
    print(f"  {len(rows)} 行のうち、式と合わない行 {len(bad)}", bad if bad else "")
    check(not bad, "AWWA の表")
else:
    print("  std_yfork_draft.json が無いので飛ばした")


# ---------------- 例 ----------------
def example(name, Do, t, b1, b2, L, H1, H2, basis="mid", amp="outer", split=1, d=None, tb=None, sr=None, tc=None):
    print(f"== {name} ==")
    Rs = Do / 2
    o = {"outer": 0, "mid": t / 2, "inner": t}[basis]
    ao = {"outer": 0, "mid": t / 2, "inner": t}[amp]
    if d is None:
        F = Fork(Rs, b1, b2)
    else:
        ps = psi_from(Rs, d / 2, sr)
        F = Fork(Rs, b1, b2, ps, ps)
        print(f"  円すいの片側の角 ψ={ps:.4f}°・頂点まで（交点から中心線に沿って）{Rs / sin(rad(ps)):.2f}・交点での円すいの外半径 {Rs / cos(rad(ps)):.2f}")
        ob = {"outer": 0, "mid": tc / 2, "inner": tc}[basis]
        sab = (Rs - ob) / sin(rad(ps))
        print(f"  円すいの展開（板厚の基準の面）: 頂点から小さい口まで {(sab - sr) / cos(rad(ps)):.2f}・扇の角 {360 * sin(rad(ps)):.4f}°")
        yt = Rs * sin(rad(ps)) / sin(rad(b1))
        print(f"  本管と枝1 の円すいが接する点（つなぐ線がここを通り、股の点の側で本管の先が交点より前に出る）: x=0・y={yt:.2f}・z=±{sqrt(Rs * Rs - yt * yt):.2f}")
    xc, yc, zc = F.crotch()
    print(f"  股の点（3本が会う点）: x={xc:.2f}・y={yc:.2f}・z=±{zc:.2f}")
    tot = 0.0
    names = ["本管", "枝1", "枝2"]
    ends = [L, H1 if d is None else sr, H2 if d is None else sr]
    for i in range(3):
        if d is not None and i > 0:
            tt, oo, aa = tc, {"outer": 0, "mid": tc / 2, "inner": tc}[basis], {"outer": 0, "mid": tc / 2, "inner": tc}[amp]
        else:
            tt, oo, aa = t, o, ao
        p = piece(F, i, ends[i], tt, oo, aa)
        lab = names[i] if (d is None or i == 0) else names[i] + "の円すい"
        print(f"  {lab}: 板の幅（小さい口の周） {p['circ']:.2f}・外形 {p['w']:.2f}×{p['h']:.2f}・一番短い {p['short']:.2f}・一番長い {p['long']:.2f}・重さ {p['kg']:.1f} kg")
        print("    割り出し（0＝相手1 の側、4＝+z の股の点、8＝相手2 の側、12＝-z の股の点。管端・小さい口から切り口まで）:", [round(v, 2) for v in p["marks"]])
        if split > 1 and F.psi[i] == 0:
            W = p["circ"] / split
            for q in range(split):
                seg = [pt for pt in p["poly"][2:] if q * W - 1e-9 <= pt[0] <= (q + 1) * W + 1e-9]
                print(f"    縦の継ぎ目 {split} 本の {q + 1} 枚目: 幅 {W:.2f}・高さ {max(pt[1] for pt in seg):.2f}")
        tot += p["kg"]
        if d is not None and i > 0:
            Hb = [H1, H2][i - 1]
            rb = d / 2 - {"outer": 0, "mid": tb / 2, "inner": tb}[basis]
            kg = 2 * pi * rb * (Hb - sr) * tb * RHO
            print(f"  {names[i]}の管: {2 * pi * rb:.2f}×{Hb - sr:.2f}・重さ {kg:.1f} kg")
            tot += kg
    show("合計の重さ kg", tot, 1)
    lens = [(0, 1), (0, 2), (1, 2)]
    print("  継ぎ目の長さ（3D・外面）: " + "・".join(f"{names[i]}と{names[j]} {seam_len(F, i, j):.1f}" for i, j in lens))


example("例1 挟む角 90°・同じ径 1016×12（AWWA C208 Figure 1E の形）・本管 500・枝 900", 1016, 12, 45, 45, 500, 900, 900)
example("例2 挟む角 60°・同じ径 1625.6×12・本管 600・枝 2200", 1625.6, 12, 30, 30, 600, 2200, 2200)
example("例3 左右で違う角（枝1 30°・枝2 45°）・同じ径 610×9・本管 400・枝1 1000・枝2 900・縦の継ぎ目 2 本", 610, 9, 30, 45, 400, 1000, 900, split=2)
example("例4 枝が細い（円すいでつなぐ・共通の球）: 本管 1016×12・枝 610×9・挟む角 60°・円すいの小さい口まで 900・円すいの板厚 12・本管 500・枝 1500", 1016, 12, 30, 30, 500, 1500, 1500, d=610, tb=9, sr=900, tc=12)

print()
print("NG", len(NG), "件" + ("" if not NG else "：" + "・".join(NG)))
