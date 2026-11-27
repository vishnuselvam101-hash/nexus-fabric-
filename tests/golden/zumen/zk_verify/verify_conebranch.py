# 円すい（片落管・らっぱ口）に立つ枝の展開の期待値を、画面の計算（shared.js）と別に出す。python verify_conebranch.py
# 座標は shared.js の T字管・円すいと同じ: 円すいの軸を x（始点側の口が x=0、終点側の口が x=L）、天を +y、右（始点側の管端から見て）を +z。
# 方位 a は断面の円の中心から測る（天0・右90・地180・左270）。偏心の円すいは、断面（x 一定）が円で、中心が平らな側の方位 f へ cs·x だけずれる（eccStrip と同じ形）。
# 板厚は今の円すい（flarePiece・eccStrip）と同じく径の向きに取る: 外面 Ro(x)、板厚の中心 Ro(x)−T/2、内面 Ro(x)−T。
# 枝の軸は円すいの軸に直角で、向き dir（方位）、片寄せ e（方位が増える向きを正）、x0 は始点側の口から枝の中心まで。
# θ は枝の周りの角度で、0＝+x（終点側の口の向き）、90°＝方位が増える側のわき。H は円すいの軸（x 軸）から枝の管端まで、枝の軸に沿って。
import math, sys
sys.stdout.reconfigure(encoding="utf-8")
sin, cos, tan, asin, atan2, sqrt, pi, hypot = math.sin, math.cos, math.tan, math.asin, math.atan2, math.sqrt, math.pi, math.hypot
rad, deg = math.radians, math.degrees
RHO = 7.85e-6


class Body:
    def __init__(s, D1, D2, L, T, ecc=False, flat=0.0):
        s.D1, s.D2, s.L, s.T, s.ecc = D1, D2, L, T, ecc
        s.Ra, s.Rb = D1 / 2, D2 / 2
        s.k = (s.Rb - s.Ra) / L
        s.cs = (s.Ra - s.Rb) / L if ecc else 0.0
        s.f = rad(flat)

    def R(s, x, d):
        return s.Ra + s.k * x - d

    def C(s, x):
        c = s.cs * x
        return (c * cos(s.f), c * sin(s.f))

    def P(s, x, a, d):
        cy, cz = s.C(x)
        r = s.R(x, d)
        return (x, cy + r * cos(a), cz + r * sin(a))

    def a_of(s, x, y, z):
        cy, cz = s.C(x)
        return atan2(z - cz, y - cy)

    def gamma(s):
        # 同心の半頂角（偏心は、平らな側の反対で一番急な所の傾き）
        return math.atan(abs(s.k) if not s.ecc else abs(s.k) + abs(s.cs))


class Branch:
    def __init__(b, d, t, x0, dirdeg, e, H, mount="on"):
        b.ro, b.t = d / 2, t
        b.ri, b.rm = d / 2 - t, d / 2 - t / 2
        b.x0, b.dir, b.e, b.H, b.mount = x0, rad(dirdeg), e, H, mount
        b.u = (cos(b.dir), sin(b.dir))
        b.v = (-sin(b.dir), cos(b.dir))


def hit_t(B, b, rho, th, d):
    # 枝の壁の母線 (ρ, θ) が、円すいの板厚の中の面（外面から径の向きに d）に当たる所の、x 軸から枝の軸に沿った位置
    x = b.x0 + rho * cos(th)
    l = b.e + rho * sin(th)
    cy, cz = B.C(x)
    cu, cv = cy * b.u[0] + cz * b.u[1], cy * b.v[0] + cz * b.v[1]
    R = B.R(x, d)
    g = R * R - (l - cv) ** 2
    return None if g < 0 else cu + sqrt(g)


def cut_t(B, b, th, d):
    # 壁（ri..ro）のうち一番先に当たる所（閉じた式）。t(ρ) = α0+α1ρ + √(g0+g1ρ+g2ρ²)。候補は壁の両端・g=0 の点・t'=0 の点
    c, s = cos(th), sin(th)
    sf, cf = sin(B.f - b.dir), cos(B.f - b.dir)
    a0, a1 = B.cs * cf * b.x0, B.cs * cf * c
    w0, w1 = b.e - B.cs * sf * b.x0, s - B.cs * sf * c
    q0, q1 = B.R(b.x0, d), B.k * c
    g0, g1, g2 = q0 * q0 - w0 * w0, 2 * (q0 * q1 - w0 * w1), q1 * q1 - w1 * w1
    G = lambda r: g0 + g1 * r + g2 * r * r
    lo, hi = b.ri, b.ro
    cand = [lo, hi]

    def roots(A2, A1, A0):
        if abs(A2) < 1e-18:
            return [] if abs(A1) < 1e-18 else [-A0 / A1]
        D = A1 * A1 - 4 * A2 * A0
        if D < 0:
            return []
        q = sqrt(D)
        return [(-A1 - q) / (2 * A2), (-A1 + q) / (2 * A2)]

    cand += roots(g2, g1, g0)
    if abs(a1) < 1e-15:
        if abs(g2) > 1e-18:
            cand.append(-g1 / (2 * g2))
    else:
        for r in roots(4 * g2 * g2 - 4 * a1 * a1 * g2, 4 * g1 * g2 - 4 * a1 * a1 * g1, g1 * g1 - 4 * a1 * a1 * g0):
            if (g1 + 2 * g2 * r) * a1 <= 1e-12:
                cand.append(r)
    best = None
    for r in cand:
        if r < lo - 1e-12 or r > hi + 1e-12:
            continue
        r = min(max(r, lo), hi)
        gv = G(r)
        if gv < -1e-7:
            continue
        tv = a0 + a1 * r + sqrt(max(gv, 0))
        best = tv if best is None else max(best, tv)
    return best


def cut_t_brute(B, b, th, d, n=20001):
    vals = [hit_t(B, b, b.ri + (b.ro - b.ri) * i / (n - 1), th, d) for i in range(n)]
    vals = [v for v in vals if v is not None]
    return max(vals) if vals else None


def branch_dev(B, b, M=3600):
    d = 0.0 if b.mount == "on" else B.T
    W = 2 * pi * b.rm
    ln = lambda th: b.H - cut_t(B, b, th, d)
    pts = [(W * i / M, ln(pi / 2 + 2 * pi * i / M)) for i in range(M + 1)]
    area = sum((pts[i + 1][0] - pts[i][0]) * (pts[i + 1][1] + pts[i][1]) / 2 for i in range(M))
    marks = [ln(pi / 2 + 2 * pi * k / 16) for k in range(17)]
    return {"W": W, "marks": marks, "short": min(p[1] for p in pts), "long": max(p[1] for p in pts), "area": area, "kg": area * b.t * RHO}


