# 斜めの枝（ラテラル）の展開の期待値を、画面の計算（shared.js）と別に出す。python verify_lateral.py
# 座標は shared.js の T字管と同じ: 本管の軸を x、天を +y、わきを +z。枝の軸は (cos α, sin α, 0) の向きで +x の側へ倒れる（α=90 で T字管）。
# 枝の軸は側面図の交点 (x0, 0, e) を通る。θ は枝の周りの角度で、0＝鋭角の側（枝が倒れている側・α=90 なら本管の軸の向き）、90°＝わき、180°＝鈍角の側。
# 周方向の長さは板厚の中心で測る。角度と高さは実際に当たる面で決める。
import math, sys
sys.stdout.reconfigure(encoding="utf-8")
sin, cos, tan, asin, sqrt, pi = math.sin, math.cos, math.tan, math.asin, math.sqrt, math.pi
RHO = 7.85e-6
rad = math.radians


def wall_tau(Rc, rho, th, e, a):
    # 枝の壁の母線 (θ, ρ) が本管の面（半径 Rc）に当たる所の、交点から枝の軸に沿った位置
    z = e + rho * sin(th)
    if abs(z) > Rc:
        return None
    return (sqrt(Rc * Rc - z * z) + rho * cos(th) * cos(a)) / sin(a)


def cut_tau(Rc, ri, ro, e, th, a):
    # 壁（ri..ro）のうち一番先に当たる所。ρ について凹の関数なので、山の頂点を区間に収めればよい
    s, c, ca = sin(th), cos(th), cos(a)
    lo, hi = ri, ro
    if abs(s) > 1e-15:
        b1, b2 = (-Rc - e) / s, (Rc - e) / s
        lo, hi = max(lo, min(b1, b2)), min(hi, max(b1, b2))
    if lo > hi + 1e-12:
        return None
    k = c * ca
    if abs(s) < 1e-15:
        rho = hi if k > 0 else lo
    else:
        zs = 0.0 if abs(k) < 1e-15 else math.copysign(abs(k) * Rc / sqrt(s * s + k * k), k * s)
        rho = min(max((zs - e) / s, lo), hi)
    return wall_tau(Rc, rho, th, e, a)


def cut_tau_brute(Rc, ri, ro, e, th, a, n=20001):
    vals = [wall_tau(Rc, ri + (ro - ri) * i / (n - 1), th, e, a) for i in range(n)]
    vals = [v for v in vals if v is not None]
    return max(vals) if vals else None


def branch(Do, Tm, d, t, G, e, a, mount="on", M=3600):
    Ro, ro = Do / 2, d / 2
    Ri, ri, rm = Ro - Tm, ro - t, ro - t / 2
    Rc = Ri if mount == "in" else Ro
    th0 = pi / 2
    ln = lambda th: G - cut_tau(Rc, ri, ro, e, th, a)
    pts = [(2 * pi * rm * i / M, ln(th0 + 2 * pi * i / M)) for i in range(M + 1)]
    area = sum((pts[i + 1][0] - pts[i][0]) * (pts[i + 1][1] + pts[i][1]) / 2 for i in range(M))
    marks = [ln(th0 + 2 * pi * k / 16) for k in range(17)]
    return {"circ": 2 * pi * rm, "marks": marks, "short": min(p[1] for p in pts), "long": max(p[1] for p in pts), "area": area, "kg": area * t * RHO}


def footprint(R, rh, e, a, phi):
    # 半径 R の面の角度 φ（天から）で、枝の筒（半径 rh）の中に入る x の範囲（交点 x0 からの位置）
    z = R * sin(phi)
    q = rh * rh - (z - e) ** 2
    if q < 0:
        return None
    c, w = R * cos(phi) / tan(a), sqrt(q) / sin(a)
    return c - w, c + w


