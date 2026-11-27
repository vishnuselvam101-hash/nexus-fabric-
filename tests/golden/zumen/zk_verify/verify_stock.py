# 板の大きさに片が収まるかと、縦の継ぎ目で周方向に割った片の期待値を、画面の計算（shared.js）と別に出す。python verify_stock.py
# 標準ライブラリだけ。座標と板厚の決まりは shared.js に合わせる（x＝周の長さ、y＝管の軸の向き）
import math, sys
sys.stdout.reconfigure(encoding="utf-8")
PI, RHO = math.pi, 7.85e-6
rad, deg = math.radians, math.degrees
ng = 0


def ok(name, got, want, tol):
    global ng
    good = abs(got - want) <= tol
    ng += 0 if good else 1
    print(f"  {'OK' if good else 'NG'} {name}: {got:.3f}（期待 {want}・許容 {tol}）")


def shoelace(p):
    return abs(sum(p[i][0] * p[i + 1][1] - p[i + 1][0] * p[i][1] for i in range(len(p) - 1))) / 2


def bbox(p):
    xs, ys = [q[0] for q in p], [q[1] for q in p]
    return max(xs) - min(xs), max(ys) - min(ys)


def fits(w, h, stock, margin=0.0):
    # 向きは 0° と 90° だけ。板の短い辺に片の短い辺、長い辺に長い辺を当てる
    a, b = sorted((w, h))
    sa, sb = sorted(stock)
    return a <= sa - 2 * margin + 1e-9 and b <= sb - 2 * margin + 1e-9


def wrap(a, c):
    # 角 a を c を中心とする (c-π, c+π] に入れる
    return a - 2 * PI * math.floor((a - c + PI) / (2 * PI))


def clamp1(v):
    return max(-1.0, min(1.0, v))


STOCKS = {"5×20 (1524×6096)": (1524, 6096), "6×20 (1829×6096)": (1829, 6096), "8×20 (2438×6096)": (2438, 6096)}


# ---------- 円筒: 周の長さ W＝π(D−t)。n 本の縦の継ぎ目で W/n × L の四角が n 枚 ----------
def cyl(D, t, L, n):
    W = PI * (D - t)
    return [[[0, 0], [W / n, 0], [W / n, L], [0, L], [0, 0]] for _ in range(n)], W


def seams_needed(make, stock, nmax=6):
    for n in range(1, nmax + 1):
        ps = make(n)
        if ps is not None and all(fits(*bbox(p), stock) for p in ps):
            return n
    return None


print("== 例1 円筒 2600A（外径 2641.6・板厚 16） ==")
for L in (1500, 2500, 7000):
    print(f" 管の長さ {L}")
    for n in (1, 2, 3, 4):
        ps, W = cyl(2641.6, 16, L, n)
        w, h = bbox(ps[0])
        print(f"   継ぎ目 {n} 本: 1枚 {w:.1f} × {h:.1f}  " + "  ".join(f"{k}:{'入る' if fits(w, h, s) else '入らない'}" for k, s in STOCKS.items()))
    for k, s in STOCKS.items():
        need = seams_needed(lambda n: cyl(2641.6, 16, L, n)[0], s)
        print(f"   {k} に入る一番少ない継ぎ目: {need if need else '6本でも入らない（管の長さが板に入らない）'}")
ps, W = cyl(2641.6, 16, 1500, 2)
ok("円筒 周の長さ π(D−t)", W, 8248.57, 0.01)
ok("円筒 2枚の面積の合計 ＝ W×L", sum(shoelace(p) for p in ps), W * 1500, 1e-6)
ok("円筒 重さ（2枚の合計）＝ π(D−t)·L·t·ρ", sum(shoelace(p) for p in ps) * 16 * RHO, 1554.03, 0.01)


