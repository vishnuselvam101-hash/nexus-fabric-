# 1本の本管に枝が2本以上つく形と、枝の向き（天・横・地・角度）の展開の期待値を、画面の計算（shared.js）と別に出す。
#   python verify_branches.py
# 標準ライブラリだけ。寸法は mm、角度は度。式は spec_branches.md。
import math, sys
sys.stdout.reconfigure(encoding="utf-8")
PI, RHO = math.pi, 7.85e-6
asin, sqrt, sin, cos = math.asin, math.sqrt, math.sin, math.cos
rad = math.radians
NG = []


def ok(name, got, want, tol):
    good = abs(got - want) <= tol
    if not good: NG.append(name)
    print(f"  {'OK' if good else 'NG'} {name}: {got:.4f}（照らす値 {want:.4f}）")


def wrap(a): return a % (2 * PI)


# 本管の方位 α: 本管を始点側の管端（x=0）から見て 天=0・右=90・地=180・左=270
def seam_angle(frm="地", ang=45.0):
    return wrap(rad(180 + ang) if frm == "地" else rad(-ang))


def signed_e(e, e_side, dr):
    # 片寄せ e（大きさ）を、方位が増える向きを正とした値にする。e_side は寄せる向きの方位
    if not e: return 0.0
    if e_side is None: return abs(e)
    d = (e_side - dr) % 360
    if abs(d - 90) < 1: return abs(e)
    if abs(d - 270) < 1: return -abs(e)
    raise ValueError("片寄せの向きが枝の向きに直角でない")


def hole(D, T, b, a_s, mount="on", M=720, face="inner"):
    """本管の板の上の穴。u は縦の継ぎ目からの周の長さ（板厚の中心）、x は始点側の管端から"""
    Ro, Ri, Rm = D / 2, D / 2 - T, D / 2 - T / 2
    Rc = Ri if face == "inner" else Ro
    ro = b["d"] / 2
    rh = ro if mount == "in" else ro - b["tb"]
    e = b["e"]
    c = wrap(rad(b["dir"]) - a_s)
    pts, clamped = [], []
    for i in range(M):
        th = 2 * PI * i / M
        q = (e + rh * sin(th)) / Rc
        if abs(q) > 1: clamped.append(i); q = max(-1.0, min(1.0, q))
        pts.append((Rm * (c + asin(q)), b["x"] + rh * cos(th)))
    lo = asin(max(-1.0, min(1.0, (e - rh) / Rc))); hi = asin(max(-1.0, min(1.0, (e + rh) / Rc)))
    return dict(pts=pts, clamped=clamped, Rm=Rm, Rc=Rc, rh=rh, e=e, c=c, lo=lo, hi=hi, x=b["x"])


def thick_w(Ri, Ro, rh, e, phi):
    # 本管の板厚の中（R=Ri〜Ro）の方位 φ で、枝の筒（半径 rh・片寄せ e）が通る軸方向の幅の半分。R を変えたときの一番広い所
    s = sin(phi)
    if abs(s) < 1e-15: m = abs(e)
    elif Ri <= e / s <= Ro: m = 0.0
    else: m = min(abs(Ri * s - e), abs(Ro * s - e))
    return sqrt(rh * rh - m * m) if m < rh else 0.0