def hole(R1, R2, Rm, rh, e, a, nR=121, nu=4001):
    # 本管（または補強板）の板の穴。板厚の中（半径 R1..R2）のどこでも枝の筒が当たらない範囲。R1=R2 なら1つの面で合わせた穴
    Rs = [R1 + (R2 - R1) * j / (nR - 1) for j in range(nR)] if nR > 1 and R2 != R1 else [R1]
    plo = min(asin(max(-1, min(1, (e - rh) / R))) for R in Rs)
    phi_hi = max(asin(max(-1, min(1, (e + rh) / R))) for R in Rs)
    lo_edge, hi_edge = [], []
    for i in range(nu + 1):
        phi = plo + (phi_hi - plo) * i / nu
        iv = [footprint(R, rh, e, a, phi) for R in Rs]
        iv = [v for v in iv if v]
        if not iv:
            continue
        lo_edge.append((Rm * phi, min(v[0] for v in iv)))
        hi_edge.append((Rm * phi, max(v[1] for v in iv)))
    area = sum((hi_edge[i + 1][0] - hi_edge[i][0]) * ((hi_edge[i + 1][1] - lo_edge[i + 1][1]) + (hi_edge[i][1] - lo_edge[i][1])) / 2 for i in range(len(hi_edge) - 1))
    return {"W": Rm * (phi_hi - plo), "xmin": min(p[1] for p in lo_edge), "xmax": max(p[1] for p in hi_edge), "area": area, "lo": lo_edge, "hi": hi_edge}


def hole_marks(Ri, Rm, rh, e, a, k=16):
    # 1つの面（内面）で合わせた穴の割り出し（今の T字管と同じ取り方）: 継ぎ目を除いた天からの周の長さと、交点からの軸方向の位置
    out = []
    for i in range(k):
        th = 2 * pi * i / k
        z = e + rh * sin(th)
        phi = asin(z / Ri)
        out.append((Rm * phi, Ri * cos(phi) / tan(a) + rh * cos(th) / sin(a)))
    return out


def pad_offset(lo, hi, B):
    # 穴の縁を展開図の上で B だけ外へ平行にずらした線（補強板の外周）。外周の囲む四角と面積
    ring = lo + hi[::-1]
    n = len(ring)
    area2 = sum(ring[i][0] * ring[(i + 1) % n][1] - ring[(i + 1) % n][0] * ring[i][1] for i in range(n))
    sg = 1 if area2 > 0 else -1
    out = []
    for i in range(n):
        p0, p1 = ring[(i - 1) % n], ring[(i + 1) % n]
        tx, ty = p1[0] - p0[0], p1[1] - p0[1]
        tl = math.hypot(tx, ty) or 1
        out.append((ring[i][0] + sg * B * ty / tl, ring[i][1] - sg * B * tx / tl))
    xs, ys = [p[0] for p in out], [p[1] for p in out]
    ao = abs(sum(out[i][0] * out[(i + 1) % n][1] - out[(i + 1) % n][0] * out[i][1] for i in range(n))) / 2
    return {"w": max(xs) - min(xs), "h": max(ys) - min(ys), "outer_area": ao, "hole_area": abs(area2) / 2}


def curve3d_len(R, r, e, a, N=200000):
    # 板厚0の交わりの線の長さ（3D）・枝の展開の切り口の長さ・本管の穴の周の長さ
    P, Bc, Hc = [], [], []
    for i in range(N + 1):
        th = 2 * pi * i / N
        tau = wall_tau(R, r, th, e, a)
        z = e + r * sin(th)
        x = tau * cos(a) + r * cos(th) * sin(a)
        y = tau * sin(a) - r * cos(th) * cos(a)
        P.append((x, y, z)); Bc.append((r * th, tau)); Hc.append((R * asin(z / R), x))
    L = lambda Q: sum(math.dist(Q[i], Q[i + 1]) for i in range(len(Q) - 1))
    return L(P), L(Bc), L(Hc)


def show(name, v, d=2):
    print(f"  {name}: {v:.{d}f}")