# ---------- 円すいの板の穴（x, a の範囲） ----------
def lat_coef(B, b, a, d):
    # 断面の点の、枝の v の向きの位置 = p0 + p1·x
    sa, sf = sin(a - b.dir), sin(B.f - b.dir)
    return (B.Ra - d) * sa, B.cs * sf + B.k * sa


def iv_level(B, b, rh, a, d):
    # 外面から径の向きに d の面で、方位 a の所が枝の筒（半径 rh）の中に入る x の範囲
    p0, p1 = lat_coef(B, b, a, d)
    A2 = 1 + p1 * p1
    A1 = 2 * (-b.x0 + p1 * (p0 - b.e))
    A0 = b.x0 ** 2 + (p0 - b.e) ** 2 - rh * rh
    D = A1 * A1 - 4 * A2 * A0
    if D < 0:
        return None
    q = sqrt(D)
    return ((-A1 - q) / (2 * A2), (-A1 + q) / (2 * A2))


def iv_B(B, b, rh, a):
    # 板厚の中（d=0..T）のどこでも枝の筒が当たらない範囲（閉じた式）。(x, d) の面で楕円と帯 0≤d≤T の重なりの x の端
    los, his = [], []
    for d in (0.0, B.T):
        iv = iv_level(B, b, rh, a, d)
        if iv:
            los.append(iv[0]); his.append(iv[1])
    sa = sin(a - b.dir)
    if abs(sa) > 1e-15:
        _, p1 = lat_coef(B, b, a, 0.0)
        for xe, lst in ((b.x0 - rh, los), (b.x0 + rh, his)):
            dstar = B.Ra + (p1 * xe - b.e) / sa
            if -1e-12 <= dstar <= B.T + 1e-12:
                lst.append(xe)
    if not los:
        return None
    return (min(los), max(his))


def sq_map(B, b, a):
    # 板に直角に切る形: 展開の面（板厚の中心・径の向きに T/2）の点 M(x, a) から面の法線 n に沿って w（−T/2..T/2）の点を、
    # 枝の軸に直角な面（枝の軸の位置 x・v の向きの位置）へ写す。X = x − w·κ/N、V = V0 + p1·x + w·s/N。母線（a 一定）の上で n は一定
    s_ = sin(a - b.dir)
    kap = B.k + B.cs * cos(a - B.f)
    N = sqrt(1 + kap * kap)
    p1 = B.cs * sin(B.f - b.dir) + B.k * s_
    V0 = (B.Ra - B.T / 2) * s_
    return s_, kap, N, p1, V0


def iv_sq_w(B, b, rh, a, w):
    s_, kap, N, p1, V0 = sq_map(B, b, a)
    cx = b.x0 + w * kap / N
    q = V0 + w * s_ / N - b.e
    A2 = 1 + p1 * p1
    A1 = 2 * (-cx + p1 * q)
    A0 = cx * cx + q * q - rh * rh
    D = A1 * A1 - 4 * A2 * A0
    if D < 0:
        return None
    r = sqrt(D)
    return ((-A1 - r) / (2 * A2), (-A1 + r) / (2 * A2))


def iv_sq(B, b, rh, a):
    # 閉じた式: (x, w) の面で、楕円 |A(x,w)+c| < rh と帯 −T/2≤w≤T/2 の重なりの x の端。楕円の x の端（帯の中なら）と、帯の縁の2本の線
    s_, kap, N, p1, V0 = sq_map(B, b, a)
    los, his = [], []
    for w in (-B.T / 2, B.T / 2):
        iv = iv_sq_w(B, b, rh, a, w)
        if iv:
            los.append(iv[0]); his.append(iv[1])
    det = (s_ + p1 * kap) / N
    if abs(det) > 1e-14:
        r1 = (s_ / N / det, kap / N / det)
        r2 = (-p1 / det, 1 / det)
        q0 = (b.x0, b.e - V0)
        n1 = hypot(*r1)
        for sg, lst in ((-1, los), (1, his)):
            qx, qy = q0[0] + sg * rh * r1[0] / n1, q0[1] + sg * rh * r1[1] / n1
            x = r1[0] * qx + r1[1] * qy
            w = r2[0] * qx + r2[1] * qy
            if -B.T / 2 - 1e-12 <= w <= B.T / 2 + 1e-12:
                lst.append(x)
    if not los:
        return None
    return (min(los), max(his))


def iv_sq_levels(B, b, rh, a, n):
    los, his = [], []
    for j in range(n):
        iv = iv_sq_w(B, b, rh, a, -B.T / 2 + B.T * j / (n - 1))
        if iv:
            los.append(iv[0]); his.append(iv[1])
    return (min(los), max(his)) if los else None


def iv_levels(B, b, rh, a, n):
    los, his = [], []
    for j in range(n):
        iv = iv_level(B, b, rh, a, B.T * j / (n - 1) if n > 1 else B.T)
        if iv:
            los.append(iv[0]); his.append(iv[1])
    return (min(los), max(his)) if los else None


def hole(B, b, method="S", N=720, levels=None):
    # 穴の縁: 方位 a を両端に寄せて刻み、各 a で x の範囲。
    # S（推す）: 板に直角に切った穴で、板厚の中のどこでも枝の筒が当たらない形。R: 壁を径の向き（x 一定）にした形。A: 内面（径の向きに T）の線だけ（今の T字管の決まり）
    rh = b.ri if b.mount == "on" else b.ro
    if method == "A":
        f = lambda a: iv_level(B, b, rh, a, B.T)
    elif method == "R":
        f = (lambda a: iv_levels(B, b, rh, a, levels)) if levels else (lambda a: iv_B(B, b, rh, a))
    else:
        f = (lambda a: iv_sq_levels(B, b, rh, a, levels)) if levels else (lambda a: iv_sq(B, b, rh, a))
    th = hit_t(B, b, 0.0, 0.0, B.T)
    cy, cz = B.C(b.x0)
    y, z = th * b.u[0] + b.e * b.v[0], th * b.u[1] + b.e * b.v[1]
    ac = atan2(z - cz, y - cy)

    def edge(sg):
        a_in, step = ac, 1e-3
        while f(a_in + sg * step):
            a_in += sg * step
        a_out = a_in + sg * step
        for _ in range(80):
            m = (a_in + a_out) / 2
            if f(m):
                a_in = m
            else:
                a_out = m
        return a_in

    alo, ahi = edge(-1), edge(1)
    am, ah = (alo + ahi) / 2, (ahi - alo) / 2
    lo, hi, grid = [], [], []
    for i in range(N + 1):
        a = am - ah * cos(pi * i / N)
        iv = f(a)
        if not iv:
            continue
        grid.append(a); lo.append((iv[0], a)); hi.append((iv[1], a))
    return {"alo": alo, "ahi": ahi, "lo": lo, "hi": hi, "grid": grid, "f": f, "am": am, "ah": ah, "rh": rh}