# ---------- えび曲げの片: shared.js の bendPiece と同じ外形を、縦の継ぎ目で周方向の帯に割る ----------
# 下の縁 y＝−h−lo·k·cosφ、上の縁 y＝cl+h+up·k·cosφ、φ＝2πx/C+ph。ph は継ぎ目（x＝0）の角（天から。地から45°なら 135°）
# n 本の継ぎ目は工場の決まりの位置から等間隔: φ_j＝ph+2πj/n、x_j＝Cj/n
def bend_bands(D, t, cl, lo, up, gam, n, basis="mid", amp="outer", allow=4.0, seam=("地", 45), M=720):
    o = {"outer": 0, "inner": t}.get(basis, t / 2)
    ao = {"mid": t / 2, "inner": t}.get(amp, 0)
    r, k, h = D / 2 - o, (D / 2 - ao) * math.tan(gam), allow / 2
    C = 2 * PI * r
    ph = rad(seam[1] if seam[0] == "天" else 180 - seam[1])
    m = M // n
    out = []
    for j in range(n):
        x0, x1 = C * j / n, C * (j + 1) / n
        xs = [x0 + (x1 - x0) * i / m for i in range(m + 1)]
        lower = [[x - x0, -h - lo * k * math.cos(2 * PI * x / C + ph)] for x in xs]
        upper = [[x - x0, cl + h + up * k * math.cos(2 * PI * x / C + ph)] for x in xs]
        pts = lower + upper[::-1] + [lower[0]]
        fa, fb = ph + 2 * PI * j / n, ph + 2 * PI * (j + 1) / n
        cmax = 1.0 if math.floor(fb / (2 * PI)) > math.floor(fa / (2 * PI)) or abs(math.cos(fa) - 1) < 1e-12 else max(math.cos(fa), math.cos(fb))
        out.append(dict(pts=pts, fa=fa, fb=fb,
                        area=(C / n) * (cl + 2 * h) + (up + lo) * k * C / (2 * PI) * (math.sin(fb) - math.sin(fa)),
                        height=cl + 2 * h + (up + lo) * k * cmax,
                        seam_len=cl + 2 * h + (up + lo) * k * math.cos(fa)))
    return out, C, k



print()
print("== 例2 えび曲げ 2600A（外径 2641.6・板厚 16・R2600・90°・切り口 3）の中間の片と始点側の片 ==")
gam = rad(90 / 3 / 2)
for name, cl, lo, up in (("中間", 2 * 2600 * math.tan(gam), 1, 1), ("始点側（直管 300）", 300 + 2600 * math.tan(gam), 0, 1)):
    print(f" {name}: cl {cl:.2f}")
    whole, C, k = bend_bands(2641.6, 16, cl, lo, up, gam, 1)
    print(f"   周の長さ C {C:.2f}・振れ幅 k {k:.2f}・継ぎ目1本の外形 {bbox(whole[0]['pts'])[0]:.1f} × {whole[0]['height']:.1f}")
    for n in (1, 2, 3):
        bands, C, k = bend_bands(2641.6, 16, cl, lo, up, gam, n)
        print(f"   継ぎ目 {n} 本:")
        for j, b in enumerate(bands):
            w, hgt = bbox(b["pts"])
            print(f"     {j + 1}/{n}: 天からの角 {deg(b['fa']) % 360:.1f}°〜{deg(b['fb']) % 360:.1f}°  外形 {w:.1f} × {hgt:.1f}（式 {b['height']:.1f}）  面積 {shoelace(b['pts']):.0f}（式 {b['area']:.0f}）  左の縁（継ぎ目）の長さ {b['seam_len']:.1f}  "
                  + " ".join(f"{kk[:4]}:{'入' if fits(w, hgt, s) else '×'}" for kk, s in STOCKS.items()))
        if n > 1:
            # 帯の右の縁と次の帯の左の縁（同じ継ぎ目）は同じ長さ
            for j in range(n):
                p, q = bands[j]["pts"], bands[(j + 1) % n]["pts"]
                m = len(p) // 2
                right = p[m][1] - p[m - 1][1]
                left = q[-2][1] - q[0][1]
                ok(f"   継ぎ目 {j + 1}-{(j + 1) % n + 1} の縁の長さが両側で同じ", right, left, 1e-6)
        ok(f"   継ぎ目 {n} 本 帯の面積の合計 ＝ 継ぎ目1本の面積", sum(shoelace(b["pts"]) for b in bands), shoelace(whole[0]["pts"]), 1.0)
        ok(f"   継ぎ目 {n} 本 帯の面積の合計 ＝ C(cl+2h)", sum(b["area"] for b in bands), C * (cl + 4), 1e-6)
        for b in bands:
            ok("   帯の外形の高さ（多角形 と 式）", bbox(b["pts"])[1], b["height"], 0.05)


