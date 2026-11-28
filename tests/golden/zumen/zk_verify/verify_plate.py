# 平板（四角・台形・三角・面取り・丸穴）と、T字管のリブの切図の期待値を、画面の計算（shared.js）と別に出す。
#   python verify_plate.py
# 座標は shared.js と同じ: 片ごとに左下の角を原点、x は右、y は上、mm。外形は閉じた点の並び（最初と最後が同じ点）。
# 重さは 面積 × 板厚 × 7.85e-6（kg/mm^3）。穴の面積は引く。
import math, sys, unicodedata, re
sys.stdout.reconfigure(encoding="utf-8")
PI, RHO = math.pi, 7.85e-6

def shoelace(p):  # 閉じた点の並びの面積（向きを問わない）
    return abs(sum(p[i][0] * p[i + 1][1] - p[i + 1][0] * p[i][1] for i in range(len(p) - 1))) / 2

def plen(p):
    return sum(math.hypot(p[i + 1][0] - p[i][0], p[i + 1][1] - p[i][1]) for i in range(len(p) - 1))

def seg_dist(q, a, b):
    ax, ay, bx, by = a[0], a[1], b[0], b[1]
    L2 = (bx - ax) ** 2 + (by - ay) ** 2
    s = 0 if L2 == 0 else max(0, min(1, ((q[0] - ax) * (bx - ax) + (q[1] - ay) * (by - ay)) / L2))
    return math.hypot(q[0] - ax - s * (bx - ax), q[1] - ay - s * (by - ay))

def inside(q, p):
    c = False
    for i in range(len(p) - 1):
        (x1, y1), (x2, y2) = p[i], p[i + 1]
        if (y1 > q[1]) != (y2 > q[1]) and q[0] < (x2 - x1) * (q[1] - y1) / (y2 - y1) + x1: c = not c
    return c

# ---------- 平板の外形 ----------
# 角の番号は左下から左回り: 1 左下・2 右下・3 右上（三角は頂点）・4 左上
def corners(W, H, W2=None, off=0.0):
    if W2 is None: W2, off = W, 0.0                       # 四角
    if W2 == 0: return [(0.0, 0.0), (W, 0.0), (off, H)]   # 三角（off=0 で左下が直角）
    return [(0.0, 0.0), (W, 0.0), (off + W2, H), (off, H)]  # 台形

def chamfer(cs, c, at):
    # C 面取り: 角から両側の辺に沿って c ずつ下がった2点を結ぶ。at=0 は全部の角、1〜4 はその角だけ
    if not c: return cs
    n, out = len(cs), []
    for i, P in enumerate(cs):
        if at and at != i + 1: out.append(P); continue
        A, B = cs[i - 1], cs[(i + 1) % n]
        ua = (A[0] - P[0], A[1] - P[1]); la = math.hypot(*ua)
        ub = (B[0] - P[0], B[1] - P[1]); lb = math.hypot(*ub)
        out.append((P[0] + c * ua[0] / la, P[1] + c * ua[1] / la))
        out.append((P[0] + c * ub[0] / lb, P[1] + c * ub[1] / lb))
    return out

def holes(d, x, y, nx=1, ny=1, px=0.0, py=0.0, M=360):
    cs = [(x + i * px, y + j * py) for j in range(ny) for i in range(nx)]
    r = d / 2
    return cs, [[(cx + r * math.cos(2 * PI * k / M), cy + r * math.sin(2 * PI * k / M)) for k in range(M)] for cx, cy in cs]