print("== 確かめ1: α=90 で今の T字管（verify_tee.py の例1）と一致する ==")
Do, Tm, d, t, G, L, x0 = 1016.0, 12.0, 610.0, 9.0, 900.0, 1500.0, 750.0
Ro, Ri, Rm, ro, ri = Do / 2, Do / 2 - Tm, Do / 2 - Tm / 2, d / 2, d / 2 - t
b = branch(Do, Tm, d, t, G, 0, rad(90))
show("枝 板の幅", b["circ"]); print("  枝 割り出し（わき→鈍角の側→わき→鋭角の側）:", [round(v, 2) for v in b["marks"][:9]])
hA = hole(Ri, Ri, Rm, ri, 0, rad(90)); hU = hole(Ri, Ro, Rm, ri, 0, rad(90))
show("本管の穴 周方向（内面で合わせる）", hA["W"]); show("本管の穴 軸方向", hA["xmax"] - hA["xmin"])
show("本管の穴 周方向（板厚の中で当たらない）", hU["W"]); show("本管の穴 軸方向（同）", hU["xmax"] - hU["xmin"])
bi = branch(Do, Tm, d, t, G, 0, rad(90), "in")
print("  差し込む 枝 わき・鈍角の側:", round(bi["marks"][0], 2), round(bi["marks"][4], 2)); show("差し込む 本管の穴 周方向", hole(Ri, Ro, Rm, ro, 0, rad(90))["W"])
pU = hole(Ro, Ro + 12, Ro + 6, ro, 0, rad(90))
show("補強板 穴の周方向（枝の外面が本管の外面に当たる線）", pU["W"])
b150 = branch(Do, Tm, d, t, G, 150, rad(90))
print("  片寄せ150 枝 θ=-90°・0°・90° の当たる高さ:", round(G - b150["marks"][8], 2), round(G - b150["marks"][12], 2), round(G - b150["marks"][0], 2))
hA150, hU150 = hole(Ri, Ri, Rm, ri, 150, rad(90)), hole(Ri, Ro, Rm, ri, 150, rad(90))
dev = max(max(abs(hA150["lo"][i][1] - hU150["lo"][i][1]), abs(hA150["hi"][i][1] - hU150["hi"][i][1])) for i in range(min(len(hA150["lo"]), len(hU150["lo"]))))
show("片寄せ150 穴の2つの決め方の差（一番大きい所）", dev, 3)

print("== 確かめ2: 壁の中で一番先に当たる所（閉じた式と、壁を2万に刻んだ総当たり） ==")
worst = 0
for (Rc, rI, rO, e, al) in [(508, 296, 305, 0, 45), (508, 296, 305, 150, 60), (496, 194.2, 203.2, 150, 60), (508, 499, 508, 0, 45), (508, 296, 305, 200, 30)]:
    for k in range(64):
        th = 2 * pi * k / 64
        p, q = cut_tau(Rc, rI, rO, e, th, rad(al)), cut_tau_brute(Rc, rI, rO, e, th, rad(al))
        worst = max(worst, abs(p - q))
show("一番大きい差 mm", worst, 6)

print("== 確かめ3: 板厚0・同径・片寄せなし。切り口は平面2枚＝えび曲げの片（片側の角 γ）の半周ずつ ==")
R = 508.0
for al in (30, 45, 60):
    a = rad(al)
    ga, gb = 90 - al / 2, al / 2
    err = 0
    for k in range(360):
        th = 2 * pi * k / 360
        miter = R * tan(rad(ga)) * cos(th) if cos(th) >= 0 else -R * tan(rad(gb)) * cos(th)
        err = max(err, abs(wall_tau(R, R, th, 0, a) - miter))
    print(f"  α={al}°: 鋭角の側の半周は γ={ga}°、鈍角の側は γ={gb}° のえび曲げの切り口。差 {err:.1e}。振れ幅 R·tanγ = {R*tan(rad(ga)):.2f} / {R*tan(rad(gb)):.2f}")

print("== 確かめ4: AWWA C208 のラテラルの寸法（幾何の部分。余裕 f を除く）と、外面の交わりの端 ==")
for (Do_, d_, al) in [(1016, 1016, 45), (1016, 610, 45), (1016, 610, 60)]:
    a = rad(al); Ro_, ro_ = Do_ / 2, d_ / 2
    xa = (Ro_ + ro_ * cos(a)) / sin(a) * cos(a) + ro_ * sin(a)
    xo = (Ro_ - ro_ * cos(a)) / sin(a) * cos(a) - ro_ * sin(a)
    ta = (Ro_ + ro_ * cos(a)) / sin(a)
    if Do_ == d_:
        Gw, G1w, Gow = Do_ / (2 * tan(a / 2)), Do_ / 2 * tan(a / 2), Do_ / (2 * tan(a / 2))
    else:
        Gw, G1w, Gow = Do_ / (2 * tan(a)) + d_ / (2 * sin(a)), d_ / (2 * sin(a)) - Do_ / (2 * tan(a)), Do_ / (2 * sin(a)) + d_ / (2 * tan(a))
    print(f"  Do={Do_} d={d_} α={al}°: 鋭角の端 x={xa:.2f}（G {Gw:.2f}）・鈍角の端 x={xo:.2f}（−G1 {-G1w:.2f}）・枝の軸に沿って {ta:.2f}（Go {Gow:.2f}）")