def weld_line(B, b, M=720):
    # 枝の外面が円すいの外面に当たる線（x, a）
    out = []
    for i in range(M):
        th = 2 * pi * i / M
        t = hit_t(B, b, b.ro, th, 0.0)
        x = b.x0 + b.ro * cos(th)
        l = b.e + b.ro * sin(th)
        y, z = t * b.u[0] + l * b.v[0], t * b.u[1] + l * b.v[1]
        out.append((x, B.a_of(x, y, z)))
    return out


# ---------- 展開（扇形）: 板厚の中心の面を頂点のまわりに開く ----------
class Fan:
    def __init__(s, B, N=200000):
        s.B = B
        d = B.T / 2
        s.r0 = B.R(0, d)
        xA = -s.r0 / B.k
        s.A = (xA,) + B.C(xA)
        if not B.ecc:
            s.sg = abs(B.k) / sqrt(1 + B.k * B.k)
            s.l0 = s.r0 / s.sg
            s.tab = None
            return
        s.N = N
        s.tab = [0.0]
        prev = s.U(B.f)
        for i in range(1, N + 1):
            cur = s.U(B.f + 2 * pi * i / N)
            ch = hypot(cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2])
            s.tab.append(s.tab[-1] + 2 * asin(min(1, ch / 2)))
            prev = cur
        s.total = s.tab[-1]

    def ell(s, a):
        if s.tab is None:
            return s.l0
        q = s.B.P(0, a, s.B.T / 2)
        return hypot(q[0] - s.A[0], q[1] - s.A[1], q[2] - s.A[2])

    def U(s, a):
        q = s.B.P(0, a, s.B.T / 2)
        v = (q[0] - s.A[0], q[1] - s.A[1], q[2] - s.A[2])
        n = hypot(*v)
        return (v[0] / n, v[1] / n, v[2] / n)

    def psi(s, a):
        if s.tab is None:
            return a * s.sg
        t = (a - s.B.f) / (2 * pi)
        k = math.floor(t)
        p = (t - k) * s.N
        i = min(int(p), s.N - 1)
        fr = p - i
        return k * s.total + s.tab[i] + (s.tab[i + 1] - s.tab[i]) * fr

    def lam(s, x):
        return s.B.R(x, s.B.T / 2) / s.r0

    def pt(s, x, a):
        rr, ps = s.lam(x) * s.ell(a), s.psi(a)
        return (rr * sin(ps), rr * cos(ps))


def shoelace(p):
    n = len(p)
    return abs(sum(p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1] for i in range(n))) / 2


def plen(p, closed=False):
    q = p + ([p[0]] if closed else [])
    return sum(hypot(q[i + 1][0] - q[i][0], q[i + 1][1] - q[i][1]) for i in range(len(q) - 1))


def ring_xa(H):
    return [(x, a) for x, a in H["lo"]] + [(x, a) for x, a in reversed(H["hi"])]


def surf_area_strip(B, a0, a1, xs, n=4000):
    # 板厚の中心の面の面積 ∫∫ r_m(x)·√(1+(k+cs·cos(a−f))²) dx da（xs(a) は x の範囲を返す。a は両端に寄せて刻む）
    am, ah = (a0 + a1) / 2, (a1 - a0) / 2
    tot = 0.0
    for i in range(n + 1):
        u = pi * i / n
        a = am - ah * cos(u)
        iv = xs(a)
        if not iv:
            continue
        x0, x1 = iv
        rint = (B.Ra - B.T / 2) * (x1 - x0) + B.k * (x1 * x1 - x0 * x0) / 2
        val = rint * sqrt(1 + (B.k + B.cs * cos(a - B.f)) ** 2) * ah * sin(u)
        tot += val * (0.5 if i in (0, n) else 1)
    return tot * pi / n


# ---------- 継ぎ目 ----------
def seg_dist(F, P, a_s):
    ps = F.psi(a_s)
    ux, uy = sin(ps), cos(ps)
    l0, l1 = F.lam(0) * F.ell(a_s), F.lam(F.B.L) * F.ell(a_s)
    lo, hi = min(l0, l1), max(l0, l1)
    pr = min(max(P[0] * ux + P[1] * uy, lo), hi)
    return hypot(P[0] - pr * ux, P[1] - pr * uy)


def wrap(a, c):
    return a - 2 * pi * math.floor((a - c + pi) / (2 * pi))


def seams(B, F, b, H, n, rule0, gap):
    # 継ぎ目 n 本（方位 rule0 から等間隔）。穴にかかるか、穴・溶接の線から gap より近ければ、間隔のまま回して穴を1枚の真ん中に置く
    hole_pts = [F.pt(x, a) for x, a in ring_xa(H)]
    wl = weld_line(B, b)
    wl_pts = [F.pt(x, a) for x, a in wl]
    a_lo = min(H["alo"], min(wrap(a, H["am"]) for _, a in wl))
    a_hi = max(H["ahi"], max(wrap(a, H["am"]) for _, a in wl))

    def check(ss):
        res = []
        for a_s in ss:
            w = wrap(a_s, H["am"])
            cross = H["alo"] < w < H["ahi"]
            dh = 0.0 if cross else min(seg_dist(F, P, a_s) for P in hole_pts)
            dw = min(seg_dist(F, P, a_s) for P in wl_pts)
            res.append((a_s, cross, dh, dw))
        return res

    step = 2 * pi / n
    rule = [rule0 + step * j for j in range(n)]
    r0 = check(rule)
    hit = any(c or dh < gap or dw < gap for _, c, dh, dw in r0)
    moved, ss, r1 = 0.0, rule, r0
    kc = (a_lo + a_hi) / 2
    if hit:
        ss = [kc - pi / n + step * j for j in range(n)]
        r1 = check(ss)
        moved = deg((ss[0] - rule0 + step / 2) % step - step / 2)
    cross = any(c or dh < gap or dw < gap for _, c, dh, dw in r1)
    return {"rule": r0, "final": r1, "hit": hit, "moved": moved, "cross": cross, "kc": kc, "a_lo": a_lo, "a_hi": a_hi, "span": deg(a_hi - a_lo)}