def plate(W, H, t, W2=None, off=0.0, C=None, C_at=0, hole=None, n=1):
    cs = corners(W, H, W2, off)
    pts = chamfer(cs, C, C_at); pts = pts + [pts[0]]
    hc, hp = holes(**hole) if hole else ([], [])
    area = shoelace(pts) - sum(shoelace(h + [h[0]]) for h in hp)
    # 別の求め方: 台形の式から、面取りで落ちる三角と、穴の円を引く
    W2v = W if W2 is None else W2
    m = len(cs); ang = []
    for i, P in enumerate(cs):
        A, B = cs[i - 1], cs[(i + 1) % m]
        ua, ub = (A[0] - P[0], A[1] - P[1]), (B[0] - P[0], B[1] - P[1])
        ang.append(math.acos((ua[0] * ub[0] + ua[1] * ub[1]) / math.hypot(*ua) / math.hypot(*ub)))
    sel = [i for i in range(m) if C and (not C_at or C_at == i + 1)]
    a_exact = (W + W2v) / 2 * H - sum(C * C * math.sin(ang[i]) / 2 for i in sel) - len(hc) * PI * (hole["d"] / 2) ** 2 if hole else (W + W2v) / 2 * H - sum(C * C * math.sin(ang[i]) / 2 for i in sel)
    per0 = plen(cs + [cs[0]])
    per_exact = per0 - sum(2 * C for i in sel) + sum(2 * C * math.sin(ang[i] / 2) for i in sel)
    # 穴が外形の内側に収まるか（縁までの距離が半径以上）
    ok = all(inside(q, pts) and min(seg_dist(q, pts[k], pts[k + 1]) for k in range(len(pts) - 1)) >= hole["d"] / 2 for q in hc) if hole else True
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return dict(pts=pts, area=area, area_exact=a_exact, per=plen(pts), per_exact=per_exact,
                kg=area * t * RHO * n, kg_exact=a_exact * t * RHO * n,
                gross=(max(c[0] for c in cs) - min(c[0] for c in cs)) * (max(c[1] for c in cs) - min(c[1] for c in cs)) * t * RHO * n,  # 外形を囲む四角
                w=max(xs) - min(xs), h=max(ys) - min(ys), holes_ok=ok, centers=hc, angles=[math.degrees(a) for a in ang])

# ---------- 材料表の「PL-12×300×80」 ----------
def parse_pl(size, length="", t_known=None):
    s = unicodedata.normalize("NFKC", size or "").upper().replace("Ø", "Φ")
    if "Φ" in s or "φ" in s: return None          # 円板・輪（今の ring で出す）
    ns = [float(v) for v in re.findall(r"\d+(?:\.\d+)?", s)]
    ln = re.findall(r"\d+(?:\.\d+)?", unicodedata.normalize("NFKC", length or ""))
    m = re.search(r"(\d+(?:\.\d+)?)\s*T\b", s)       # 「12t×300×80」の t の付いた数は板厚
    if m: t = float(m.group(1)); ns.remove(t)
    elif t_known is not None and t_known in ns: t = t_known; ns.remove(t)
    elif ns: t = ns.pop(0)                           # 「PL-板厚×幅×長さ」の順
    else: return None
    if len(ns) == 1 and ln: ns.append(float(ln[0]))  # 「PL-16×800」と長さの欄「2700」
    return (t, ns[0] if ns else None, ns[1] if len(ns) > 1 else None)

# ---------- T字管のリブ（本管と枝の付け根に立てる板） ----------
# 本管の軸に直角な面に立つリブは、本管に当たる辺が本管の外面の円（半径 Ro）に沿う。
# 片の座標: 原点 A ＝ 底の辺（枝の外面）が本管の外面に当たる点。x は枝に沿って外へ、y はリブの高さ。
# vc は、底の辺から本管の中心までの、高さの向きの距離（リブのある側が正）。枝の片寄せ e・枝の外半径 ro で
#   本管の中心の側に立つリブ: vc = e - ro　（排水T字管の上のリブ: e = Ro - ro なので vc = Ro - d）
#   中心に立つ枝（e = 0）のリブ: vc = -ro
def rib_arc(Ro, vc, b, h, top=0.0, N=360):
    u0 = math.sqrt(Ro * Ro - vc * vc)
    xa = lambda v: math.sqrt(Ro * Ro - (v - vc) ** 2) - u0   # 本管の外面の、高さ v での x
    xC = xa(h)
    arc = [(xa(h - h * k / N), h - h * k / N) for k in range(N + 1)]  # C から A へ
    pts = [(0.0, 0.0), (b, 0.0), (xC + top, h)] + arc
    if top == 0: pts = [(0.0, 0.0), (b, 0.0)] + arc
    # 別の求め方: 外の辺（B→C'）より内側の面積から、本管の円より内側の分を、円の式の積分で引く
    F = lambda w: (w * math.sqrt(Ro * Ro - w * w) + Ro * Ro * math.asin(w / Ro)) / 2
    # x_line(v): B(b,0) → C'(xC+top, h) の直線
    line = (b + (xC + top)) / 2 * h
    arcint = (F(h - vc) - F(-vc)) - u0 * h                     # ∫0..h xa(v) dv
    return dict(pts=pts, area=shoelace(pts), area_exact=line - arcint, u0=u0, xC=xC)