def hole_thick(D, T, b, a_s, mount="on", M=4000):
    """片寄せが穴の半径より大きい直角の枝の穴: 本管の板厚の中（R=Ri〜Ro）のどこでも枝の筒が当たらない形（spec_lateral.md の B を α=90 で）"""
    Ro, Ri, Rm = D / 2, D / 2 - T, D / 2 - T / 2
    ro = b["d"] / 2
    rh = ro if mount == "in" else ro - b["tb"]
    e = b["e"]
    c = wrap(rad(b["dir"]) - a_s)
    # φ の両端: 寄せた側と反対の縁は本管の外面（Ro）で、寄せた側の縁は内面（Ri）で一番外に出る
    lo = asin(max(-1.0, min(1.0, (e - rh) / (Ri if e - rh < 0 else Ro))))
    hi = asin(max(-1.0, min(1.0, (e + rh) / (Ri if e + rh > 0 else Ro))))
    phis = [lo + (hi - lo) * k / M for k in range(M + 1)]
    low = [(Rm * (c + p), b["x"] - thick_w(Ri, Ro, rh, e, p)) for p in phis]
    up = [(Rm * (c + p), b["x"] + thick_w(Ri, Ro, rh, e, p)) for p in phis]
    return dict(pts=low + up[::-1], clamped=[], Rm=Rm, Rc=Ri, Ri=Ri, Ro=Ro, rh=rh, e=e, c=c, lo=lo, hi=hi, x=b["x"], thick=True)


def shoelace(p):
    a = 0.0
    for i in range(len(p)):
        x0, y0 = p[i]; x1, y1 = p[(i + 1) % len(p)]
        a += x0 * y1 - x1 * y0
    return abs(a) / 2


def perim(p, closed=True):
    n = len(p) if closed else len(p) - 1
    return sum(math.hypot(p[(i + 1) % len(p)][0] - p[i][0], p[(i + 1) % len(p)][1] - p[i][1]) for i in range(n))


def area_integral(h, N=200000):
    # 別の求め方: 方位 φ ごとの穴の軸方向の幅を足す。A = Rm ∫ 2√(rh²−(Rc·sinφ−e)²) dφ（φ は Rc の面の上の角）。板厚の中で合わせた穴は thick_w の幅
    Rm, Rc, rh, e = h["Rm"], h["Rc"], h["rh"], h["e"]
    a, b = (h["lo"], h["hi"]) if h.get("thick") else (asin(max(-1.0, (e - rh) / Rc)), asin(min(1.0, (e + rh) / Rc)))
    hh = (b - a) / N; s = 0.0
    for k in range(N + 1):
        p = a + k * hh
        if h.get("thick"): f = 2 * thick_w(h["Ri"], h["Ro"], rh, e, p)
        else:
            f = rh * rh - (Rc * sin(p) - e) ** 2
            f = 2 * sqrt(f) if f > 0 else 0.0
        s += f * (1 if k in (0, N) else 4 if k % 2 else 2)
    return Rm * s * hh / 3


def bbox(p):
    us = [q[0] for q in p]; xs = [q[1] for q in p]
    return min(us), max(us), min(xs), max(xs)


def gap(p1, p2, W):
    # 板の上の2つの穴の縁どうしの一番近い距離（板は周で一回りするので u は W でつながる）
    best = 1e18
    for (u1, x1) in p1:
        for (u2, x2) in p2:
            du = (u1 - u2) % W
            du = min(du, W - du)
            d = du * du + (x1 - x2) ** 2
            if d < best: best = d
    return sqrt(best)


def seam_gap(h, W):
    # 縦の継ぎ目（u=0 と u=W の線）から穴までの周の長さ。穴が継ぎ目にかかれば負の数（かかっている深さ）
    u0, u1, _, _ = bbox(h["pts"])
    if u0 < 0: return u0
    if u1 > W: return W - u1
    return min(u0, W - u1)


def branch_marks(Dm, Tm, d, tb, H, e, mount="on", n=16):
    # 枝管の切り口（今の branchPiece と同じ決まり: 載せるなら本管の外面、壁の中で一番高く当たる点）。0＝わき（片寄せの側）
    Ro = Dm / 2; Ri = Ro - Tm; ro = d / 2; ri = ro - tb; e = abs(e); Rc = Ri if mount == "in" else Ro
    out = []
    for k in range(n):
        th = PI / 2 + 2 * PI * k / n
        a, b = e + ri * sin(th), e + ro * sin(th)
        m = 0.0 if a * b <= 0 else min(abs(a), abs(b))
        out.append(H - sqrt(max(Rc * Rc - m * m, 0)))
    return out