# ---------- 出力 ----------
def fmt(v, n=2):
    return f"{v:.{n}f}"


def piece_frame_conc(F, ac):
    # 同心: 片の真ん中の方位 ac を +Y に向ける（flarePiece と同じ向き。頂点が原点）
    pc = F.psi(ac)
    c, s = cos(pc), sin(pc)
    # 扇の点 (S·sinψ, S·cosψ) を ψ−pc の所へ回す
    return lambda P: (P[0] * c - P[1] * s, P[0] * s + P[1] * c)


def piece_frame_ecc(F, a0):
    # 偏心: eccStrip と同じ向き。始点側の口（x=0）の継ぎ目の点が原点、継ぎ目の母線が +X（終点側の口へ）、方位が増える向きが −Y
    O = F.pt(0, a0)
    E = F.pt(F.B.L, a0)
    ang = atan2(E[1] - O[1], E[0] - O[0])
    c, s = cos(-ang), sin(-ang)
    def raw(P):
        X, Y = P[0] - O[0], P[1] - O[1]
        return (X * c - Y * s, X * s + Y * c)
    sgn = -1 if raw(F.pt(F.B.L / 2, a0 + 1e-3))[1] > 0 else 1
    return lambda P: (raw(P)[0], sgn * raw(P)[1])


