# 径違いのえび曲げ（始点側と終点側で口の径が違う）の展開の期待値を、画面の計算（shared.js）と別に出す。
# python verify_redbend.py （標準ライブラリだけ。最後に NG の件数を出す）
import math, sys

sys.stdout.reconfigure(encoding="utf-8")
PI = math.pi
RHO = 7.85e-6
NG = []


def ok(name, got, want, tol):
    good = abs(got - want) <= tol
    if not good:
        NG.append(name)
    print(("OK " if good else "NG ") + f"{name}: {got:.6f}（期待 {want:.6f}・差 {got - want:+.2e}）")


def add(a, b): return (a[0] + b[0], a[1] + b[1], a[2] + b[2])
def sub(a, b): return (a[0] - b[0], a[1] - b[1], a[2] - b[2])
def mul(a, s): return (a[0] * s, a[1] * s, a[2] * s)
def dot(a, b): return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
def norm(a): return math.sqrt(dot(a, a))
def unit(a): return mul(a, 1 / norm(a))


def offs(basis, t):
    return {"outer": 0.0, "mid": t / 2, "inner": t}[basis]


# ---------- 形を組む ----------
# 曲げる面は XY。始点側の管端の中心を原点、始点側の軸を +X、曲げの中心は左（+Y）。
# 切り口の点（頂点）v=1..J は、半径 R の円に外接する正多角形の頂点（今のえび曲げと同じ）。
# 片 m=1..J+1（1＝始点側、J+1＝終点側）。片の軸 e_z、天（曲げの外側）e_x、e_y＝(0,0,1)。母線の向き ψ は天から e_y へ。
# 片は「切り口の点を中心とする球」に外接する直円すい（AWWA C208 Figure 5）。θ は片の傾き（進む向きに細くなると正）。
def model(D, De, t, R, ang, J, s1, s2, amp="outer", taper=None):
    rho = math.radians(ang) / (2 * J)
    L = 2 * R * math.tan(rho)
    ao = offs(amp, t)
    r1, rn = D / 2 - ao, De / 2 - ao
    if J < 2 and abs(D - De) > 1e-12:
        raise ValueError("切り口が1つでは中間の片が無い")
    if taper is None:
        rv = [r1 + (rn - r1) * (v - 1) / (J - 1) if J > 1 else r1 for v in range(1, J + 1)]
    else:
        if not (2 <= taper <= J):
            raise ValueError("円すいにできるのは中間の片（2〜J）")
        rv = [r1 if v <= taper - 1 else rn for v in range(1, J + 1)]
    C = [None] * (J + 2)
    C[1] = (s1 + R * math.tan(rho), 0.0, 0.0)
    for v in range(1, J):
        b = 2 * rho * v
        C[v + 1] = add(C[v], (L * math.cos(b), L * math.sin(b), 0.0))
    pieces = []
    for m in range(1, J + 2):
        b = 2 * rho * (m - 1)
        ez, ex, ey = (math.cos(b), math.sin(b), 0.0), (math.sin(b), -math.cos(b), 0.0), (0.0, 0.0, 1.0)
        if m == 1:
            ref, r, sth = C[1], rv[0], 0.0
        elif m == J + 1:
            ref, r, sth = C[J], rv[J - 1], 0.0
        else:
            ref, r, sth = C[m - 1], rv[m - 2], (rv[m - 2] - rv[m - 1]) / L
        if abs(sth) >= 1:
            raise ValueError("円すいの傾きが 90° を超える")
        th = math.asin(sth)
        pieces.append(dict(m=m, ez=ez, ex=ex, ey=ey, ref=ref, r=r, th=th, rho0=r / math.cos(th)))
    joints = []
    for v in range(1, J + 1):
        A, B = pieces[v - 1], pieces[v]
        Nv = add(mul(A["ez"], 1 / math.cos(A["th"])), mul(B["ez"], 1 / math.cos(B["th"])))
        d = rv[v - 1] * (math.tan(A["th"]) + math.tan(B["th"]))
        nn = norm(Nv)
        joints.append(dict(v=v, C=C[v], n=mul(Nv, 1 / nn), d=d / nn, r=rv[v - 1]))
    for m, p in enumerate(pieces, start=1):
        if m == 1:
            p["lo"] = (p["ez"], sub(C[1], mul(p["ez"], s1 + R * math.tan(rho))))
        else:
            j = joints[m - 2]
            p["lo"] = (j["n"], add(j["C"], mul(j["n"], j["d"])))
        if m == J + 1:
            p["hi"] = (p["ez"], add(C[J], mul(p["ez"], R * math.tan(rho) + s2)))
        else:
            j = joints[m - 1]
            p["hi"] = (j["n"], add(j["C"], mul(j["n"], j["d"])))
    return dict(rho=rho, L=L, ao=ao, rv=rv, C=C, pieces=pieces, joints=joints, J=J, R=R, t=t, D=D, De=De, s1=s1, s2=s2)