def branch_area(Dm, Tm, d, tb, H, e, M=3600):
    rm = d / 2 - tb / 2
    L = branch_marks(Dm, Tm, d, tb, H, e, n=M)
    return sum(L) * 2 * PI * rm / M


def show_main(title, D, T, L, brs, a_s=None, mount="on"):
    a_s = seam_angle() if a_s is None else a_s
    Rm = D / 2 - T / 2; W = 2 * PI * Rm
    print(f"\n== {title} ==")
    print(f"  本管 外径{D}・板厚{T}・長さ{L}。板の幅 2π·Rm = {W:.2f}。縦の継ぎ目の方位 {math.degrees(a_s):.0f}°")
    hs = []
    for b in brs:
        rh0 = b["d"] / 2 if mount == "in" else b["d"] / 2 - b["tb"]
        # 片寄せが穴の半径より大きい枝は、板厚の中のどこでも当たらない形（画面の決まり 11/21）。ほかは本管の内面で合わせる
        h = hole_thick(D, T, b, a_s, mount) if abs(b["e"]) > rh0 else hole(D, T, b, a_s, mount)
        u0, u1, x0, x1 = bbox(h["pts"])
        A = shoelace(h["pts"]); Ai = area_integral(h)
        rng = (math.degrees(rad(b["dir"]) + h["lo"]), math.degrees(rad(b["dir"]) + h["hi"]))
        print(f"  枝 {b['name']}（外径{b['d']}・板厚{b['tb']}・管端から{b['x']}・向き{b['dir']}°・片寄せ{b['e']:+.2f}）")
        print(f"    穴の周方向の幅 {u1 - u0:.2f}・軸方向の幅 {x1 - x0:.2f}・穴の中心 継ぎ目から {Rm * h['c'] + Rm * asin(max(-1, min(1, h['e'] / h['Rc']))):.2f}")
        print(f"    穴の方位の範囲 {rng[0]:.2f}°〜{rng[1]:.2f}°・面積 {A:.1f}（別の求め方 {Ai:.1f}）")
        ok(f"{b['name']} 穴の面積（多角形 と 積分）", A, Ai, max(0.5, A * 2e-4))
        if h["clamped"] or (h.get("thick") and abs(h["hi"] if h["e"] > 0 else h["lo"]) >= PI / 2 - 1e-12):
            s0 = (h["Rc"] - abs(h["e"])) / h["rh"]
            want = 2 * h["rh"] * sqrt(1 - s0 * s0)
            got = 2 * thick_w(h["Ri"], h["Ro"], h["rh"], h["e"], PI / 2 if h["e"] > 0 else -PI / 2) if h.get("thick") else max(h["pts"][i][1] for i in h["clamped"]) - min(h["pts"][i][1] for i in h["clamped"])
            print(f"    穴の縁が管の線（方位 {math.degrees(rad(b['dir']) + (PI / 2 if h['e'] > 0 else -PI / 2)) % 360:.0f}°）に沿う長さ {want:.2f}（" + ("板厚の中の幅で" if h.get("thick") else "点で数えて") + f" {got:.2f}）")
        sg = seam_gap(h, W)
        print(f"    縦の継ぎ目から穴まで {sg:.2f}" + ("  ← 穴が継ぎ目にかかる" if sg < 0 else ""))
        if x0 < 0 or x1 > L: print("    ← 穴が管端からはみ出す")
        hs.append((b, h))
    for i in range(len(hs)):
        for j in range(i + 1, len(hs)):
            g = gap(hs[i][1]["pts"], hs[j][1]["pts"], W)
            print(f"  穴どうしの一番近い所（{hs[i][0]['name']}・{hs[j][0]['name']}）{g:.2f}")
            hs[i][1].setdefault("gaps", []).append(g)
    holes = sum(shoelace(h["pts"]) for _, h in hs)
    kg = (W * L - holes) * T * RHO
    print(f"  本管の重さ（穴を抜いた後）{kg:.2f} kg・切る前の四角 {W * L * T * RHO:.2f} kg")
    return dict(W=W, Rm=Rm, hs=hs, kg=kg)