def cut_y(Rc, ri, ro, e, th):  # verify_tee.py と同じ（枝の壁が本管の面に当たる一番高い点）
    a, b = e + ri * math.sin(th), e + ro * math.sin(th)
    m = 0.0 if a * b <= 0 else min(abs(a), abs(b))
    return math.sqrt(Rc * Rc - m * m)

def show(name, v, d=2): print(f"  {name}: {v:.{d}f}")

# ======================================================================
print("== 例1 四角（材料表の PL-12×300×80 から）・四隅 C10 ==")
for size, ln in [("PL-12×300×80", ""), ("ＰＬ－１２×３００×８０", ""), ("12t×300×80", ""), ("PL-16×800", "2700"), ("PL-9×75", ""), ("PL-19×φ1900", "φ1600")]:
    print(f"  「{size}」「{ln}」 → 板厚・幅・長さ {parse_pl(size, ln)}")
t, W, H = parse_pl("PL-12×300×80")
p0 = plate(W, H, t)
show("四角 面積 mm2", p0["area"], 1); show("四角 重さ kg（1枚）", p0["kg"], 4)
p1 = plate(W, H, t, C=10, C_at=0)
show("C10 四隅 面積（点の並び）", p1["area"], 2); show("C10 四隅 面積（式: 24000 − 4×10²/2）", p1["area_exact"], 2)
show("C10 四隅 周の長さ（点の並び）", p1["per"], 3); show("C10 四隅 周の長さ（式: 760 − 8×10 + 4×10√2）", p1["per_exact"], 3)
show("C10 四隅 重さ kg（切った後）", p1["kg"], 4); show("切る前の四角 kg", p1["gross"], 4)

print("\n== 例2 台形と三角（直角の当て板）・丸穴 ==")
pt = plate(300, 200, 9, W2=150, off=0, hole=dict(d=22, x=60, y=50))
show("台形 下300・上150・高さ200（左が直角） 外形の幅", pt["w"], 1); show("外形の高さ", pt["h"], 1)
show("面積（点の並び・穴φ22を360角で引く）", pt["area"], 2); show("面積（式: (300+150)/2×200 − π×11²）", pt["area_exact"], 2)
show("重さ kg", pt["kg"], 4); show("切る前の四角 kg（300×200）", pt["gross"], 4)
print("  穴が外形の内側に収まる:", pt["holes_ok"], " 角の角度:", [round(a, 3) for a in pt["angles"]])
ps = plate(300, 200, 9, W2=150, off=75)
show("左右対称（off=75）の面積（左が直角と同じ）", ps["area"], 2)
print("  左右対称の角の角度:", [round(a, 3) for a in ps["angles"]])
pr = plate(200, 150, 9, W2=0, off=0)
show("三角 底200・高さ150（左下が直角） 面積", pr["area"], 2); show("三角 斜めの辺", math.hypot(200, 150), 2)
prc = plate(200, 150, 9, W2=0, off=0, C=15, C_at=1)
show("三角 直角の角に C15 面積（点の並び）", prc["area"], 3); show("三角 直角の角に C15 面積（式: 15000 − 15²/2）", prc["area_exact"], 3)
show("三角 C15 重さ kg", prc["kg"], 4); show("材料表が「PL-9×200×150」で切る前の四角なら kg", prc["gross"], 4)
prc2 = plate(200, 150, 9, W2=0, off=0, C=15, C_at=2)
show("三角 右下（鋭い角 36.87°）に C15 面積（点の並び）", prc2["area"], 3); show("同（式: 15000 − 15²·sin36.87°/2）", prc2["area_exact"], 3)
pbad = plate(300, 200, 9, W2=150, off=0, hole=dict(d=22, x=280, y=150))
print("  穴を斜めの辺の近く（左から280・下から150）に置くと、収まる:", pbad["holes_ok"])