def run(name, B, b, n, rule0deg, mount_note):
    print(f"\n=== {name} ===")
    T = B.T
    g = deg(math.atan(abs(B.k))) if not B.ecc else None
    print(f"円すい: 始点側の口 {B.D1}・終点側の口 {B.D2}・長さ {B.L}・板厚 {T}・{'偏心（平らな側 ' + fmt(deg(B.f), 0) + '°）' if B.ecc else '同心（半頂角 ' + fmt(g, 3) + '°）'}")
    print(f"枝: 外径 {2 * b.ro}・板厚 {b.t}・x0 {b.x0}・向き {fmt(deg(b.dir), 0)}°・片寄せ {b.e}・H {b.H}・{mount_note}")
    bd = branch_dev(B, b)
    print(f"枝の板の幅 2π·rm = {fmt(bd['W'])}")
    print("枝の割り出し k=0..16（0 わき・4 始点側・8 反対のわき・12 終点側）: " + ", ".join(fmt(v) for v in bd["marks"]))
    print(f"枝 一番短い {fmt(bd['short'])}・一番長い {fmt(bd['long'])}・重さ {fmt(bd['kg'], 1)} kg")
    F = Fan(B)
    HB = hole(B, b, "S")
    HA = hole(B, b, "A")
    HR = hole(B, b, "R")
    H9 = hole(B, b, "S", levels=9)
    H401 = hole(B, b, "S", levels=401)

    def dif(H1, H2):
        m = 0.0
        for a in H1["grid"]:
            p, q = H1["f"](a), H2["f"](a)
            if p and q:
                m = max(m, abs(p[0] - q[0]), abs(p[1] - q[1]))
        return m

    print(f"穴 S（板に直角に切る）: 閉じた式と板厚を401段に刻んだのとの差 {dif(HB, H401):.4f}・9段との差 {dif(HB, H9):.4f}")
    xs = [x for x, _ in HB["lo"]] + [x for x, _ in HB["hi"]]
    xr = [x for x, _ in HR["lo"]] + [x for x, _ in HR["hi"]]
    xa = [x for x, _ in HA["lo"]] + [x for x, _ in HA["hi"]]

    def fan_gap(H1, H2):
        # 扇の上で、H1 の縁の点から H2 の縁の線までの一番遠い所（H1 が H2 よりどれだけ外に出るか）
        p1 = [F.pt(x, a) for x, a in ring_xa(H1)][::3]
        p2 = [F.pt(x, a) for x, a in ring_xa(H2)]
        m = 0.0
        for P in p1:
            best = 1e18
            for i in range(len(p2)):
                A_, B_ = p2[i], p2[(i + 1) % len(p2)]
                dx, dy = B_[0] - A_[0], B_[1] - A_[1]
                L2 = dx * dx + dy * dy
                t = 0 if L2 == 0 else min(1, max(0, ((P[0] - A_[0]) * dx + (P[1] - A_[1]) * dy) / L2))
                best = min(best, hypot(P[0] - A_[0] - t * dx, P[1] - A_[1] - t * dy))
            m = max(m, best)
        return m

    print(f"穴 S が外に出る大きさ（扇の上で縁から縁まで・一番大きい所）: A（内面だけ）より {fan_gap(HB, HA):.2f}・R（壁を径の向き）より {fan_gap(HB, HR):.2f}")
    print(f"穴の方位の範囲 S: {fmt(deg(HB['alo']))}°〜{fmt(deg(HB['ahi']))}°（A: {fmt(deg(HA['alo']))}°〜{fmt(deg(HA['ahi']))}°）")
    print(f"穴の x の範囲（始点側の口から・展開の面で）S: {fmt(min(xs))}〜{fmt(max(xs))}（長さ {fmt(max(xs) - min(xs))}）・R: {fmt(min(xr))}〜{fmt(max(xr))}・A: {fmt(min(xa))}〜{fmt(max(xa))}")
    ring = [F.pt(x, a) for x, a in ring_xa(HB)]
    area_fan = shoelace(ring)
    area_srf = surf_area_strip(B, HB["alo"], HB["ahi"], HB["f"], n=4000)
    print(f"穴 S の面積: 扇の上 {fmt(area_fan, 1)}・円すいの面の上の積分 {fmt(area_srf, 1)}（差 {fmt(abs(area_fan - area_srf) / area_srf * 100, 4)}%）")
    # 継ぎ目
    gap = 5 * T
    sm = seams(B, F, b, HB, n, rad(rule0deg), gap)
    print(f"{n} 枚割り: 決まりの継ぎ目 " + "・".join(fmt(deg(a) % 360, 1) + "°" for a, *_ in sm["rule"]) + f" → {'穴または空き ' + fmt(gap, 0) + ' にかかる' if sm['hit'] else 'かからない'}")
    print(f"  避ける所（穴と溶接の線）の方位の幅 {fmt(sm['span'])}°（1枚の方位 {fmt(360 / n, 1)}°）")
    if sm["hit"]:
        print("  回した継ぎ目 " + "・".join(fmt(deg(a) % 360, 2) + "°" for a, *_ in sm["final"]) + f"（決まりから {fmt(sm['moved'])}°）")
    for a, c, dh, dw in sm["final"]:
        print(f"  継ぎ目 {fmt(deg(a) % 360, 2)}°: 穴の縁まで {('かかる' if c else fmt(dh))}・溶接の線まで {fmt(dw)}")
    print(f"  それでもかかるか: {sm['cross']}")
    # 穴のある片
    a_first = [a for a, *_ in sm["final"]]
    a0 = wrap(min(a_first, key=lambda s: (sm["kc"] - wrap(s, sm["kc"])) % (2 * pi)), sm["kc"])
    if wrap(a0, sm["kc"]) > sm["kc"]:
        a0 -= 2 * pi
    a1 = a0 + 2 * pi / n
    if not B.ecc:
        tr = piece_frame_conc(F, (a0 + a1) / 2)
        Sx0, SxL = F.lam(0) * F.l0, F.lam(B.L) * F.l0
        Rl, Rs = max(Sx0, SxL), min(Sx0, SxL)
        half = 2 * pi * F.sg / n
        print(f"扇: 外の半径 {fmt(Rl)}・内の半径 {fmt(Rs)}・全体の角 {fmt(deg(2 * pi * F.sg), 4)}°・1枚の角 {fmt(deg(half), 4)}°・弦 {fmt(2 * Rl * sin(half / 2))}・高さ {fmt(Rl - Rs * cos(half / 2))}")
        piece_area = (Rl * Rl - Rs * Rs) * half / 2
    else:
        tr = piece_frame_ecc(F, a0)
        rim0 = [tr(F.pt(0, a0 + (a1 - a0) * i / 720)) for i in range(721)]
        rimL = [tr(F.pt(B.L, a0 + (a1 - a0) * i / 720)) for i in range(721)]
        poly = rim0 + rimL[::-1]
        xs_ = [p[0] for p in poly]; ys_ = [p[1] for p in poly]
        print(f"穴のある片（方位 {fmt(deg(a0) % 360, 2)}°〜{fmt(deg(a1) % 360, 2)}°）: 外形 {fmt(max(xs_) - min(xs_))}×{fmt(max(ys_) - min(ys_))}・角の点 始点側の口の端 (0,0)・{fmt(rim0[-1][0])},{fmt(rim0[-1][1])}・終点側の口の端 {fmt(rimL[0][0])},{fmt(rimL[0][1])}・{fmt(rimL[-1][0])},{fmt(rimL[-1][1])}")
        piece_area = shoelace(poly)
        srf = surf_area_strip(B, a0, a1, lambda a: (0.0, B.L), n=4000)
        print(f"  片の面積 扇 {fmt(piece_area, 1)}・面の積分 {fmt(srf, 1)}")
        for j in range(1, n):
            b0, b1 = a0 + 2 * pi * j / n, a0 + 2 * pi * (j + 1) / n
            tj = piece_frame_ecc(F, b0)
            pj = [tj(F.pt(0, b0 + (b1 - b0) * i / 720)) for i in range(721)] + [tj(F.pt(B.L, b1 - (b1 - b0) * i / 720)) for i in range(721)]
            print(f"  ほかの片（方位 {fmt(deg(b0) % 360, 2)}°〜{fmt(deg(b1) % 360, 2)}°）: 外形 {fmt(max(p[0] for p in pj) - min(p[0] for p in pj))}×{fmt(max(p[1] for p in pj) - min(p[1] for p in pj))}・重さ {fmt(shoelace(pj) * T * RHO, 1)} kg")
    hp = [tr(P) for P in ring]
    hx = [p[0] for p in hp]; hy = [p[1] for p in hp]
    print(f"穴 S の扇の上の外形（片の座標）: X {fmt(min(hx))}〜{fmt(max(hx))}（幅 {fmt(max(hx) - min(hx))}）・Y {fmt(min(hy))}〜{fmt(max(hy))}（高さ {fmt(max(hy) - min(hy))}）")
    # 割り出し（枝の円周を16等分した点が円すいの内面を通る方位で、穴 S の縁。0＝+x の側）
    mk = []
    for k in range(16):
        th = 2 * pi * k / 16
        t = hit_t(B, b, HB["rh"], th, T)
        x = b.x0 + HB["rh"] * cos(th)
        l = b.e + HB["rh"] * sin(th)
        y, z = t * b.u[0] + l * b.v[0], t * b.u[1] + l * b.v[1]
        a = min(max(wrap(B.a_of(x, y, z), HB["am"]), HB["alo"]), HB["ahi"])
        iv = HB["f"](a)
        cq = cos(th)
        xx = iv[1] if cq > 1e-9 else iv[0] if cq < -1e-9 else (iv[0] + iv[1]) / 2
        P = tr(F.pt(xx, a))
        mk.append(f"({fmt(P[0], 1)}, {fmt(P[1], 1)})")
    print("穴 S の割り出し k=0..15（片の座標）: " + " ".join(mk))
    print(f"穴のある片の重さ {fmt((piece_area - area_fan) * T * RHO, 1)} kg（穴 {fmt(area_fan * T * RHO, 1)} kg を引く）")
    full = surf_area_strip(B, 0.0, 2 * pi, lambda a: (0.0, B.L), n=4000)
    if not B.ecc:
        r1m, r2m = B.R(0, T / 2), B.R(B.L, T / 2)
        full_c = pi * (r1m + r2m) * hypot(B.L, r2m - r1m)
        print(f"円すい全体の重さ（穴を引く） {fmt((full - area_fan) * T * RHO, 1)} kg（面積 {fmt(full, 0)}・π(r1+r2)·母線 {fmt(full_c, 0)}）")
    else:
        print(f"円すい全体の重さ（穴を引く） {fmt((full - area_fan) * T * RHO, 1)} kg（面積 {fmt(full, 0)}）")
    return {"B": B, "b": b, "F": F, "HB": HB, "bd": bd, "tr": tr, "piece_area": piece_area, "area_fan": area_fan}