# ---------------------------------------------------------------------------
print("■ 照らし1: 枝1本・天・片寄せなしで、今の T字管（test_build.mjs の E_tee_public）と同じ値になる")
r = show_main("例1 今の T字管（本管1016×12・枝610×9・長さ1500）", 1016, 12, 1500,
              [dict(name="枝", d=610, tb=9, x=750, dir=0, e=0)])
h = r["hs"][0][1]; u0, u1, x0, x1 = bbox(h["pts"])
ok("穴の周方向の幅（今の値 642.03）", u1 - u0, 642.03, 0.05)
ok("穴の軸方向の幅（今の値 592.0）", x1 - x0, 592.0, 0.05)
ok("穴の中心 継ぎ目から（今の値 1182.81）", r["Rm"] * h["c"], 1182.81, 0.05)

print("\n■ 照らし2: 枝の向きを回すと、穴は同じ形のまま周方向に Rm×角度 だけ動く")
for dr in (45, 90, 180, 270):
    h2 = hole(1016, 12, dict(d=610, tb=9, x=750, dir=dr, e=0), seam_angle())
    sh = r["Rm"] * rad(dr)
    W = r["W"]
    dif = max(math.hypot(((p2[0] - p1[0] - sh) + W / 2) % W - W / 2, p2[1] - p1[1]) for p1, p2 in zip(h["pts"], h2["pts"]))
    ok(f"向き {dr}°: 天の穴を {sh:.2f} 動かした形との一番大きいずれ", dif, 0.0, 1e-6)

print("\n■ 照らし3: 板厚ゼロでは、穴の周の長さ・枝の切り口の長さ・3D の交わりの線の長さが同じ（展開しても長さは変わらない）")
for (Rr, rr, ee, dr) in [(500.0, 300.0, 0.0, 0), (500.0, 300.0, 150.0, 90), (500.0, 100.0, 400.0, 90), (500.0, 300.0, -150.0, 200)]:
    M = 20000
    hb = hole(2 * Rr, 1e-9, dict(d=2 * rr, tb=1e-9, x=0, dir=dr, e=ee), seam_angle(), M=M)
    Lh = perim(hb["pts"])
    P, B = [], []
    for i in range(M):
        th = 2 * PI * i / M; z = ee + rr * sin(th); y = sqrt(max(Rr * Rr - z * z, 0)); x = rr * cos(th)
        a = rad(dr); P.append((x, y * cos(a) - z * sin(a), y * sin(a) + z * cos(a)))
        B.append((rr * th, -y))
    L3 = sum(math.dist(P[i], P[(i + 1) % M]) for i in range(M))
    Lb = perim(B + [(2 * PI * rr, B[0][1])], closed=False)
    print(f"  本管R{Rr:.0f}・枝r{rr:.0f}・片寄せ{ee:+.0f}・向き{dr}°: 穴 {Lh:.3f}・切り口 {Lb:.3f}・3D {L3:.3f}")
    ok("  穴の周の長さ ＝ 3D", Lh, L3, 0.01); ok("  切り口の長さ ＝ 3D", Lb, L3, 0.01)

print("\n■ 照らし4: 管底にそろえた枝（接線の枝）で、上側の当たる高さが AWWA C208 の式と同じ（板厚ゼロ）")
Do, d = 2.0, 0.8
R, rr = Do / 2, d / 2
e = R - rr
y_top = sqrt(R * R - (e - rr) ** 2)
al = math.acos((0.5 * Do - d) / (0.5 * Do))
ok("上側の高さ √(R²−(R−d)²) と 0.5·Do·sinα", y_top, 0.5 * Do * sin(al), 1e-12)

