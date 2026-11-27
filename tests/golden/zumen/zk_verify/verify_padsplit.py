# 補強板を n 枚に割ったときの期待値を、画面の計算（shared.js）と別に出す。python verify_padsplit.py
# 座標は shared.js の padPiece と同じ [u, x]。u＝補強板の板厚の中心（Rp＝Ro＋tp/2）で測った周方向の長さ（本管の天から、片寄せの向きを＋）、
# x＝本管の軸の方向（枝の中心が 0）。割る線は、枝の中心 C＝(Rp·asin(e/Ro), 0) から出る直線（展開図の上の直線＝本管の上ではつる巻きの線）
import math, sys
sys.stdout.reconfigure(encoding="utf-8")
pi, asin, sin, cos, sqrt, hypot, atan2 = math.pi, math.asin, math.sin, math.cos, math.sqrt, math.hypot, math.atan2
RHO = 7.85e-6
cl = lambda v: max(-1.0, min(1.0, v))
NG = 0

def check(name, got, want, tol):
    global NG
    ok = abs(got - want) <= tol
    NG += 0 if ok else 1
    print(f"  {'OK' if ok else 'NG'} {name}: {got:.4f}（照らす値 {want:.4f}）")

class Pad:
    def __init__(s, Dm, d, tp, Dp=None, B=None, e=0.0):
        s.Ro, s.ro, s.tp, s.e = Dm / 2, d / 2, tp, abs(e)
        s.Rp, s.Rt, s.rD, s.B = s.Ro + tp / 2, s.Ro + tp, (Dp / 2 if Dp else None), B
        s.uc = s.Rp * asin(s.e / s.Ro)

    def hole(s, th):
        return (s.Rp * asin(cl((s.e + s.ro * sin(th)) / s.Ro)), s.ro * cos(th))

    def hole_d(s, th):
        q = s.Ro ** 2 - (s.e + s.ro * sin(th)) ** 2
        return (s.Rp * s.ro * cos(th) / sqrt(max(q, 1e-12)), -s.ro * sin(th))

    def outer(s, th):
        if s.rD:
            return (s.Rp * asin(cl((s.e + s.rD * sin(th)) / s.Rt)), s.rD * cos(th))
        (u, x), (du, dx) = s.hole(th), s.hole_d(th)
        n = hypot(du, dx)
        # θ が増えると時計回りなので、外向きは進む向きの左
        return (u - s.B * dx / n, x + s.B * du / n)

    def cross(s, f, beta):
        # 曲線 f の上で、C から見た向きが beta になる θ（θ が増えると向きは減る）
        def g(th):
            u, x = f(th)
            return (atan2(x, u - s.uc) - beta + pi) % (2 * pi) - pi
        a, b = pi / 2 - beta - 0.6, pi / 2 - beta + 0.6
        assert g(a) > 0 > g(b), "割る線が曲線に1回だけ当たらない"
        for _ in range(200):
            m = (a + b) / 2
            if g(m) > 0: a = m
            else: b = m
        return (a + b) / 2

def shoelace(p):
    return abs(sum(p[i][0] * p[i + 1][1] - p[i + 1][0] * p[i][1] for i in range(len(p) - 1))) / 2

def plen(p):
    return sum(hypot(p[i + 1][0] - p[i][0], p[i + 1][1] - p[i][1]) for i in range(len(p) - 1))

def bb(p):
    us, xs = [q[0] for q in p], [q[1] for q in p]
    return max(us) - min(us), max(xs) - min(xs)

def star_ok(P, f, N=20000):
    # C から見た向きが、θ を進めると必ず減る＝どの割る線も、穴と外周に1回ずつしか当たらない
    prev, tot = None, 0.0
    for i in range(N + 1):
        u, x = f(2 * pi * i / N)
        a = atan2(x, u - P.uc)
        if prev is not None:
            d = (a - prev + pi) % (2 * pi) - pi
            if d >= 0: return False
            tot += d
        prev = a
    return abs(tot + 2 * pi) < 1e-6