def uvec(p, psi):
    return add(mul(p["ex"], math.cos(psi)), mul(p["ey"], math.sin(psi)))


def zcut(p, plane, psi):
    n, P0 = plane
    u = uvec(p, psi)
    den = dot(n, p["ez"]) - math.tan(p["th"]) * dot(n, u)
    if den <= 0:
        raise ValueError("切る面が母線と交わらない")
    return (dot(n, sub(P0, p["ref"])) - p["rho0"] * dot(n, u)) / den


def zaxis(p, plane):
    n, P0 = plane
    return dot(n, sub(P0, p["ref"])) / dot(n, p["ez"])


def pt3(p, z, psi, delta=0.0):
    return add(add(p["ref"], mul(p["ez"], z)), mul(uvec(p, psi), p["rho0"] - delta - z * math.tan(p["th"])))


def outer_surface_err(p, P):
    w = sub(P, p["ref"])
    z = dot(w, p["ez"])
    perp = norm(sub(w, mul(p["ez"], z)))
    return perp - (p["rho0"] - z * math.tan(p["th"]))


# ---------- 展開（板厚の基準の面。切り口の高さは振れ幅の面で決め、同じ軸の位置で基準の面に移す） ----------
def develop(p, t, basis, amp, allow, ph, n=1, M=360, j=0):
    delta = offs(basis, t) - offs(amp, t)
    th, h = p["th"], allow / 2
    zlo_ax = zaxis(p, p["lo"])
    a0, a1 = ph + 2 * PI * j / n, ph + 2 * PI * (j + 1) / n
    m = M if n == 1 else math.ceil(720 / n)
    rB0 = p["rho0"] - delta
    s, c = math.sin(th), math.cos(th)

    def xy(z, psi):
        rB = rB0 - z * math.tan(th)
        if abs(s) < 1e-15:
            return (rB * (psi - a0), z - zlo_ax)
        za = rB0 / math.tan(th)
        ell, al = rB / abs(s), (psi - (a0 + a1) / 2) * abs(s)
        return (ell * math.sin(al), (za - zlo_ax) / c - math.copysign(1, th) * ell * math.cos(al))

    lo, up = [], []
    for i in range(m + 1):
        psi = a0 + (a1 - a0) * i / m
        lo.append(xy(zcut(p, p["lo"], psi) - h * c, psi))
        up.append(xy(zcut(p, p["hi"], psi) + h * c, psi))
    pts = lo + up[::-1] + [lo[0]]
    return pts, lo, up


def shoelace(pts):
    return abs(sum(pts[i][0] * pts[i + 1][1] - pts[i + 1][0] * pts[i][1] for i in range(len(pts) - 1))) / 2


def bbox(pts):
    xs, ys = [q[0] for q in pts], [q[1] for q in pts]
    return max(xs) - min(xs), max(ys) - min(ys)


def plen(ps):
    return sum(math.dist(ps[i], ps[i + 1]) for i in range(len(ps) - 1))


def gen_len(p, psi, allow):
    return (zcut(p, p["hi"], psi) - zcut(p, p["lo"], psi)) / math.cos(p["th"]) + allow


def area3d(p, t, basis, amp, N=20000):
    delta = offs(basis, t) - offs(amp, t)
    th = p["th"]
    rB0 = p["rho0"] - delta
    F = lambda z: (rB0 * z - z * z * math.tan(th) / 2) / math.cos(th)
    tot = 0.0
    for i in range(N):
        psi = 2 * PI * i / N
        tot += F(zcut(p, p["hi"], psi)) - F(zcut(p, p["lo"], psi))
    return tot * 2 * PI / N