print("\n== 例3 穴の並び（水管橋の板など） PL-16×400×400・4×φ24（2×2・間隔300・端から50） ==")
pg = plate(400, 400, 16, hole=dict(d=24, x=50, y=50, nx=2, ny=2, px=300, py=300))
print("  穴の中心:", pg["centers"])
show("面積（点の並び）", pg["area"], 2); show("面積（式: 160000 − 4×π×12²）", pg["area_exact"], 2)
show("重さ kg（切った後）", pg["kg"], 3); show("切る前の四角 kg", pg["gross"], 3)
show("穴1つを360角で引いたときの誤差 mm2", PI * 144 - shoelace(holes(24, 0, 0)[1][0] + [holes(24, 0, 0)[1][0][0]]), 4)

print("\n== 例4 T字管のリブ（排水T字管 JIS G 3443-2 表13・大阪広域水道企業団 標準図 6-92／仕切弁副管A1号 6-116・6-117） ==")
# 大阪 6-92 の表（F12）: 呼び径, D2, d2, T, t, H, I, t'1, b, h, 参考質量
DRAIN = [("300×80", 318.5, 89.1, 6.9, 4.2, 400, 300, 6.0, 60, 50, 43.9), ("350×80", 355.6, 89.1, 6.0, 4.2, 500, 350, 6.0, 70, 50, 53.7),
         ("400×150", 406.4, 165.2, 6.0, 5.0, 500, 350, 6.0, 70, 50, 62.3), ("600×200", 609.6, 216.3, 6.0, 5.8, 750, 500, 6.0, 80, 60, 141),
         ("800×300", 812.8, 318.5, 8.0, 6.9, 1000, 600, 9.0, 100, 80, 328), ("1000×300", 1016.0, 318.5, 9.0, 6.9, 1000, 750, 9.0, 140, 120, 462),
         ("1000×400", 1016.0, 406.4, 9.0, 6.0, 1000, 750, 9.0, 140, 120, 458), ("1500×400", 1524.0, 406.4, 14.0, 6.0, 1000, 1100, 9.0, 220, 200, 1060),
         ("2600×400", 2641.6, 406.4, 24.0, 6.0, 1000, 1600, 9.0, 220, 200, 3110)]
print("  呼び径 | 上のリブ: 底の辺 b・高さ h・頂点のx・面積(点の並び/式)・重さ | 横のリブ（三角）重さ | 1基のリブ3枚 kg | 今のT字管の式の角Aと一致")
for name, D2, d2, T, tb, Hh, I, tr, b, h, mref in DRAIN:
    Ro, ro = D2 / 2, d2 / 2
    e = Ro - ro                    # 枝が本管の底に接する
    vc = e - ro                    # = Ro - d2
    r = rib_arc(Ro, vc, b, h)
    side = plate(b, h, tr, W2=0, off=0)
    # 今の T字管の式（teeY）で、枝の内側の線（θ=-90°）が本管の外面に当たる高さ ＝ 角 A の位置 u0
    uA = cut_y(Ro, ro - tb, ro, e, -PI / 2)
    tot = (r["area"] + 2 * side["area"]) * tr * RHO
    print(f"  {name:>9} | b={b} h={h} xC={r['xC']:.2f} 面積 {r['area']:.1f}/{r['area_exact']:.1f} {r['area'] * tr * RHO:.3f}kg | {side['kg']:.3f}kg | {tot:.3f} | u0 {r['u0']:.3f} = teeY {uA:.3f}")