# ---------------------------------------------------------------------------
print("\n■ 例2 十字管（手で作った例。本管1016×12・長さ1500、枝610×9 が2本、どちらも管端から750）")
x2 = show_main("十字管 天と地", 1016, 12, 1500, [dict(name="天の枝", d=610, tb=9, x=750, dir=0, e=0), dict(name="地の枝", d=610, tb=9, x=750, dir=180, e=0)])
ok("2つの穴の中心の差 ＝ π·Rm", r["Rm"] * ((x2["hs"][1][1]["c"] - x2["hs"][0][1]["c"]) % (2 * PI)), PI * 502, 1e-6)
g = x2["hs"][0][1]["gaps"][0]; hw = r["Rm"] * (h["hi"] - h["lo"])
ok("穴どうしの一番近い所 ＝ π·Rm − 穴の周方向の幅", g, PI * 502 - hw, 0.05)
show_main("十字管 右と左", 1016, 12, 1500, [dict(name="右の枝", d=610, tb=9, x=750, dir=90, e=0), dict(name="左の枝", d=610, tb=9, x=750, dir=270, e=0)])
x3 = show_main("天と右（90°ずつ。同じ位置）", 1016, 12, 1500, [dict(name="天の枝", d=610, tb=9, x=750, dir=0, e=0), dict(name="右の枝", d=610, tb=9, x=750, dir=90, e=0)])
ok("天と右の穴どうし ＝ Rm·(90° − 2·asin(ri/Ri))", x3["hs"][0][1]["gaps"][0], 502 * (PI / 2 - 2 * asin(296 / 496)), 0.05)
Ro = 508
print(f"  溶接の側（本管の外面で、枝の外面どうし）: Ro·(90° − 2·asin(ro/Ro)) = {Ro * (PI / 2 - 2 * asin(305 / Ro)):.2f}")


def pad_outline(Dm, tp, Dp, b, a_s, M=720):
    # 補強板の外周（今の padPiece と同じ: 平面図の円を補強板の外面で合わせ、長さは補強板の板厚の中心で測る）を本管の方位に置く
    Ro = Dm / 2; Rp = Ro + tp / 2; Rt = Ro + tp; c = wrap(rad(b["dir"]) - a_s)
    pts = [(Rp * (c + asin(max(-1, min(1, (b["e"] + Dp / 2 * sin(2 * PI * i / M)) / Rt)))), b["x"] + Dp / 2 * cos(2 * PI * i / M)) for i in range(M)]
    lo = math.degrees(rad(b["dir"]) + asin((b["e"] - Dp / 2) / Rt)); hi = math.degrees(rad(b["dir"]) + asin((b["e"] + Dp / 2) / Rt))
    return pts, Rp, lo, hi


def inside(q, poly, W):
    for s in (-W, 0, W):
        x, y = q[0] + s, q[1]; c = False
        for i in range(len(poly)):
            (x0, y0), (x1, y1) = poly[i], poly[i - 1]
            if (y0 > y) != (y1 > y) and x < (x1 - x0) * (y - y0) / (y1 - y0) + x0: c = not c
        if c: return True
    return False


print("\n== 補強板（板厚12・平面図の外径1000）を2本の枝に付けたとき ==")
for nm, d2 in (("天と地", 180), ("天と右", 90)):
    p1, Rp, l1, h1 = pad_outline(1016, 12, 1000, dict(dir=0, e=0, x=750), seam_angle())
    p2, _, l2, h2 = pad_outline(1016, 12, 1000, dict(dir=d2, e=0, x=750), seam_angle())
    Wp = 2 * PI * Rp
    ov = any(inside(q, p2, Wp) for q in p1[::6]) or any(inside(q, p1, Wp) for q in p2[::6])
    print(f"  {nm}: 補強板の方位の範囲 {l1:.2f}°〜{h1:.2f}° と {l2:.2f}°〜{h2:.2f}°。" + ("重なる" if ov else f"重ならない（一番近い所 {gap(p1, p2, Wp):.2f}）"))
    s = 225.0
    under = [n for n, (lo, hi) in (("天", (l1, h1)), (nm[-1], (l2, h2))) if lo <= s <= hi or lo <= s - 360 <= hi]
    print(f"    縦の継ぎ目（方位225°）が補強板の下を通る: {'・'.join(under) if under else 'なし'}")