def ph_of(seam_from="地", seam_angle=45):
    return math.radians(seam_angle if seam_from == "天" else 180 - seam_angle)


# ---------- 1片の数をまとめて出す ----------
def piece_report(G, p, basis="mid", amp="outer", allow=4, ph=None, fine=20000):
    ph = ph_of() if ph is None else ph
    t = G["t"]
    pts, lo, up = develop(p, t, basis, amp, allow, ph, 1, 360)
    pts0, _, _ = develop(p, t, basis, amp, 0, ph, 1, 360)
    ptsF, _, _ = develop(p, t, basis, amp, 0, ph, 1, fine)
    w, hh = bbox(pts)
    zl, zh = zaxis(p, p["lo"]), zaxis(p, p["hi"])
    ao = offs(amp, t)
    rA = lambda z: p["rho0"] - z * math.tan(p["th"])
    return dict(
        kind="円筒" if abs(p["th"]) < 1e-15 else "円すい", th=math.degrees(p["th"]),
        axis=zh - zl, w=w, h=hh, area0=shoelace(pts0), areaF=shoelace(ptsF),
        kg0=shoelace(pts0) * t * RHO, kgF=shoelace(ptsF) * t * RHO, kgA=shoelace(pts) * t * RHO,
        ten=gen_len(p, 0, allow), chi=gen_len(p, PI, allow), yo=gen_len(p, PI / 2, allow), yo2=gen_len(p, -PI / 2, allow),
        Dlo=2 * (rA(zl) + ao), Dhi=2 * (rA(zh) + ao), seam=gen_len(p, ph, allow))


def print_piece(name, q):
    print(f"  {name}: {q['kind']} θ={q['th']:.4f}°  外形 {q['w']:.2f}×{q['h']:.2f}（余長込み）  軸の長さ {q['axis']:.2f}（余長なし）"
          f"  母線の長さ 天 {q['ten']:.2f}・地 {q['chi']:.2f}・ヨ {q['yo']:.2f}（余長込み）  口の外径（軸に直角）{q['Dlo']:.2f}→{q['Dhi']:.2f}"
          f"  正味 {q['kg0']:.3f} kg（{q['kgF']:.3f}）・余長込み {q['kgA']:.3f} kg  継ぎ目の縁 {q['seam']:.2f}")


def joint_report(G, j, A, B):
    n = j["n"]
    phA = math.degrees(math.acos(max(-1, min(1, dot(n, A["ez"])))))
    phB = math.degrees(math.acos(max(-1, min(1, dot(n, B["ez"])))))
    ring = [pt3(A, zcut(A, A["hi"], 2 * PI * i / 4000), 2 * PI * i / 4000) for i in range(4001)]
    return dict(phA=phA, phB=phB, off=j["d"], weld=plen(ring), ring=ring)


# ---------- 今のえび曲げ（bendPiece）の式 ----------
def plain_bend(D, t, R, ang, J, cl, lo, up, allow=4, basis="mid", amp="outer", n=1, ph=None):
    ph = ph_of() if ph is None else ph
    o, ao = offs(basis, t), offs(amp, t)
    r, gam = D / 2 - o, math.radians(ang / J / 2)
    k = (D / 2 - ao) * math.tan(gam)
    circ, h = 2 * PI * r, allow / 2
    out = []
    m = 360 if n == 1 else math.ceil(720 / n)
    for jj in range(n):
        x0, l2, u2 = circ * jj / n, [], []
        for q in range(m + 1):
            xx = x0 + circ / n * q / m
            cc = math.cos(2 * PI * xx / circ + ph)
            l2.append((xx - x0, -h - lo * k * cc))
            u2.append((xx - x0, cl + h + up * k * cc))
        out.append(l2 + u2[::-1] + [l2[0]])
    return dict(k=k, circ=circ, center=cl + allow, long=cl + allow + (up + lo) * k, short=cl + allow - (up + lo) * k, bands=out)


# ====================================================================
print("=== 1. 径が同じなら今のえび曲げと同じ（極限） ===")