# ---------- 法線の向きに板厚を取ったときの差（同心だけ） ----------
def normal_model(B, b):
    # 板厚 T を面に直角に取る: 外面 Ro(x)、深さ ν の面 Ro(x)−ν/cosγ。扇は深さ T/2 の面。穴 B は面に直角に切った線分が枝の筒に当たらない形
    k = B.k
    cg, sgm = 1 / sqrt(1 + k * k), k / sqrt(1 + k * k)
    T = B.T
    m0 = B.Ra - T / (2 * cg)
    rh = b.ri if b.mount == "on" else b.ro
    sg = abs(sgm)

    def iv(a, w):
        # 板厚の中心の面の点 (x_m, a) から外へ w（−T/2..T/2）の点が、枝の筒の中に入る x_m の範囲（dir=0 の向きに直して）
        s_ = sin(a - b.dir)
        # x = x_m − w·sinγ、r = m0 + k·x_m + w·cosγ、横の位置 = r·s_
        A2 = 1 + (k * s_) ** 2
        q = (m0 + w * cg) * s_ - b.e
        cx = b.x0 + w * sgm
        A1 = 2 * (-cx + k * s_ * q)
        A0 = cx * cx + q * q - rh * rh
        D = A1 * A1 - 4 * A2 * A0
        if D < 0:
            return None
        r = sqrt(D)
        return ((-A1 - r) / (2 * A2), (-A1 + r) / (2 * A2))

    def ivB(a, n=401):
        los, his = [], []
        for j in range(n):
            v = iv(a, -T / 2 + T * j / (n - 1))
            if v:
                los.append(v[0]); his.append(v[1])
        return (min(los), max(his)) if los else None

    def pt(xm, a):
        S = (m0 + k * xm) / sg
        return (S * sin(a * sg), S * cos(a * sg))

    return {"ivB": ivB, "pt": pt, "S0": m0 / sg, "SL": (m0 + k * B.L) / sg, "ang": 2 * pi * sg}


def compare_normal(name, res, n):
    # 展開の面を「口の外径から面に直角に T/2」（本当の板厚の中心）にしたときと、今の取り方（径の向きに T/2）との差
    B, b, F, HB = res["B"], res["b"], res["F"], res["HB"]
    NM = normal_model(B, b)
    cg = 1 / sqrt(1 + B.k * B.k)
    print(f"\n--- {name}: 展開の面を本当の板厚の中心（口の外径から面に直角に T/2）にしたときとの差 ---")
    Rl_r, Rs_r = max(F.lam(0) * F.l0, F.lam(B.L) * F.l0), min(F.lam(0) * F.l0, F.lam(B.L) * F.l0)
    Rl_n, Rs_n = max(NM["S0"], NM["SL"]), min(NM["S0"], NM["SL"])
    print(f"扇の外の半径 {fmt(Rl_r)} → {fmt(Rl_n)}（差 {fmt(Rl_n - Rl_r, 3)}）・内の半径 {fmt(Rs_r)} → {fmt(Rs_n)}（差 {fmt(Rs_n - Rs_r, 3)}）。口の所の板厚の中心の半径の差 T/2·(1/cosγ−1) = {fmt(B.T / 2 * (1 / cg - 1), 3)}")
    print(f"大きい口の周の長さ（板厚の中心） {fmt(2 * pi * F.sg * Rl_r)} → {fmt(NM['ang'] * Rl_n)}（差 {fmt(NM['ang'] * Rl_n - 2 * pi * F.sg * Rl_r, 3)}）")
    S0r, S0n = F.lam(0) * F.l0, NM["S0"]

    def box(fpt, ivf, S0, alo, ahi, N=721):
        pts = []
        for i in range(N + 1):
            a = (alo + ahi) / 2 - (ahi - alo) / 2 * cos(pi * i / N)
            v = ivf(a)
            if v:
                pts += [fpt(v[0], a), fpt(v[1], a)]
        r = [abs(hypot(*p) - S0) for p in pts]
        return min(r), max(r), max(p[0] for p in pts) - min(p[0] for p in pts)

    a_lo, a_hi = HB["alo"] - 0.02, HB["ahi"] + 0.02
    r_lo, r_hi, wr = box(F.pt, lambda a: iv_sq(B, b, HB["rh"], a), S0r, a_lo, a_hi)
    n_lo, n_hi, wn = box(NM["pt"], NM["ivB"], S0n, a_lo, a_hi)
    print(f"穴 S の始点側の口からの母線に沿った距離 {fmt(r_lo)}〜{fmt(r_hi)} → {fmt(n_lo)}〜{fmt(n_hi)}（差 {fmt(n_lo - r_lo, 3)}・{fmt(n_hi - r_hi, 3)}）・扇の上の幅 {fmt(wr)} → {fmt(wn)}（差 {fmt(wn - wr, 3)}）")
    # 枝の切り口が当たる面: 外面（読んだ外径）・内面（径の向きに T）と、本当の内面（面に直角に T）・板（展開の面±T/2 を面に直角に）の面
    d_true = B.T / cg
    d_slab_in, d_slab_out = B.T / 2 + B.T / (2 * cg), B.T / 2 - B.T / (2 * cg)
    d = 0.0 if b.mount == "on" else B.T
    alt = [d_slab_out] if b.mount == "on" else [d_true, d_slab_in]
    for dd, lab in zip(alt, ["板の外面"] if b.mount == "on" else ["本当の内面", "板の内面"]):
        mx = max(abs(cut_t(B, b, 2 * pi * i / 64, dd) - cut_t(B, b, 2 * pi * i / 64, d)) for i in range(64))
        print(f"枝の切り口: 当たる面を{lab}にしたときとの差 最大 {fmt(mx, 3)}")


# ================= 確かめ =================
print("=== 確かめ1. 半頂角→0 で今の T字管と一致（verify_tee.py 例1: 本管 1016×12・枝 610×9・H 900・本管の長さ 1500） ===")
for lab, D2 in (("半頂角 1e-7 rad 相当", 1016.0 + 2 * 1500 * 1e-7), ("半頂角 0.001°", 1016.0 + 2 * 1500 * tan(rad(0.001)))):
    Bt = Body(1016.0, D2, 1500.0, 12.0)
    bt = Branch(610.0, 9.0, 750.0, 0, 0.0, 900.0, "on")
    bd = branch_dev(Bt, bt)
    m = bd["marks"]
    print(f"{lab}: 枝の板の幅 {fmt(bd['W'])}・わき {fmt(m[0])}・22.5° {fmt(m[1])}・45° {fmt(m[2])}・67.5° {fmt(m[3])}・背 {fmt(m[4])}（T字管 1888.10・487.15・471.89・437.12・404.79・392.00）")
    Ft = Fan(Bt)
    Ht = hole(Bt, bt, "S")
    tr = piece_frame_conc(Ft, 0.0)
    hp = [tr(Ft.pt(x, a)) for x, a in ring_xa(Ht)]
    print(f"  穴 周方向 {fmt(max(p[0] for p in hp) - min(p[0] for p in hp))}・軸方向 {fmt(max(p[1] for p in hp) - min(p[1] for p in hp))}（T字管 642.03・592.00）・重さ 枝 {fmt(bd['kg'], 2)}（T字管 58.47）")
    bt2 = Branch(610.0, 9.0, 750.0, 0, 150.0, 900.0, "on")
    print("  片寄せ150 の当たる高さ θ=−90°,0°,90°: " + ", ".join(fmt(cut_t(Bt, bt2, rad(q), 0.0)) for q in (-90, 0, 90)) + "（T字管 486.57・485.35・243.20）")
    bt3 = Branch(610.0, 9.0, 750.0, 0, 0.0, 900.0, "in")
    bd3 = branch_dev(Bt, bt3)
    Ht3 = hole(Bt, bt3, "S")
    hp3 = [tr(Ft.pt(x, a)) for x, a in ring_xa(Ht3)]
    print(f"  差し込む: わき {fmt(bd3['marks'][0])}・背 {fmt(bd3['marks'][4])}・穴 周方向 {fmt(max(p[0] for p in hp3) - min(p[0] for p in hp3))}（T字管 502.01・404.00・664.93）")