def first_ray(n, along=False):
    # 継ぎ目を本管の軸から一番離す向き（二つ割は軸に直角・四つ割は軸から45°）。along は二つ割を軸に沿う線で割る場合
    if along: return pi / 2
    return pi / n if n % 2 == 0 and (n // 2) % 2 == 0 else 0.0

def pieces(P, n, along=False, N=4000):
    b0 = first_ray(n, along)
    betas = [b0 + 2 * pi * k / n for k in range(n)]
    to = [P.cross(P.outer, b) for b in betas]
    th = [P.cross(P.hole, b) for b in betas]
    out = []
    for k in range(n):
        k1 = (k + 1) % n
        oa, ob = to[k], to[k1]
        while ob > oa: ob -= 2 * pi
        ha, hb = th[k1], th[k]
        while hb < ha: hb += 2 * pi
        po = [P.outer(oa + (ob - oa) * i / N) for i in range(N + 1)]
        ph = [P.hole(ha + (hb - ha) * i / N) for i in range(N + 1)]
        out.append({"pts": po + ph + [po[0]], "hole_len": plen(ph), "outer_len": plen(po)})
    seams = [hypot(P.outer(a)[0] - P.hole(b)[0], P.outer(a)[1] - P.hole(b)[1]) for a, b in zip(to, th)]
    angs = [abs(((b % pi) / pi * 180) - 90) for b in betas]
    return out, betas, min(angs), seams

def whole(P, N=20000):
    return [P.outer(2 * pi * i / N) for i in range(N + 1)], [P.hole(2 * pi * i / N) for i in range(N + 1)]

def js_like(P, M=360):
    # shared.js の padPiece と同じ点の取り方（穴は 360 点、幅のときは前後の点で外向きを取る）
    inn = [P.hole(2 * pi * i / M) for i in range(M)]
    if P.rD:
        return [P.outer(2 * pi * i / M) for i in range(M)], inn
    a2 = sum(inn[i][0] * inn[(i + 1) % M][1] - inn[(i + 1) % M][0] * inn[i][1] for i in range(M))
    sg = 1 if a2 > 0 else -1
    out = []
    for i in range(M):
        a, c = inn[(i - 1) % M], inn[(i + 1) % M]
        tx, ty = c[0] - a[0], c[1] - a[1]
        tl = hypot(tx, ty) or 1
        out.append((inn[i][0] + sg * P.B * ty / tl, inn[i][1] - sg * P.B * tx / tl))
    return out, inn

def ray_hit(poly, C, beta):
    # 閉じた多角形 poly（点の並び・最後の点は最初の点に戻らない）と、C から beta の向きの半直線が交わる所。(辺の番号 i, 辺の上の割合 s, 点)
    dx, dy = cos(beta), sin(beta)
    best = None
    for i in range(len(poly)):
        a, b = poly[i], poly[(i + 1) % len(poly)]
        ex, ey = b[0] - a[0], b[1] - a[1]
        den = dx * (-ey) - dy * (-ex)
        if abs(den) < 1e-15: continue
        rx, ry = a[0] - C[0], a[1] - C[1]
        t = (rx * (-ey) - ry * (-ex)) / den
        s = (dx * ry - dy * rx) / den
        if t > 0 and -1e-12 <= s < 1 - 1e-12 and (best is None or t < best[3]):
            best = (i, s, (a[0] + s * ex, a[1] + s * ey), t)
    return best[:3]

def js_pieces(P, n, along=False, M=360):
    # 画面と同じ 360 点の外周と穴を、割る線との交わりの点を足して切る（片の面積の合計が、割る前の多角形の面積と同じになる）
    jo, ji = js_like(P, M)
    C = (P.uc, 0.0)
    b0 = first_ray(n, along)
    betas = [b0 + 2 * pi * k / n for k in range(n)]
    ho = [ray_hit(jo, C, b) for b in betas]
    hh = [ray_hit(ji, C, b) for b in betas]
    def walk(poly, h0, h1):
        # h0 から h1 まで、点の番号が増える向き（θ が増える＝時計回り）にたどる
        (i0, s0, p0), (i1, s1, p1) = h0, h1
        pts = [p0]
        if not (i1 == i0 and s1 >= s0):
            j = (i0 + 1) % len(poly)
            while True:
                pts.append(poly[j])
                if j == i1: break
                j = (j + 1) % len(poly)
        if s1 > 1e-12: pts.append(p1)
        return pts
    out = []
    for k in range(n):
        k1 = (k + 1) % n
        po = walk(jo, ho[k1], ho[k])[::-1]
        ph = walk(ji, hh[k1], hh[k])
        out.append(po + ph + [po[0]])
    return out

def area_strip(P, R, r):
    # 別の求め方: 展開図の面積を x ごとの幅で積分する（x＝r·cosφ と置く）。R は合わせる面の半径
    K = 4000
    f = lambda ph: (asin(cl((P.e + r * sin(ph)) / R)) - asin(cl((P.e - r * sin(ph)) / R))) * r * sin(ph)
    h = (pi / 2) / K
    s = f(0) + f(pi / 2) + sum((4 if i % 2 else 2) * f(i * h) for i in range(1, K))
    return P.Rp * s * h / 3 * 2

def report(name, P, ns):
    print(f"\n== {name} ==")
    po, ph = whole(P)
    A0 = shoelace(po) - shoelace(ph)
    w0, h0 = bb(po)
    check("C から見て星形（穴）", 1 if star_ok(P, P.hole) else 0, 1, 0)
    check("C から見て星形（外周）", 1 if star_ok(P, P.outer) else 0, 1, 0)
    jo, ji = js_like(P)
    Ajs = shoelace(jo + [jo[0]]) - shoelace(ji + [ji[0]])
    jw, jh = bb(jo + [jo[0]])
    print(f"  割る前: 外形 {w0:.2f}×{h0:.2f}  面積 {A0:.1f} mm²  重さ {A0 * P.tp * RHO:.4f} kg（画面と同じ 360 点で {Ajs * P.tp * RHO:.4f} kg・外形 {jw:.2f}×{jh:.2f}）  穴の周 {plen(ph):.2f}  外周 {plen(po):.2f}")
    if P.rD:
        check("割る前の面積（x ごとの幅の積分）", area_strip(P, P.Rt, P.rD) - area_strip(P, P.Ro, P.ro), A0, 5e-5 * A0)
    else:
        check("割る前の面積（穴の周 × 幅 ＋ π × 幅²・穴が凸のとき）", plen(ph) * P.B + pi * P.B ** 2, A0, 1e-4 * A0)
    for n, along in ns:
        ps, betas, amin, seams = pieces(P, n, along)
        tag = f"{n}枚・{'本管の軸に沿う線' if along else ('本管の軸に直角な線' if n == 2 else '枝の中心から等しい角度')}"
        print(f"  -- {tag}（割る線の向き {', '.join(f'{math.degrees(b):.1f}°' for b in betas)}・継ぎ目と本管の軸のいちばん小さい角度 {amin:.1f}°）")
        print(f"     継ぎ目の長さ（割る線の向きの順）: {', '.join(f'{x:.2f}' for x in seams)}")
        As = 0
        for k, pc in enumerate(ps):
            a = shoelace(pc["pts"]); As += a
            w, h = bb(pc["pts"])
            print(f"     片{k + 1}: 外形 {w:.2f}×{h:.2f}  面積 {a:.1f}  重さ {a * P.tp * RHO:.4f} kg  切る前の四角 {w * h * P.tp * RHO:.4f} kg  穴の部分 {pc['hole_len']:.2f}  外周の部分 {pc['outer_len']:.2f}")
        check(f"{tag} 片の重さの合計 ＝ 割る前", As * P.tp * RHO, A0 * P.tp * RHO, 1e-4)
        jp = js_pieces(P, n, along)
        jA = [shoelace(p) for p in jp]
        check(f"{tag} 画面と同じ 360 点で切った片の重さの合計 ＝ 360 点の割る前", sum(jA) * P.tp * RHO, Ajs * P.tp * RHO, 1e-9)
        jb = [bb(p) for p in jp]
        print("     画面（360 点）で出る値: " + "  ".join(f"片{k + 1} {w:.2f}×{h:.2f} {a * P.tp * RHO:.4f}kg" for k, ((w, h), a) in enumerate(zip(jb, jA)))
              + f"  外形の一番大きい幅 {max(w for w, _ in jb):.2f}・高さ {max(h for _, h in jb):.2f}  切る前の四角の合計 {sum(w * h for w, h in jb) * P.tp * RHO:.4f} kg")
        if n == 2 and not along:
            check("二つ割（軸に直角）の2枚は同じ面積（裏返しで同じ形）", shoelace(ps[0]["pts"]), shoelace(ps[1]["pts"]), 1e-3)
            check("二つ割（軸に直角）の穴の部分 ＝ 穴の周の半分", ps[0]["hole_len"], plen(ph) / 2, 0.01)
            check("二つ割（軸に直角）の外周の部分 ＝ 外周の半分", ps[0]["outer_len"], plen(po) / 2, 0.01)
            if P.B: check("二つ割（軸に直角）の継ぎ目の長さ ＝ 幅", seams[0], P.B, 1e-6)
            else: check("二つ割（軸に直角）の継ぎ目の長さ ＝ Rp(asin((e+rD)/Rt)−asin((e+ro)/Ro))", seams[0], P.Rp * (asin(cl((P.e + P.rD) / P.Rt)) - asin(cl((P.e + P.ro) / P.Ro))), 1e-6)
            # 画面の 360 点は θ＝±90° を点に持つので、その点で切れば x≥0 の半分になる
            M = len(jo); q = M // 4
            idx = list(range(3 * q, M)) + list(range(0, q + 1))
            half = [jo[i] for i in idx] + [ji[i] for i in reversed(idx)]
            half.append(half[0])
            check("画面の 360 点の継ぎ目の端は x＝0 にある", max(abs(jo[q][1]), abs(jo[3 * q][1]), abs(ji[q][1]), abs(ji[3 * q][1])), 0, 1e-9)
            check("画面の 360 点を θ＝±90° で切った1枚の重さ ＝ 割る前の半分", shoelace(half) * P.tp * RHO, Ajs / 2 * P.tp * RHO, 1e-9)
            hw, hh = bb(half)
            print(f"     画面（360 点）で出る値: 1枚の外形 {hw:.2f}×{hh:.2f}  1枚 {shoelace(half) * P.tp * RHO:.4f} kg × 2枚（2枚目は裏返し）  切る前の四角 {hw * hh * P.tp * RHO:.4f} kg × 2枚")

# 例1: 公開の計算例の T字管（E_tee_public.json）。本管 1016×12・枝 610・補強板 12・平面図の外径 1000・片寄せ 0
report("例1 本管1016・枝610・補強板12・外径1000", Pad(1016, 610, 12, Dp=1000), [(2, False), (2, True), (4, False)])
# 例2: G2_tee_claude.json の補強板。本管 1016・枝 610・補強板 12・幅 150・片寄せ 150
report("例2 本管1016・枝610・補強板12・幅150・片寄せ150", Pad(1016, 610, 12, B=150, e=150), [(2, False), (2, True), (3, False), (4, False)])
# 例3: 大径（作った例）。本管 2400A（外径 2438.4）・枝 1500A（外径 1524）・補強板 9・幅 100（大阪標準図の 2400A の t1・B）・片寄せ 0
report("例3 本管2438.4・枝1524・補強板9・幅100", Pad(2438.4, 1524, 9, B=100), [(2, False), (2, True), (4, False)])
# 例4: 外径で書いた補強板に片寄せがある（作った例）。本管 1016・枝 406.4・補強板 12・外径 700・片寄せ 150
report("例4 本管1016・枝406.4・補強板12・外径700・片寄せ150", Pad(1016, 406.4, 12, Dp=700, e=150), [(2, False), (2, True)])

print("\n== 極限の照らし ==")
# 本管がとても太い（平らな板）: 輪の面積 π(rD²−ro²)、二つ割の1枚の周 π·rD＋π·ro＋2(rD−ro)
fp = pieces(Pad(2e8, 610, 12, Dp=1000), 2)[0]
check("平らな板 二つ割の1枚の面積 ＝ π(rD²−ro²)/2", shoelace(fp[0]["pts"]), pi * (500 ** 2 - 305 ** 2) / 2, 0.5)
check("平らな板 二つ割の1枚の周の長さ ＝ π·rD＋π·ro＋2(rD−ro)", plen(fp[0]["pts"]), pi * 500 + pi * 305 + 2 * (500 - 305), 0.05)
# 板厚ほぼ0: 補強板の穴の周 ＝ 本管の穴の周 ＝ 枝の切り口の長さ（test_build.mjs の 2036.19・本管1000・枝600・片寄せ150）
Z0 = Pad(1000, 600, 0.001, B=100, e=150)
check("板厚0 補強板の穴の周（test_build の枝の切り口 2036.19）", plen(whole(Z0)[1]), 2036.19, 0.1)
check("板厚0 二つ割の1枚の穴の部分 ＝ その半分", pieces(Z0, 2)[0][0]["hole_len"], 2036.19 / 2, 0.05)
# 今の画面の値（test_build.mjs）: 割る前の外周の周方向 1328.74・軸方向 1000・穴の周方向 662.03
jo, ji = js_like(Pad(1016, 610, 12, Dp=1000))
check("割る前 外周の周方向（画面 1328.74）", bb(jo + [jo[0]])[0], 1328.74, 0.01)
check("割る前 穴の周方向（画面 662.03）", bb(ji + [ji[0]])[0], 662.03, 0.01)
# 継ぎ目にかかる応力（推論・薄肉の円筒の膜の応力）: 周方向 σh・軸方向 σh/2 のとき、本管の軸から α の継ぎ目に直角にかかる応力は σh(cos²α ＋ ½sin²α)
for a in (0, 30, 45, 90):
    r = math.radians(a)
    print(f"  継ぎ目が本管の軸から {a:>2}°: 継ぎ目に直角にかかる応力 ＝ 周方向の応力の {cos(r) ** 2 + 0.5 * sin(r) ** 2:.3f} 倍")
print(f"\nNG {NG} 件")