print("== 確かめ5: 板厚0の長さ（3D の交わり・枝の切り口・本管の穴の周）が3つとも同じ ==")
for (R_, r_, e_, al) in [(1.0, 1.0, 0.0, 45), (1.0, 0.5, 0.25, 45), (508.0, 305.0, 150.0, 60)]:
    l3, lb, lh = curve3d_len(R_, r_, e_, rad(al))
    print(f"  R={R_} r={r_} e={e_} α={al}°: {l3:.5f} / {lb:.5f} / {lh:.5f}")

print("== 確かめ6: 穴の面積は T字管の穴の面積 ÷ sin α（軸方向に 1/sin α 伸ばして、ずらすだけ） ==")
for (rh, e_, al) in [(296.0, 0.0, 45), (296.0, 150.0, 60), (194.2, 100.0, 30)]:
    tee, lat = hole(Ri, Ri, Rm, rh, e_, rad(90)), hole(Ri, Ri, Rm, rh, e_, rad(al))
    print(f"  ρh={rh} e={e_} α={al}°: 斜め {lat['area']:.1f}・T字管÷sinα {tee['area']/sin(rad(al)):.1f}・周方向の幅 {lat['W']:.2f}＝{tee['W']:.2f}")
th_area = 2 * pi * 508 * 900 - branch(1016, 1e-9, 1016, 1e-9, 900, 0, rad(45))["area"]
print(f"  板厚0・同径・α=45°: 枝から切り落とす面積 {th_area:.1f}、閉じた式 4R²/sinα = {4*508**2/sin(rad(45)):.1f}")

print("== 確かめ7: α を 90 に近づけると T字管に続く（片寄せ150・載せる。わき・鈍角の側・わき・鋭角の側） ==")
for al in (90, 89.99, 89.9):
    print(f"  α={al}:", [round(v, 2) for v in branch(1016, 12, 610, 9, 900, 150, rad(al))["marks"][:16:4]])

print("== 確かめ8: 穴を「板厚の中で当たらない形」にするとき、板厚を何段に刻めば足りるか（401段との一番大きい差 mm） ==")
for (rh, e_, al) in [(296.0, 0.0, 45), (194.2, 150.0, 60), (296.0, 150.0, 90)]:
    ref = hole(Ri, Ro, Rm, rh, e_, rad(al), nR=401)
    row = []
    for n in (2, 3, 5, 9, 17):
        h = hole(Ri, Ro, Rm, rh, e_, rad(al), nR=n)
        row.append(f"{n}段 {max(max(abs(h['lo'][i][1]-ref['lo'][i][1]), abs(h['hi'][i][1]-ref['hi'][i][1])) for i in range(len(h['lo']))):.4f}")
    print(f"  ρh={rh} e={e_} α={al}°: " + "・".join(row))