# ---------- T字管の本管: 板 W×L（W＝2πRm）に枝の穴。縦の継ぎ目 n 本 ----------
# α は本管の周りの角（天＝枝の立つ所が 0、+ は shared.js の u が増える向き）。今の継ぎ目は α＝−ps（地から45°なら ps＝135°）
# 避ける範囲 K: 枝の穴（本管の内面）と、枝の外面が本管の外面に当たる線の両方を含む角の範囲に、空き c（板厚の中心で測る周の長さ）を足す
def tee_main(D, T, L, d, tb, e, xb, mount, n, gap=None, seam=("地", 45)):
    Ro, Ri, Rm = D / 2, D / 2 - T, D / 2 - T / 2
    ro = d / 2
    rh = ro if mount == "in" else ro - tb
    ps = rad(180 - seam[1] if seam[0] == "地" else seam[1])
    W = 2 * PI * Rm
    c = 5 * T if gap is None else gap
    g = c / Rm
    lo = min(math.asin(clamp1((e - rh) / Ri)), math.asin(clamp1((e - ro) / Ro))) - g
    hi = max(math.asin(clamp1((e + rh) / Ri)), math.asin(clamp1((e + ro) / Ro))) + g
    kc = (lo + hi) / 2
    rule = [-ps + 2 * PI * j / n for j in range(n)]
    hit = [a for a in rule if lo < wrap(a, kc) < hi]
    moved, cross = 0.0, False
    if not hit:
        seams = rule
    elif hi - lo > 2 * PI / n:
        seams, cross = None, True
    else:
        seams = [kc - PI / n + 2 * PI * j / n for j in range(n)]
        step = 2 * PI / n
        moved = (seams[0] - rule[0] + step / 2) % step - step / 2
    res = dict(W=W, plate=W / n, lo=lo, hi=hi, g=g, rule_hit=[deg(wrap(a, 0)) for a in hit], cross=cross, moved=deg(moved))
    if cross:
        return res
    # 穴のある板: その板の左の継ぎ目 a_h ≤ lo かつ hi ≤ a_h + 2π/n
    a_h = next(wrap(a, kc - PI / n) for a in seams if wrap(a, kc - PI / n) <= lo + 1e-12 and hi <= wrap(a, kc - PI / n) + 2 * PI / n + 1e-12)
    M = 360
    hole = [[Rm * (math.asin(clamp1((e + rh * math.sin(2 * PI * i / M)) / Ri)) - a_h), xb + rh * math.cos(2 * PI * i / M)] for i in range(M)]
    hole.append(hole[0])
    us = [q[0] for q in hole]
    res.update(seams=sorted(deg(wrap(a, 0)) for a in seams), a_h=deg(a_h), holeU=Rm * (math.asin(clamp1(e / Ri)) - a_h),
               holeW=max(us) - min(us), hole_u0=min(us), hole_u1=max(us), hole_area=shoelace(hole),
               keep_u0=Rm * (lo - a_h), keep_u1=Rm * (hi - a_h),
               kg=(W * L - shoelace(hole)) * T * RHO)
    return res


def show_tee(title, D, T, L, d, tb, e, xb, mount, gap=None):
    print(f" {title}")
    base = tee_main(D, T, L, d, tb, e, xb, mount, 1, gap=0)
    for n in (1, 2, 3, 4, 5, 6):
        r = tee_main(D, T, L, d, tb, e, xb, mount, n, gap)
        head = f"   継ぎ目 {n} 本（板 {r['plate']:.1f} × {L}）: 避ける範囲 天から {deg(r['lo']):.2f}°〜{deg(r['hi']):.2f}°"
        if r["cross"]:
            print(head + f"  → 範囲 {deg(r['hi'] - r['lo']):.2f}° が継ぎ目の間 {360 / n:.1f}° より広い。どう回しても穴か空きにかかる（確認）")
            continue
        how = "工場の決まりの位置のまま" if not r["rule_hit"] else f"決まりの位置では {', '.join(f'{a:.1f}°' for a in r['rule_hit'])} の継ぎ目がかかる → {r['moved']:+.2f}° 回して穴を板の真ん中に"
        print(head + f"  {how}")
        print(f"     継ぎ目（天からの角）: {', '.join(f'{a:.2f}°' for a in r['seams'])}  穴の板の左の継ぎ目 {r['a_h']:.2f}°")
        print(f"     穴の板の中: 穴の中心（継ぎ目から）{r['holeU']:.2f}  穴の周方向 {r['hole_u0']:.2f}〜{r['hole_u1']:.2f}（幅 {r['holeW']:.2f}）  避ける範囲 {r['keep_u0']:.2f}〜{r['keep_u1']:.2f}  板の幅 {r['plate']:.2f}")
        ok(f"     継ぎ目 {n} 本 穴の面積は継ぎ目1本と同じ", r["hole_area"], base["hole_area"], 1e-6)
        ok(f"     継ぎ目 {n} 本 穴の周方向の幅は継ぎ目1本と同じ", r["holeW"], base["holeW"], 1e-6)
        if not (r["keep_u0"] >= -1e-9 and r["keep_u1"] <= r["plate"] + 1e-9):
            global ng
            ng += 1
            print("     NG 避ける範囲が板からはみ出す")
    return base