def teeY(Rc, ri, ro, e, th):
    a, b2 = e + ri * sin(th), e + ro * sin(th)
    m = 0.0 if a * b2 <= 0 else min(abs(a), abs(b2))
    return sqrt(Rc * Rc - m * m)


Bt = Body(1016.0, 1016.0 + 2 * 1500 * 1e-9, 1500.0, 12.0)
mx = 0.0
for e in (0.0, 150.0, -80.0):
    for mount in ("on", "in"):
        bt = Branch(610.0, 9.0, 750.0, 37.0, e, 900.0, mount)
        Rc = 508.0 if mount == "on" else 496.0
        for i in range(64):
            th = 2 * pi * i / 64
            mx = max(mx, abs(cut_t(Bt, bt, th, 0.0 if mount == "on" else 12.0) - teeY(Rc, bt.ri, bt.ro, e, th)))
print(f"T字管の teeY との差（片寄せ 0・150・−80、載せる・差し込む、64方向、枝の向き 37°）: 最大 {mx:.2e}")

print("\n=== 確かめ2. 壁の中で一番先に当たる所: 閉じた式と、壁を2万に刻んだ総当たり ===")
cfgs = [
    (Body(1016.0, 812.8, 812.8, 12.0), Branch(406.4, 9.0, 406.4, 0, 0.0, 800.0)),
    (Body(914.4, 1219.2, 600.0, 12.0), Branch(508.0, 9.0, 300.0, 90, 100.0, 900.0, "in")),
    (Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=0), Branch(406.4, 9.0, 600.0, 0, 0.0, 900.0)),
    (Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=0), Branch(406.4, 9.0, 600.0, 90, 0.0, 900.0)),
    (Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=180), Branch(406.4, 9.0, 500.0, 60, 120.0, 900.0, "in")),
    (Body(2000.0, 1200.0, 1500.0, 0.0), Branch(600.0, 0.0001, 750.0, 0, 0.0, 1500.0)),
]
mx = 0.0
for Bc, bc in cfgs:
    d = 0.0 if bc.mount == "on" else Bc.T
    for i in range(64):
        th = 2 * pi * i / 64
        mx = max(mx, abs(cut_t(Bc, bc, th, d) - cut_t_brute(Bc, bc, th, d)))
print(f"6つの寸法×64方向: 最大の差 {mx:.6f} mm")

print("\n=== 確かめ3. 板厚0の長さ（3D の交わりの線・枝の切り口の線・円すいの穴の周）が3つとも同じ ===")


def lens0(B, r, x0, dirdeg, e, N=200000):
    b = Branch(2 * r, 0.0, x0, dirdeg, e, 0.0)
    b.ri = b.ro = b.rm = r
    P3, Pb, Ph = [], [], []
    F = Fan(B)
    for i in range(N + 1):
        th = 2 * pi * i / N
        t = hit_t(B, b, r, th, 0.0)
        x = x0 + r * cos(th)
        l = e + r * sin(th)
        y, z = t * b.u[0] + l * b.v[0], t * b.u[1] + l * b.v[1]
        P3.append((x, y, z))
        Pb.append((r * th, -t))
        Ph.append(F.pt(x, B.a_of(x, y, z)))
    l3 = sum(hypot(P3[i + 1][0] - P3[i][0], P3[i + 1][1] - P3[i][1], P3[i + 1][2] - P3[i][2]) for i in range(N))
    return l3, plen(Pb), plen(Ph)


for lab, B0, r, x0, dd, e in (
    ("同心 Ra=1000・Rb=600・長さ1500・x0=750・r=300（前の調べ 1936.190）", Body(2000.0, 1200.0, 1500.0, 0.0), 300.0, 750.0, 0, 0.0),
    ("同心 同じ寸法・片寄せ 250・向き 45°", Body(2000.0, 1200.0, 1500.0, 0.0), 300.0, 750.0, 45, 250.0),
    ("偏心 1219.2→914.4・長さ1200・平らな側 天・枝 r=203.2 を天に", Body(1219.2, 914.4, 1200.0, 0.0, ecc=True, flat=0), 203.2, 600.0, 0, 0.0),
    ("偏心 同じ・枝を右（90°）に・片寄せ 100", Body(1219.2, 914.4, 1200.0, 0.0, ecc=True, flat=0), 203.2, 600.0, 90, 100.0),
):
    l3, lb, lh = lens0(B0, r, x0, dd, e)
    print(f"{lab}: 3D {l3:.5f}・枝 {lb:.5f}・穴 {lh:.5f}")

print("\n=== 確かめ4. 扇の大きさ: 1枚の外の弧 = π·Dm/n、偏心は口の周の長さ ===")
B4 = Body(1016.0, 812.8, 812.8, 12.0)
F4 = Fan(B4)
for n in (2, 3, 4):
    S0, SL = F4.lam(0) * F4.l0, F4.lam(B4.L) * F4.l0
    ang = 2 * pi * F4.sg / n
    print(f"同心 1016→812.8 n={n}: 1枚の角 {deg(ang):.4f}°・大きい口の弧 {S0 * ang:.3f}（π·(1016−12)/n={pi * 1004 / n:.3f}）・小さい口の弧 {SL * ang:.3f}（π·(812.8−12)/n={pi * 800.8 / n:.3f}）")