# 板厚ゼロの極限の代わりに、本管の径を大きくした極限: 円が直線になり、リブは直角三角形（面積 b·h/2）に近づく
for R in (1e3, 1e5, 1e7):
    r = rib_arc(R, 0.0, 60, 50)
    print(f"  本管の半径 {R:.0e}: 面積 {r['area']:.4f}（直角三角形 1500）")
# 仕切弁副管A1号 500×100×100（6-116・6-117）: D 508.0・d1 114.3・リブ t3 6.0・q 60（高さ）・m 80（長さ）
r = rib_arc(254.0, 254.0 - 114.3, 80, 60)
show("仕切弁副管A1 500×100×100 上のリブ（上を尖らせた形）面積", r["area"], 1); show("同 式", r["area_exact"], 1); show("同 頂点のx", r["xC"], 2)
r2 = rib_arc(254.0, 254.0 - 114.3, 80, 60, top=20)
show("仕切弁副管A1 上が平ら（仮に幅20）の面積", r2["area"], 1); show("同 式", r2["area_exact"], 1)
# リブの板厚の両面で、底の辺（枝の外面）の高さがどれだけ違うか（板厚の中心の面で出す決まりの差）
ro, tr = 89.1 / 2, 6.0
show("排水T字管 300×80 リブの面と中心の面で、枝の外面の位置の差 mm", ro - math.sqrt(ro * ro - (tr / 2) ** 2), 3)

# 参考: JIS の参考質量と、本管・枝・リブの重さの和（本管の穴は板厚の中心の面で枝の内径の中を引く。枝の端の切り口は載せる形）
print("\n  参考質量の照らし（本管の長さ 2H・枝は本管の中心から I）:")
for name, D2, d2, T, tb, Hh, I, tr, b, h, mref in DRAIN:
    Ro, ro, Rm, rm = D2 / 2, d2 / 2, D2 / 2 - T / 2, d2 / 2 - tb / 2
    e, ri = Ro - ro, ro - tb
    main = 2 * PI * Rm * 2 * Hh * T * RHO
    # 本管の板厚の中心の面のうち、枝の内径の中に入る面積
    K = 20000; hole = 0.0
    for k in range(K):
        ph = -PI / 2 + PI * (k + 0.5) / K
        q = ri * ri - (Rm * math.sin(ph) - e) ** 2
        if q > 0: hole += 2 * math.sqrt(q) * Rm * PI / K
    br = sum((I - cut_y(Ro, ri, ro, e, 2 * PI * (k + 0.5) / 3600)) for k in range(3600)) / 3600 * 2 * PI * rm * tb * RHO
    rr = rib_arc(Ro, e - ro, b, h)
    ribs = (rr["area"] + b * h) * tr * RHO
    s = main - hole * T * RHO + br + ribs
    print(f"  {name:>9}: 本管 {main:.1f} − 穴 {hole * T * RHO:.2f} ＋ 枝 {br:.2f} ＋ リブ {ribs:.3f} ＝ {s:.1f} kg（参考質量 {mref}・差 {100 * (s - mref) / mref:+.2f}%）")

# 今の T字管の決まり（載せる・本管の穴は内面で枝の内径に合わせる）で、排水T字管の本管の穴が作れるか
print("\n  今の teeFeasible の本管の判定（載せる: 枝の内径の半分＋片寄せ ≦ 本管の内半径）:")
for name, D2, d2, T, tb, *_ in DRAIN:
    Ro, ro = D2 / 2, d2 / 2
    print(f"  {name:>9}: {ro - tb + (Ro - ro):.2f} ≦ {Ro - T:.2f} → {'作れる' if ro - tb + Ro - ro <= Ro - T else '「枝の穴が本管に収まらない」で止まる'}")