ok("天と地の補強板どうし ＝ Rp·(180° − 2·asin(Dp/2/(Ro+tp)))", gap(pad_outline(1016, 12, 1000, dict(dir=0, e=0, x=750), seam_angle())[0], pad_outline(1016, 12, 1000, dict(dir=180, e=0, x=750), seam_angle())[0], 2 * PI * 514), 514 * (PI - 2 * asin(500 / 520)), 0.05)

# ---------------------------------------------------------------------------
print("\n■ 例3 排水T字管 600×200（JIS G 3443-2 表13・大阪広域水道企業団 標準図 6-92 の寸法。枝は横に出て、管底の外面にそろう）")
D, T, d, tb, Hh, I = 609.6, 6.0, 216.3, 5.8, 750, 500
e = (D - d) / 2
print(f"  片寄せ e = (D2 − d2)/2 = {e:.2f}（枝の向き 右＝90°、寄せる向き 地＝180° → 正）")
ex3 = show_main("排水T字管 600×200（本管の長さ 2H = 1500）", D, T, 2 * Hh, [dict(name="排水の枝", d=d, tb=tb, x=Hh, dir=90, e=signed_e(e, 180, 90))])
Ri = D / 2 - T; rh = d / 2 - tb
print(f"  今の決まりの止め（e + 穴の半径 ≤ 本管の内面の半径）: {e + rh:.2f} > {Ri:.2f} なので、今は「枝の穴が本管に収まらない」で止まる")
for face, nm in (("inner", "本管の内面だけで合わせる（11/21 までの画面の穴）"), ("outer", "本管の外面だけで合わせる")):
    hoo = hole(D, T, dict(d=d, tb=tb, x=Hh, dir=90, e=e), seam_angle(), face=face)
    u0, u1, x0, x1 = bbox(hoo["pts"])
    print(f"  別の決め方（{nm}）: 周方向の幅 {u1 - u0:.2f}・軸方向 {x1 - x0:.2f}・面積 {shoelace(hoo['pts']):.1f}")
th3 = ex3["hs"][0][1]; Ro3 = D / 2
ok("  板厚の中で合わせた穴の、本管の中心の側の縁 ＝ 外面で枝の内径が通る方位 asin((e−rh)/Ro)", th3["lo"], asin((e - rh) / Ro3), 1e-12)
print(f"  内面だけで合わせた穴との差（本管の中心の側・周の長さ）{th3['Rm'] * (asin((e - rh) / Ri) - asin((e - rh) / Ro3)):.2f} mm")
mk = branch_marks(D, T, d, tb, I, e)
print(f"  排水の枝（H＝I＝{I}）の管端から切り口まで: 管底の側 {mk[0]:.2f}・管端の向き {mk[4]:.2f}・上の側 {mk[8]:.2f}・管端の向き（反対）{mk[12]:.2f}")
print(f"    管底の側の当たる高さ √(Ro²−(Ro−t)²) = {sqrt((D / 2) ** 2 - (D / 2 - tb) ** 2):.2f}")
ok("  管底の側 ＝ I − √(Ro²−(Ro−t)²)", mk[0], I - sqrt((D / 2) ** 2 - (D / 2 - tb) ** 2), 1e-9)
kb = branch_area(D, T, d, tb, I, e) * tb * RHO
print(f"  枝の重さ {kb:.2f} kg。本管と枝の合計 {ex3['kg'] + kb:.2f} kg（表の参考質量 141 kg。リブ・フランジの有無が表から読めないので、照らしには使わない）")