# 2600A（test_build.mjs の帯）
D, t, R, ang, J = 2641.6, 16, 2600, 90, 3
G = model(D, D, t, R, ang, J, 300, 300)
half = R * math.tan(G["rho"])
pm, ps = G["pieces"][1], G["pieces"][0]
# 面積は shared.js の bendPiece を node で回した値（test_build.mjs の期待値は 0.05% の幅で丸めてある）
want = {("中間", 2): [(4124.283, 1897.836, 4448902.301, 896.835), (4124.283, 2105.150, 7077113.816, 1897.836)],
        ("中間", 3): [(2749.522, 1214.140, 2287401.569, 896.835), (2749.522, 2105.150, 4980054.342, 1214.140), (2749.522, 2081.032, 4258560.206, 2081.032)],
        ("始点側", 2): [(4124.283, 1250.918, None, 750.418), (4124.283, 1354.575, None, 1250.918)],
        ("始点側", 3): [(2749.522, 909.070, None, 750.418), (2749.522, 1354.575, None, 909.070), (2749.522, 1342.516, None, 1342.516)]}
for (nm, n), rows in want.items():
    p = pm if nm == "中間" else ps
    for jj, (w, hh, ar, left) in enumerate(rows):
        pts, lo, up = develop(p, t, "mid", "outer", 4, ph_of(), n, 360, jj)
        bw, bh = bbox(pts)
        ok(f"2600A {nm} 継ぎ目{n}本 帯{jj + 1} 幅", bw, w, 0.001)
        ok(f"2600A {nm} 継ぎ目{n}本 帯{jj + 1} 高さ", bh, hh, 0.001)
        ok(f"2600A {nm} 継ぎ目{n}本 帯{jj + 1} 左の縁", up[0][1] - lo[0][1], left, 0.001)
        if ar is not None:
            ok(f"2600A {nm} 継ぎ目{n}本 帯{jj + 1} 面積 ＝ shared.js", shoelace(pts), ar, 0.01)
ok("2600A 中間 振れ幅", (gen_len(pm, 0, 0) - gen_len(pm, PI, 0)) / 4, 353.91, 0.005)

# 径の差をごく小さくすると、今の形に連続してつながる
Gs = model(2641.6, 2641.6 - 1e-6, 16, 2600, 90, 3, 300, 300)
qs, qp = piece_report(Gs, Gs["pieces"][1]), piece_report(G, G["pieces"][1])
ok("径の差 1e-6 mm の中間 天の長さ ≒ 同じ径", qs["ten"], qp["ten"], 1e-5)
ok("径の差 1e-6 mm の中間 正味の重さ ≒ 同じ径", qs["kg0"], qp["kg0"], 1e-4)


# ====================================================================
def full_case(title, D, De, t, R, ang, J, s1, s2, taper=None, splits=()):
    print()
    print(f"=== {title} ===")
    G = model(D, De, t, R, ang, J, s1, s2, taper=taper)
    rho = G["rho"]
    print(f"  ρ（切り口1つの曲がりの半分）={math.degrees(rho):.4f}°  切り口の点の間 2R·tanρ={G['L']:.3f}  球の半径（外面）{', '.join(f'{r:.3f}' for r in G['rv'])}")
    tot_kg, tot_axis = 0.0, 0.0
    reps = []
    for p in G["pieces"]:
        q = piece_report(G, p)
        reps.append(q)
        nm = "始点側" if p["m"] == 1 else "終点側" if p["m"] == J + 1 else f"中間{p['m'] - 1}"
        print_piece(f"片{p['m']}（{nm}）", q)
        tot_kg += q["kg0"]
        tot_axis += q["axis"]
        if q["chi"] - 4 <= 0:
            NG.append(title + " 地で切り口が交わる")
    poly = s1 + s2 + 2 * J * R * math.tan(rho)
    print(f"  合計: 正味 {tot_kg:.3f} kg・軸の長さの合計 {tot_axis:.3f}・中心線（切り口の点を結ぶ線）の長さ {poly:.3f}（差 {tot_axis - poly:+.3f}）")
    print(f"  同じ角度・同じ長さの今のえび曲げ: 始点側の径で {PI * (D - t) * t * RHO * poly:.3f} kg、終点側の径で {PI * (De - t) * t * RHO * poly:.3f} kg")
    for j in G["joints"]:
        A, B = G["pieces"][j["v"] - 1], G["pieces"][j["v"]]
        jr = joint_report(G, j, A, B)
        print(f"  切り口{j['v']}: 面の法線と前の片の軸 {jr['phA']:.4f}°・後の片の軸 {jr['phB']:.4f}°、面と球の中心の距離 {jr['off']:.3f}、溶接線（外面の楕円）の長さ {jr['weld']:.3f}")
    return G, reps