print()
print("== 例3 T字管の本管（E の寸法: 本管 1016×12×1500、枝 610×9、載せる、片寄せ 0、空き 5×12＝60） ==")
b = show_tee("E", 1016, 12, 1500, 610, 9, 0, 750, "on")
ok("E 継ぎ目1本 穴の周方向（今の画面 642.03）", b["holeW"], 642.03, 0.05)
ok("E 継ぎ目1本 穴の中心（継ぎ目から・今の画面 1182.81）", b["holeU"], 1182.81, 0.05)
ok("E 本管の板の幅 2πRm（今の画面 3154.2）", b["W"], 3154.2, 0.05)
r2 = tee_main(1016, 12, 1500, 610, 9, 0, 750, "on", 2)
ok("E 継ぎ目2本 継ぎ目が決まりの位置のままなら穴の中心は継ぎ目1本と同じ", r2["holeU"], 1182.81 - 0 * r2["plate"], 0.05)
r3 = tee_main(1016, 12, 1500, 610, 9, 0, 750, "on", 3)
ok("E 継ぎ目3本 回したあと穴の中心は板の真ん中 πRm/3", r3["holeU"], PI * 502 / 3, 1e-6)
print(" 空きを 0 にしたとき（穴と枝の当たる線だけを避ける）")
for n in (4, 5):
    r = tee_main(1016, 12, 1500, 610, 9, 0, 750, "on", n, gap=0)
    print(f"   継ぎ目 {n} 本: {'収まらない（確認）' if r['cross'] else '収まる'}  避ける範囲 {deg(r['hi'] - r['lo']):.2f}°・継ぎ目の間 {360 / n:.1f}°")

print()
print("== 例4 T字管の本管（G2 の寸法: 本管 1016×12×1600、枝 610×9、片寄せ 150、枝の中心は管端から 600） ==")
show_tee("載せる・空き 60", 1016, 12, 1600, 610, 9, 150, 600, "on")
show_tee("差し込む・空き 60", 1016, 12, 1600, 610, 9, 150, 600, "in")
r = tee_main(1016, 12, 1600, 610, 9, 150, 600, "on", 4, gap=0)
print(f" 載せる・継ぎ目4本・空き 0: {'収まらない' if r['cross'] else '収まる'}（避ける範囲 {deg(r['hi'] - r['lo']):.2f}°・継ぎ目の間 90°・穴の板の中で避ける範囲 {r.get('keep_u0', 0):.2f}〜{r.get('keep_u1', 0):.2f}／板 {r['plate']:.2f}）")


# ---------- 今の例の片が、板の大きさの候補に入るか（新しい確認が出るか） ----------
def flare_size(D1, D2, L, t, n, basis="mid"):
    o = {"outer": 0, "inner": t}.get(basis, t / 2)
    rs, rl = D1 / 2 - o, D2 / 2 - o
    sinA = (rl - rs) / math.hypot(L, rl - rs)
    Rl, Rs, half = rl / sinA, rs / sinA, 2 * PI * sinA / n
    return 2 * Rl * math.sin(half / 2), Rl - Rs * math.cos(half / 2)


print()
print("== 今の例の片の外形と、板の大きさの候補（向きは 0° と 90°） ==")
rows = []
rows.append(("E 本管", 2 * PI * 502, 1500))
rows.append(("E 枝管（test_build の わき 487.15）", 2 * PI * (305 - 4.5), 487.15))
rows.append(("E 補強板（test_build の値）", 1328.74, 1000))
rows.append(("F 本管（同径・長さ 2000）", 2 * PI * 502, 2000))
rows.append(("F 枝管（同径・わき 1000−109.76）", 2 * PI * 502, 1000 - 109.76))
rows.append(("G2 本管（長さ 1600）", 2 * PI * 502, 1600))
for name, w, h in rows:
    print(f"  {name}: {w:.1f} × {h:.1f}  " + "  ".join(f"{k}:{'入る' if fits(w, h, s) else '入らない'}" for k, s in STOCKS.items()))

print()
print("合わない所 %d 件" % ng if ng else "全部合った")
sys.exit(1 if ng else 0)