# ---------------------------------------------------------------------------
print("\n■ 例4 仕切弁副管A1号 1000×600×200（JIS G 3443-2 表14・標準図 6-116〜6-117 の寸法を借りた例。e=2500 を本管の長さとし、フランジの分は引いていない）")
D, T = 1016.0, 9.0
e4 = (D - 216.3) / 2
print(f"  弁口 609.6×6.0 は天、管端から e1=1000。排水の枝 216.3×5.8 は右・管底にそろう、管端から e − e3 = 2500 − 400 = 2100。片寄せ {e4:.2f}")
a4 = show_main("仕切弁副管A1（継ぎ目は地から45°・左）", D, T, 2500,
               [dict(name="弁口", d=609.6, tb=6.0, x=1000, dir=0, e=0), dict(name="排水の枝", d=216.3, tb=5.8, x=2100, dir=90, e=signed_e(e4, 180, 90))])
a4m = show_main("同じ形で排水の枝を左に出したとき（継ぎ目は地から45°・左のまま）", D, T, 2500,
                [dict(name="弁口", d=609.6, tb=6.0, x=1000, dir=0, e=0), dict(name="排水の枝", d=216.3, tb=5.8, x=2100, dir=270, e=signed_e(e4, 180, 270))])
a4r = show_main("排水の枝が左のとき、継ぎ目を地から45°・右（方位135°）に置くと", D, T, 2500,
                [dict(name="弁口", d=609.6, tb=6.0, x=1000, dir=0, e=0), dict(name="排水の枝", d=216.3, tb=5.8, x=2100, dir=270, e=signed_e(e4, 180, 270))], a_s=wrap(rad(135)))
mk1 = branch_marks(D, T, 609.6, 6.0, 800, 0); mk2 = branch_marks(D, T, 216.3, 5.8, 670, e4)
print(f"  弁口（H2=800 をそのまま使う）: わき {mk1[0]:.2f}・背 {mk1[4]:.2f}")
print(f"  排水の枝（l=670 をそのまま使う）: 管底の側 {mk2[0]:.2f}・上の側 {mk2[8]:.2f}")

# ---------------------------------------------------------------------------
print("\n■ 例5 仕切弁副管A2号 1000×600×200（表14 の続き・標準図 6-120〜6-121。f=1655 を本管の長さ、弁口と排水の枝は同じ位置 f1=1000）")
a5 = show_main("仕切弁副管A2", D, T, 1655,
               [dict(name="弁口", d=609.6, tb=6.0, x=1000, dir=0, e=0), dict(name="排水の枝", d=216.3, tb=5.8, x=1000, dir=90, e=signed_e(e4, 180, 90))])
Rm, Ri = D / 2 - T / 2, D / 2 - T
want = Rm * ((PI / 2 + asin((e4 - (108.15 - 5.8)) / (D / 2))) - asin((304.8 - 6.0) / Ri))
ok("弁口と排水の枝の穴どうし ＝ Rm·(90° + asin((e−rh排水)/Ro) − asin(rh弁口/Ri))（排水の穴は板厚の中で合わせ、本管の中心の側の縁は外面で決まる）", a5["hs"][0][1]["gaps"][0], want, 0.05)

# ---------------------------------------------------------------------------
print("\n■ 例6 人孔と空気弁（手で作った例。本管1219.2×11・長さ2400。人孔 609.6×6 と空気弁 114.3×4.5 がどちらも天）")
a6 = show_main("人孔と空気弁", 1219.2, 11, 2400, [dict(name="人孔", d=609.6, tb=6, x=800, dir=0, e=0), dict(name="空気弁", d=114.3, tb=4.5, x=1500, dir=0, e=0)])
ok("同じ向きの2つの穴どうし ＝ 管端からの距離の差 − 2つの穴の半径（軸方向）", a6["hs"][0][1]["gaps"][0], (1500 - 52.65) - (800 + 298.8), 0.05)

print("\n" + ("全部合った" if not NG else "合わない所 " + str(len(NG)) + " 件: " + "・".join(NG)))
sys.exit(1 if NG else 0)