def checks(title, G):
    J, rho, R = G["J"], G["rho"], G["R"]
    P = G["pieces"]
    # AWWA C208 Figure 5 の式（中間の片が全部同じ傾きのとき）
    th = P[1]["th"]
    r1, rn = G["rv"][0], G["rv"][-1]
    if J >= 2 and all(abs(P[m]["th"] - th) < 1e-12 for m in range(1, J)):
        ok(f"{title} sinθ ＝ (D1−Dn)/(2(n−2)R·tanρ)", math.sin(th), (2 * r1 - 2 * rn) / (2 * (2 * J - 2) * R * math.tan(rho)), 1e-12)
        ok(f"{title} Z1（始点側の軸で球の中心から切り口まで）", zaxis(P[0], P[0]["hi"]) - 0, r1 * math.sin(th) / (math.cos(2 * rho) + math.cos(th)), 1e-9)
        ok(f"{title} Zn（終点側の軸で球の中心から切り口まで）", zaxis(P[J], P[J]["lo"]), rn * math.sin(th) / (math.cos(2 * rho) + math.cos(th)), 1e-9)
        nA = G["joints"][0]["n"]
        ok(f"{title} φ（始点側の切り口の面の傾き）", math.degrees(math.acos(dot(nA, P[0]["ez"]))), math.degrees(math.atan(math.sin(2 * rho) / (math.cos(2 * rho) + math.cos(th)))), 1e-9)
        for j in G["joints"][1:-1]:
            ok(f"{title} Y{2 * j['v'] - 1}（切り口{j['v']}の面と球の中心の距離）", j["d"], j["r"] * math.sin(th) / math.cos(rho), 1e-9)
        for m in range(2, J + 1):
            x = 2 * (m - 1)
            Dx = 2 * (r1 - (x - 1) * R * math.tan(rho) * math.sin(th)) / math.cos(th)
            p = P[m - 1]
            ok(f"{title} D{x}（中間{m - 1}の真ん中の径・軸に直角）", 2 * (p["rho0"] - G["L"] / 2 * math.tan(p["th"])), Dx, 1e-9)
    # 画面に入れる閉じた式（spec の 4 章）と、3D で面を切った値が同じ
    L, worst = G["L"], 0.0
    for j in G["joints"]:
        A, B = P[j["v"] - 1], P[j["v"]]
        cA, cB = math.cos(A["th"]), math.cos(B["th"])
        phA = math.atan2(math.sin(2 * rho) * cA, cB + cA * math.cos(2 * rho))
        nn = math.sqrt(1 / cA ** 2 + 1 / cB ** 2 + 2 * math.cos(2 * rho) / (cA * cB))
        d = j["r"] * (math.tan(A["th"]) + math.tan(B["th"])) / nn
        j["f"] = dict(phA=phA, phB=2 * rho - phA, d=d)
        worst = max(worst, abs(d - j["d"]), abs(phA - math.acos(dot(j["n"], A["ez"]))))
    for m, p in enumerate(P, start=1):
        th, r0 = p["th"], p["rho0"]
        if m == 1:
            zlo, zhi, plo, phi_ = -(G["s1"] + R * math.tan(rho)), G["joints"][0]["f"]["d"] / math.cos(G["joints"][0]["f"]["phA"]), 0.0, G["joints"][0]["f"]["phA"]
        elif m == J + 1:
            f = G["joints"][J - 1]["f"]
            zlo, zhi, plo, phi_ = f["d"] / math.cos(f["phB"]), R * math.tan(rho) + G["s2"], f["phB"], 0.0
        else:
            f0, f1 = G["joints"][m - 2]["f"], G["joints"][m - 1]["f"]
            zlo, zhi, plo, phi_ = f0["d"] / math.cos(f0["phB"]), L + f1["d"] / math.cos(f1["phA"]), f0["phB"], f1["phA"]
        for i in range(360):
            psi = 2 * PI * i / 360
            c = math.cos(psi)
            zl = (zlo - r0 * math.tan(plo) * c) / (1 - math.tan(th) * math.tan(plo) * c)
            zu = (zhi + r0 * math.tan(phi_) * c) / (1 + math.tan(th) * math.tan(phi_) * c)
            worst = max(worst, abs(zl - zcut(p, p["lo"], psi)), abs(zu - zcut(p, p["hi"], psi)))
    ok(f"{title} 閉じた式（φ・面の距離・切り口の高さ）＝ 3D で切った値（最大の差）", worst, 0, 1e-8)
    # 切り口で前後の片の外面が同じ楕円になる（同じ球に外接する2つの円すいは平面の楕円で交わる）
    worst = 0.0
    for j in G["joints"]:
        A, B = P[j["v"] - 1], P[j["v"]]
        for i in range(720):
            psi = 2 * PI * i / 720
            worst = max(worst, abs(outer_surface_err(B, pt3(A, zcut(A, A["hi"], psi), psi))), abs(outer_surface_err(A, pt3(B, zcut(B, B["lo"], psi), psi))))
    ok(f"{title} 切り口で前後の片の外面が重なる（最大のずれ mm）", worst, 0, 1e-8)
    # 総当たり: 前の片の母線に沿って後の片の面との交わりを探し（球の式を使わない）、面の式の位置と比べる
    worst = 0.0
    for j in G["joints"]:
        A, B = P[j["v"] - 1], P[j["v"]]
        for i in range(180):
            psi = 2 * PI * i / 180
            zp = zcut(A, A["hi"], psi)
            f = lambda z: outer_surface_err(B, pt3(A, z, psi))
            lo_, hi_ = zp - 0.4 * G["L"], zp + 0.4 * G["L"]
            zs = [lo_ + (hi_ - lo_) * k / 4000 for k in range(4001)]
            roots = []
            for a, b in zip(zs, zs[1:]):
                fa, fb = f(a), f(b)
                if fa == 0 or fa * fb < 0:
                    for _ in range(80):
                        c = (a + b) / 2
                        if f(a) * f(c) <= 0:
                            b = c
                        else:
                            a = c
                    roots.append((a + b) / 2)
            best = min(roots, key=lambda z: abs(z - zp))
            worst = max(worst, abs(best - zp))
    ok(f"{title} 総当たりで探した交わり ＝ 面の式（最大の差 mm）", worst, 0, 1e-6)
    # 展開は長さを変えない: 板の上の切り口の長さ ＝ 基準の面の上の 3D の長さ。面積 ＝ 3D の面の積分
    for p in P:
        nm = f"片{p['m']}"
        pts, lo, up = develop(p, G["t"], "mid", "outer", 0, ph_of(), 1, 20000)
        delta = G["t"] / 2
        L2 = plen(lo)
        L3 = plen([pt3(p, zcut(p, p["lo"], ph_of() + 2 * PI * i / 20000), ph_of() + 2 * PI * i / 20000, delta) for i in range(20001)])
        ok(f"{title} {nm} 下の切り口の長さ（板の上 ＝ 3D）", L2, L3, 1e-5 * L3)
        ok(f"{title} {nm} 面積（板の上 ＝ 3D の面の積分）", shoelace(pts), area3d(p, G["t"], "mid", "outer"), 1e-6 * shoelace(pts))