B5 = Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=0)
F5 = Fan(B5)
rim = [F5.pt(0, 2 * pi * i / 20000) for i in range(20001)]
rimL = [F5.pt(B5.L, 2 * pi * i / 20000) for i in range(20001)]
print(f"偏心 1219.2→914.4: 扇の全体の角 {deg(F5.total):.4f}°・大きい口の弧 {plen(rim):.3f}（π·1207.2={pi * 1207.2:.3f}）・小さい口の弧 {plen(rimL):.3f}（π·902.4={pi * 902.4:.3f}）")
B6 = Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=0)
print(f"偏心の二つ割（平らな側から）各片の扇の角: {deg(F5.psi(pi)):.4f}°・{deg(F5.psi(2 * pi) - F5.psi(pi)):.4f}°")
print(f"偏心の継ぎ目を 90°・270° に回した二つ割: 平らな側の片 {deg(F5.psi(pi / 2) - F5.psi(-pi / 2)):.4f}°・反対の片 {deg(F5.psi(3 * pi / 2) - F5.psi(pi / 2)):.4f}°")

# ================= 例 =================
R1 = run("例1 同心（AWWA の最小の長さ・半頂角 7.125°）・天に枝・載せる・二つ割", Body(1016.0, 812.8, 812.8, 12.0), Branch(406.4, 9.0, 406.4, 0, 0.0, 800.0, "on"), 2, 0.0, "載せる")
R2 = run("例2 同心（急な円すい・半頂角 14.25°）・右に枝・片寄せ 100（地の側）・差し込む・三つ割", Body(914.4, 1219.2, 600.0, 12.0), Branch(508.0, 9.0, 300.0, 90, 100.0, 900.0, "in"), 3, 0.0, "差し込む")
R3 = run("例3 偏心（平らな側 天）・平らな側（天）に枝・載せる・二つ割", Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=0), Branch(406.4, 9.0, 600.0, 0, 0.0, 900.0, "on"), 2, 0.0, "載せる")
R4 = run("例4 偏心（平らな側 天）・右（90°）に枝・載せる・二つ割", Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=0), Branch(406.4, 9.0, 600.0, 90, 0.0, 900.0, "on"), 2, 0.0, "載せる")

compare_normal("例1", R1, 2)
compare_normal("例2", R2, 3)

print("\n=== 確かめ5. 半頂角を小さくしていくと T字管に近づく（例1 の枝 406.4×9・H 800、口 1016 を固定して長さを伸ばす） ===")
for L in (812.8, 2000.0, 10000.0, 1e5, 1e7):
    Bl = Body(1016.0, 812.8, L, 12.0)
    bl = Branch(406.4, 9.0, L / 2, 0, 0.0, 800.0)
    m = branch_dev(Bl, bl)["marks"]
    Rmid = Bl.R(L / 2, 0.0)
    tee = [800.0 - teeY(Rmid, bl.ri, bl.ro, 0.0, pi / 2 + 2 * pi * k / 16) for k in range(17)]
    print(f"長さ {L:g}（半頂角 {deg(math.atan(abs(Bl.k))):.5f}°）: 枝の割り出しと、枝の中心の所の径の T字管との差 最大 {max(abs(m[k] - tee[k]) for k in range(17)):.4f}")

print("\n=== 確かめ6. 偏心の向きを回しても同じ（平らな側 地・枝 地 ＝ 平らな側 天・枝 天、平らな側 右・枝 地 ＝ 平らな側 天・枝 右、平らな側 左・枝 30° ＝ 平らな側 天・枝 120°） ===")
mx = 0.0
for (f1, d1, f2, d2) in ((180, 180, 0, 0), (90, 180, 0, 90), (270, 30, 0, 120)):
    Ba, Bb = Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=f1), Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=f2)
    ba, bb = Branch(406.4, 9.0, 600.0, d1, 60.0, 900.0), Branch(406.4, 9.0, 600.0, d2, 60.0, 900.0)
    ma, mb = branch_dev(Ba, ba)["marks"], branch_dev(Bb, bb)["marks"]
    ha, hb_ = hole(Ba, ba, "S"), hole(Bb, bb, "S")
    Fa, Fb = Fan(Ba), Fan(Bb)
    mx = max(mx, max(abs(p - q) for p, q in zip(ma, mb)), abs(shoelace([Fa.pt(x, a) for x, a in ring_xa(ha)]) - shoelace([Fb.pt(x, a) for x, a in ring_xa(hb_)])) / 1e3)
print(f"枝の割り出しの差・穴の面積の差（÷1000）の最大 {mx:.2e}")

print("\n=== 確かめ7. 板に直角に切った穴の両端は、片寄せ 0 の同心なら径の向きの穴より (T/2)·sinγ ずつ長い ===")
for D1, D2, L, T in ((1016.0, 812.8, 812.8, 12.0), (914.4, 1219.2, 600.0, 12.0), (1219.2, 914.4, 400.0, 16.0)):
    Bq = Body(D1, D2, L, T)
    bq = Branch(406.4, 9.0, L / 2, 0, 0.0, 900.0)
    hs, hr = hole(Bq, bq, "S"), hole(Bq, bq, "R")
    xs = [x for x, _ in hs["lo"] + hs["hi"]]; xr = [x for x, _ in hr["lo"] + hr["hi"]]
    sg = abs(Bq.k) / sqrt(1 + Bq.k ** 2)
    print(f"{D1}→{D2}・長さ {L}・板厚 {T}（半頂角 {deg(math.asin(sg)):.3f}°）: 展開の面の x で 伸び {min(xr) - min(xs):.4f}・{max(xs) - max(xr):.4f}（(T/2)·sinγ の式で {T / 2 * sg:.4f}。母線に沿うと (T/2)·tanγ＝{T / 2 * abs(Bq.k):.4f}）")

print("\n=== 確かめ8. 面の法線 n が、展開の面の母線と円の向きに直角 ===")
Bn = Body(1219.2, 914.4, 1200.0, 12.0, ecc=True, flat=40)
mx = 0.0
for i in range(36):
    a = 2 * pi * i / 36
    kap = Bn.k + Bn.cs * cos(a - Bn.f)
    N = sqrt(1 + kap * kap)
    n = (-kap / N, cos(a) / N, sin(a) / N)
    gx = (1.0, Bn.cs * cos(Bn.f) + Bn.k * cos(a), Bn.cs * sin(Bn.f) + Bn.k * sin(a))
    ga = (0.0, -sin(a), cos(a))
    mx = max(mx, abs(sum(p * q for p, q in zip(n, gx))), abs(sum(p * q for p, q in zip(n, ga))))
print(f"内積の最大 {mx:.2e}")