def example(name, Do, Tm, d, t, G, L, x0, e, al, mount, tp=None, B=None):
    print(f"== {name} ==")
    a = rad(al); Ro, Ri, Rm, ro, ri = Do / 2, Do / 2 - Tm, Do / 2 - Tm / 2, d / 2, d / 2 - t
    b = branch(Do, Tm, d, t, G, e, a, mount)
    show("枝 板の幅", b["circ"]); print("  枝 割り出し（0＝わき、4＝鈍角の側、8＝わき、12＝鋭角の側）:", [round(v, 2) for v in b["marks"][:16]])
    show("枝 一番短い所", b["short"]); show("枝 一番長い所", b["long"]); show("枝 重さ kg", b["kg"], 1)
    rh = ro if mount == "in" else ri
    hA, hU = hole(Ri, Ri, Rm, rh, e, a), hole(Ri, Ro, Rm, rh, e, a)
    for nm, h in (("内面で合わせる", hA), ("板厚の中で当たらない", hU)):
        print(f"  本管の穴（{nm}）: 周方向 {h['W']:.2f}・軸方向 {h['xmax']-h['xmin']:.2f}（管端から {x0+h['xmin']:.2f}〜{x0+h['xmax']:.2f}）・面積 {h['area']:.0f}")
    hO = hole(Ro, Ro, Rm, rh, e, a)
    print(f"  本管の穴（外面で合わせる）: 軸方向 {hO['xmax']-hO['xmin']:.2f}（管端から {x0+hO['xmin']:.2f}〜{x0+hO['xmax']:.2f}）")
    print("  本管の穴 割り出し（内面で合わせる。天からの周の長さ, 管端からの長さ）:", [(round(u, 1), round(x0 + x, 1)) for (u, x) in hole_marks(Ri, Rm, rh, e, a)[::2]])
    show("本管 板の幅 2π·Rm", 2 * pi * Rm); show("本管 重さ kg（穴は板厚の中で当たらない形）", (2 * pi * Rm * L - hU["area"]) * Tm * RHO, 1)
    ux = lambda h, u: next(((lo[1], hi[1]) for lo, hi in zip(h["lo"], h["hi"]) if lo[0] >= u - 1e-9), None)
    u0 = Rm * asin(e / Ri)
    print(f"  枝の中心の周の位置（天から {u0:.1f}）での穴の縁 管端から: 内面で合わせる {x0 + ux(hA, u0)[0]:.2f}〜{x0 + ux(hA, u0)[1]:.2f}・板厚の中で当たらない {x0 + ux(hU, u0)[0]:.2f}〜{x0 + ux(hU, u0)[1]:.2f}")
    if tp:
        ph = hole(Ro, Ro + tp, Ro + tp / 2, ro, e, a)
        pd = pad_offset(ph["lo"], ph["hi"], B)
        Hh = ph["xmax"] - ph["xmin"]
        perim = sum(math.dist(p, q) for p, q in zip(ph["lo"][:-1], ph["lo"][1:])) + sum(math.dist(p, q) for p, q in zip(ph["hi"][:-1], ph["hi"][1:])) + abs(ph["hi"][0][1] - ph["lo"][0][1]) + abs(ph["hi"][-1][1] - ph["lo"][-1][1])
        steiner = perim * B + pi * B * B
        print(f"  補強板 板厚{tp}・幅{B}: 外周の周方向 {ph['W'] + 2 * B:.2f}・軸方向 {Hh + 2 * B:.2f}（穴＋2B）・穴の周方向 {ph['W']:.2f}・穴の軸方向 {Hh:.2f}")
        print(f"  補強板 板の面積 {pd['outer_area'] - pd['hole_area']:.0f}（ずらした線で囲んだ面積−穴）＝ {steiner:.0f}（穴の周×B＋πB²）・重さ {steiner * tp * RHO:.1f} kg")
        ph1 = hole(Ro, Ro, Ro + tp / 2, ro, e, a)
        print(f"  補強板の穴を本管の外面だけで合わせると: 軸方向 {ph1['xmax']-ph1['xmin']:.2f}（補強板の外面で枝が当たる量 約 {tp/tan(a):.2f}）")


example("例1 ラテラル45°・同径 1016×12（載せる）", 1016, 12, 1016, 12, 1400, 2400, 900, 0, 45, "on")
example("例2 ラテラル45°・異径 1016×12 に 610×9（載せる・補強板 12・幅150）", 1016, 12, 610, 9, 1200, 2000, 700, 0, 45, "on", 12, 150)
example("例3 ラテラル60°・片寄せ150・1016×12 に 406.4×9（差し込む）", 1016, 12, 406.4, 9, 1000, 1600, 600, 150, 60, "in")
example("例4 α=90（T字管・例1と同じ寸法・補強板 12・幅150）", 1016, 12, 610, 9, 900, 1500, 750, 0, 90, "on", 12, 150)