def split_check(title, G, m, n):
    p = G["pieces"][m - 1]
    whole = shoelace(develop(p, G["t"], "mid", "outer", 4, ph_of(), 1, 20000)[0])
    tot, lefts, rights = 0.0, [], []
    for jj in range(n):
        pts, lo, up = develop(p, G["t"], "mid", "outer", 4, ph_of(), n, 360, jj)
        ptsF, loF, upF = develop(p, G["t"], "mid", "outer", 4, ph_of(), n, 20000 // n, jj)
        w, hh = bbox(pts)
        tot += shoelace(ptsF)
        left, right = math.dist(up[0], lo[0]), math.dist(up[-1], lo[-1])
        lefts.append(left)
        rights.append(right)
        print(f"  片{m} 継ぎ目{n}本 帯{jj + 1}: 外形 {w:.2f}×{hh:.2f}・面積 {shoelace(pts):.0f}（{shoelace(ptsF):.0f}）・左の縁 {left:.3f}・右の縁 {right:.3f}")
    ok(f"{title} 片{m} 継ぎ目{n}本 帯の面積の合計 ＝ 割る前（相対）", tot / whole, 1, 1e-7)
    for jj in range(n):
        ok(f"{title} 片{m} 継ぎ目{n}本 帯{jj + 1} の右の縁 ＝ 次の帯の左の縁", rights[jj], lefts[(jj + 1) % n], 1e-9)


G2, R2 = full_case("例1 2600A×2400A・90°・切り口3（中間の片2つを円すい）・板厚16・R2600・直管 300/300", 2641.6, 2438.4, 16, 2600, 90, 3, 300, 300)
checks("例1", G2)
split_check("例1", G2, 2, 2)
split_check("例1", G2, 3, 3)

G3, R3 = full_case("例2 1600A×1350A・90°・切り口4（中間の片3つを円すい）・板厚12・R2400・直管 400/400", 1625.6, 1371.6, 12, 2400, 90, 4, 400, 400)
checks("例2", G3)
G3b, R3b = full_case("例2b 同じ寸法で、真ん中の片（片3）だけを円すい", 1625.6, 1371.6, 12, 2400, 90, 4, 400, 400, taper=3)
checks("例2b", G3b)
print(f"  例2b − 例2 の正味の重さ {sum(q['kg0'] for q in R3b) - sum(q['kg0'] for q in R3):+.3f} kg")

print()
print("=== 板厚の基準を変えても、展開は長さと面積を変えない（例2・振れ幅は中心・周は内側） ===")
Gm = model(1625.6, 1371.6, 12, 2400, 90, 4, 400, 400, amp="mid")
for p in Gm["pieces"]:
    pts, lo, up = develop(p, 12, "inner", "mid", 0, ph_of(), 1, 20000)
    L3 = plen([pt3(p, zcut(p, p["hi"], ph_of() + 2 * PI * i / 20000), ph_of() + 2 * PI * i / 20000, 6) for i in range(20001)])
    ok(f"片{p['m']} 上の切り口の長さ（板の上 ＝ 3D）", plen(up), L3, 1e-5 * L3)
    ok(f"片{p['m']} 面積（板の上 ＝ 3D の面の積分）", shoelace(pts), area3d(p, 12, "inner", "mid"), 1e-6 * shoelace(pts))
print("  振れ幅を中心で取ったときの 例2 の切り口の面と球の中心の距離: " + "・".join(f"{j['d']:.3f}" for j in Gm["joints"]))

G4, R4 = full_case("例3 1500A×1600A・45°・切り口2（太くなる向き・中間の片1つ）・板厚12・R1500・直管 500/400", 1524.0, 1625.6, 12, 1500, 45, 2, 500, 400)
checks("例3", G4)
split_check("例3", G4, 2, 2)

print()
print("=== 2. 作れない寸法 ===")
for args, why in [((1625.6, 1371.6, 12, 2400, 30, 1, 400, 400), "切り口1つ"),
                  ((1625.6, 600.0, 12, 600, 30, 2, 400, 400), "径の差が大きく片が短い"),
                  ((1625.6, 1371.6, 12, 800, 90, 3, 300, 300), "R が小さい（1600A×1350A・R800・90°・切り口3）"),
                  ((1625.6, 1625.6, 12, 800, 90, 3, 300, 300), "同じ径でも R が小さい（今のえび曲げ）")]:
    try:
        Gx = model(*args)
        chi = [gen_len(p, PI, 0) for p in Gx["pieces"]]
        bad = [i + 1 for i, c in enumerate(chi) if c <= 0]
        print(f"  {why}: 地の母線の長さ（余長なし）" + "・".join(f"{c:.1f}" for c in chi) + f"・傾き {max(math.degrees(abs(p['th'])) for p in Gx['pieces']):.2f}°"
              + (f" → 片{'・'.join(map(str, bad))} は地で切り口が交わるので作れない" if bad else " → 作れる"))
    except ValueError as e:
        print(f"  {why}: 作れない（{e}）")

print()
print("=== 3. 傾きの目安（AWWA C208 4.1.11 片落管の最小の長さ Lr＝4(DL−DS) に当たる半頂角） ===")
print(f"  atan(1/8) ＝ {math.degrees(math.atan(1 / 8)):.3f}°")

print()
print("合わない所 " + str(len(NG)) + " 件" + ("：" + "・".join(NG) if NG else "（全部合った）"))
sys.exit(1 if NG else 0)
