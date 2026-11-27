# 規格の寸法表（JIS G 3443-1 表7・JIS G 3443-2 表5〜13）の数を式で照らし、表から埋めた値での展開の期待値を、画面の計算（shared.js）と別に出す。
#   python verify_stdtable.py [表の JSON]（既定は同じフォルダの std_stdtable_draft.json）。標準ライブラリだけ
# 座標と板厚の決まりは shared.js に合わせる（周の長さは板厚の中心、えび曲げの切り口の振れ幅は外径、T字管は載せる形）
import json, math, os, sys
from decimal import Decimal, ROUND_HALF_EVEN

sys.stdout.reconfigure(encoding="utf-8")
PI, RHO = math.pi, 7.85e-6
here = os.path.dirname(os.path.abspath(__file__))
path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "std_stdtable_draft.json")
S = json.load(open(path, encoding="utf-8"))
ng, notes = 0, []

# 照らす表は std_stdtable_draft.json（既定。引数で変更可）。build が使う ../standards/jis_g3443.json と数が同じことを確かめる
std_path = os.path.join(here, "..", "standards", "jis_g3443.json")
S2 = json.load(open(std_path, encoding="utf-8"))
TABLES = ["pipe", "bend", "tee", "reducer", "bellmouth", "drain_tee"]
diff = [k for k in TABLES if S.get(k) != S2.get(k)]
print(f"照らす表: {os.path.basename(path)}（build が使うのは ../standards/jis_g3443.json）")
if diff:
    print("  NG 下書きと standards の表の数が違う: " + "・".join(diff))
    sys.exit(1)
print("  6つの表（pipe・bend・tee・reducer・bellmouth・drain_tee）の数は下書きと standards で一致（違うのは print_notes だけ）")


def check(name, good, detail=""):
    global ng
    if not good:
        ng += 1
        print(f"  NG {name} {detail}")


def ok(name, got, want, tol):
    global ng
    good = abs(got - want) <= tol
    ng += 0 if good else 1
    print(f"  {'OK' if good else 'NG'} {name}: {got:.3f}（期待 {want}・許容 {tol}）")


def rows(tab, cols=None):
    cols = cols or tab["cols"]
    return [dict(zip(cols, r)) for r in tab["rows"]]


def W(D, t):
    # JIS の質量の式（kg/m）
    return 0.02466 * t * (D - t)


def jis_round(x):
    # JIS Z 8401 規則A（偶数丸め）で有効数字3桁、1000 を超えたら整数
    d = Decimal(repr(x))
    if d >= 1000:
        return float(d.quantize(Decimal(1), ROUND_HALF_EVEN))
    e = d.adjusted() - 2
    return float(d.quantize(Decimal(1).scaleb(e), ROUND_HALF_EVEN))


# ---------- 直管（表7） ----------
P = {r["A"]: r for r in rows(S["pipe"])}
OD = {a: r["D"] for a, r in P.items()}
GR = ("STW290", "STW370", "STW400A", "STW400B")
print("== 直管（JIS G 3443-1 表7） ==")
cnt = 0
for a, r in P.items():
    for g in GR:
        t, kg = r["t_" + g], r["kg_" + g]
        if t is None:
            check(f"{a}A {g}", kg is None, "厚さが無いのに質量がある")
            continue
        cnt += 1
        check(f"{a}A {g} 質量", jis_round(W(r["D"], t)) == kg, f"表 {kg}・式 {W(r['D'], t):.4f}→{jis_round(W(r['D'], t))}")
print(f"  {cnt} 欄の質量が W＝0.02466·t·(D−t) の丸めと一致するか照らした")


def tA(a):
    # 異形管 F12・F15（T字管・排水T字管）の管厚の並び: 300A 以下は STW290、350A 以上は STW400 A種
    r = P[a]
    return r["t_STW290"] if a <= 300 else r["t_STW400A"]


def t29(a):
    # F29・F20 の並び: 300A 以下は STW370、350A 以上は STW400 A種
    r = P[a]
    return r["t_STW370"] if a <= 300 else r["t_STW400A"]


def tB(a):
    r = P[a]
    return r["t_STW400B"] if r["t_STW400B"] is not None else r["t_STW400A"]


# ---------- 曲管（表5〜9） ----------
print("== 曲管（表5〜9） ==")
BT = {r["A"]: r for r in rows(S["bend"]["T"])}
for a, r in BT.items():
    check(f"曲管 {a}A 外径", r["D"] == OD[a])
    check(f"曲管 {a}A F15", r["F15"] == (P[a]["t_STW290"]), f"{r['F15']}")
    check(f"曲管 {a}A F25", r["F25"] == P[a]["t_STW400B"], f"{r['F25']}")
    check(f"曲管 {a}A F29", r["F29"] == t29(a), f"{r['F29']}")
tan = lambda deg: math.tan(math.radians(deg))
worst = {}
for key in ("90", "45", "22.5", "11.25", "5.625"):
    tab = S["bend"][key]
    th, j = float(key), tab["joints"]
    dl = th / j
    w = {"l1": 0, "l2": 0, "L": 0, "kg": 0}
    for r in rows(tab):
        a, R, l3 = r["A"], r["R"], r["l3"]
        h = R * tan(dl / 2)
        e_L = r["L"] - (l3 + R * tan(th / 2))
        w["L"] = max(w["L"], abs(e_L))
        check(f"{key}° {a}A L＝l3＋R·tan(θ/2)", abs(e_L) <= 0.15, f"差 {e_L:.3f}")
        if j > 1:
            e1, e2 = r["l1"] - (l3 + h), r["l2"] - 2 * h
            w["l1"], w["l2"] = max(w["l1"], abs(e1)), max(w["l2"], abs(e2))
            check(f"{key}° {a}A l1＝l3＋R·tan(δ/2)", abs(e1) <= 0.15, f"差 {e1:.3f}")
            check(f"{key}° {a}A l2＝2R·tan(δ/2)", abs(e2) <= 0.15, f"差 {e2:.3f}")
            lc = 2 * r["l1"] + (j - 1) * r["l2"]
        else:
            lc = 2 * r["L"]
        check(f"{key}° {a}A 管心長", abs(r["Lc"] - lc) <= 0.05, f"表 {r['Lc']}・計算 {lc}")
        for f in ("F15", "F25", "F29"):
            kg, T = r["kg_" + f], BT[a][f]
            check(f"{key}° {a}A {f} 質量の有無", (kg is None) == (T is None))
            if kg is None or T is None:
                continue
            m = W(OD[a], T) * r["Lc"] / 1000
            rel = (kg - m) / m
            w["kg"] = max(w["kg"], abs(rel))
            check(f"{key}° {a}A {f} 質量≒W·管心長", abs(rel) <= 0.006, f"表 {kg}・計算 {m:.2f}")
    worst[key] = w
    print(f"  {key}°（切り口 {j}）: {len(tab['rows'])} 行。最大の差 l1 {w['l1']:.3f}・l2 {w['l2']:.3f}・L {w['L']:.3f} mm・質量 {100 * w['kg']:.2f}%")


# ---------- T字管の重さ（表の参考質量と照らすための近似。本管の板・枝の板・補強板を板厚の中心の面積で） ----------
def cut_y(Rc, ri, ro, e, th):
    # shared.js teeY と同じ: 枝の壁（ri..ro）の中で一番高く当たる点の、本管の中心線からの高さ
    p, q = e + ri * math.sin(th), e + ro * math.sin(th)
    m = 0.0 if p * q <= 0 else min(abs(p), abs(q))
    return math.sqrt(max(Rc * Rc - m * m, 0))


def shoelace(pts):
    return abs(sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))) / 2


def perim(pts):
    return sum(math.dist(pts[i], pts[(i + 1) % len(pts)]) for i in range(len(pts)))


def tee_mass(D, T, d, t, H, I, t1=None, B=None, e=0.0, N=720):
    Ro, Ri, Rm = D / 2, D / 2 - T, D / 2 - T / 2
    ro, ri, rm = d / 2, d / 2 - t, d / 2 - t / 2
    main = 2 * PI * Rm * 2 * H
    if e == 0:
        hole = shoelace([(Rm * math.asin(ri * math.sin(2 * PI * k / N) / Ri), ri * math.cos(2 * PI * k / N)) for k in range(N)])
    else:
        # 片寄せの枝（排水T字管）: 本管の板厚の中心の面で、枝の内側の筒が通る所の面積（枝が出る側の半周だけ）
        hole, M = 0.0, 4000
        for k in range(M):
            ph = PI * (k + 0.5) / M
            z = e - Rm * math.cos(ph)
            if abs(z) < ri:
                hole += 2 * math.sqrt(ri * ri - z * z) * Rm * PI / M
    br = sum((I - cut_y(Ro, ri, ro, e, 2 * PI * (k + 0.5) / N)) * rm * 2 * PI / N for k in range(N))
    pad = 0.0
    if t1:
        Rp = Ro + t1 / 2
        inn = [(Rp * math.asin(max(-1, min(1, (e + ro * math.sin(2 * PI * k / N)) / Ro))), ro * math.cos(2 * PI * k / N)) for k in range(N)]
        if e == 0:
            a0 = shoelace(inn)
            pad = perim(inn) * B + PI * B * B
        else:
            # 片寄せの枝: 枝の外面の線から B の帯。管底（φ＝90°）より先には回り込まない（表13の図の正面図）
            pad = band_area(inn, B, Rp * PI / 2)
    return RHO * (T * (main - hole) + t * br + (t1 or 0) * pad)


def band_area(poly, B, umax):
    # 凸に近い多角形を外へ B ずらした線と元の線の間の面積のうち、u≦umax の分（ずらした線を u＝umax で切る）
    n, s = len(poly), (1 if sum(poly[i][0] * poly[(i + 1) % len(poly)][1] - poly[(i + 1) % len(poly)][0] * poly[i][1] for i in range(len(poly))) > 0 else -1)
    out = []
    for i in range(n):
        p, q = poly[i - 1], poly[(i + 1) % n]
        tx, ty = q[0] - p[0], q[1] - p[1]
        L = math.hypot(tx, ty) or 1e-12
        out.append((poly[i][0] + s * ty / L * B, poly[i][1] - s * tx / L * B))
    clip = []
    for i in range(n):
        a, b = out[i], out[(i + 1) % n]
        ia, ib = a[0] <= umax, b[0] <= umax
        if ia:
            clip.append(a)
        if ia != ib:
            f = (umax - a[0]) / (b[0] - a[0])
            clip.append((umax, a[1] + f * (b[1] - a[1])))
    return shoelace(clip) - shoelace(poly)


print("== T字管（表10） ==")
TC = S["tee"]["cols"]
for f in ("F12", "F15"):
    worst_t, out = 0, []
    for r in rows(S["tee"][f], TC):
        A, a = r["A"], r["a"]
        check(f"T字管 {f} {A}×{a} 本管の管厚", r["T"] == tA(A), f"{r['T']}（並びは {tA(A)}）")
        check(f"T字管 {f} {A}×{a} 枝の管厚", r["t"] == tA(a), f"{r['t']}（並びは {tA(a)}）")
        check(f"T字管 {f} {A}×{a} 補強板の欄", (r["t1"] is None) == (r["B"] is None))
        m = tee_mass(OD[A], r["T"], OD[a], r["t"], r["H"], r["I"], r["t1"], r["B"])
        rel = (r["kg"] - m) / m
        worst_t = max(worst_t, abs(rel))
        if abs(rel) > 0.03:
            out.append(f"{A}×{a}（表 {r['kg']}・近似 {m:.1f}・{100 * rel:+.1f}%）")
    check(f"T字管 {f} 参考質量が近似の ±3% に入る", not out, "・".join(out))
    print(f"  {f}: {len(S['tee'][f]['rows'])} 行。参考質量と近似の差は最大 {100 * worst_t:.2f}%")

# ---------- 片落管（表11） ----------
print("== 片落管（表11） ==")
RC = S["reducer"]["cols"]
pn = {(n["row"][0], n["row"][1]): n for n in S["print_notes"] if n["table"] == "reducer"}


def frustum_area(D1, D2, L, t):
    r1, r2 = D1 / 2 - t / 2, D2 / 2 - t / 2
    return PI * (r1 + r2) * math.hypot(L, r1 - r2)


wr, outs = 0, []
for r in rows(S["reducer"], RC):
    A, a = r["A"], r["a"]
    if r["C"] + r["E"] + r["W"] != r["L"]:
        check(f"片落管 {A}×{a} C＋E＋W＝L", (A, a) in pn, f"{r['C']}+{r['E']}+{r['W']}≠{r['L']}")
        notes.append(f"片落管 {A}×{a}: C＋E＋W＝{r['C'] + r['E'] + r['W']}、L＝{r['L']}（印字の食い違いとして print_notes に記録済み）")
    for f in ("F15", "F25", "F29"):
        T, t, t2, kg = r[f + "_T"], r[f + "_t"], r[f + "_t2"], r["kg_" + f]
        check(f"片落管 {A}×{a} {f} 欄の有無", (T is None) == (kg is None))
        if T is None:
            continue
        rule = {"F15": (P[A]["t_STW290"], P[a]["t_STW290"]), "F25": (tB(A), tB(a)), "F29": (t29(A), t29(a))}[f]
        check(f"片落管 {A}×{a} {f} 管厚の並び", (T, t) == rule, f"{(T, t)}（並びは {rule}）")
        if A >= 700:
            check(f"片落管 {A}×{a} {f} 円すいの管厚＝大径側", t2 == T, f"{t2}")
        m = RHO * (T * PI * (OD[A] - T) * r["C"] + t * PI * (OD[a] - t) * r["E"] + t2 * frustum_area(OD[A], OD[a], r["W"], t2))
        rel = (kg - m) / m
        if (A, a) in pn and "C" == pn[(A, a)]["col"]:
            # 印字の C を 300 にした重さでも照らす（表の参考質量がどちらの C で出したか）
            m300 = m - RHO * T * PI * (OD[A] - T) * (r["C"] - (r["L"] - r["E"] - r["W"]))
            notes.append(f"片落管 {A}×{a} {f}: 参考質量 {kg} は、C＝{r['C']} の近似 {m:.1f}（{100 * rel:+.1f}%）より C＝{r['L'] - r['E'] - r['W']} の近似 {m300:.1f}（{100 * (kg - m300) / m300:+.1f}%）に近い")
            continue
        wr = max(wr, abs(rel))
        if abs(rel) > 0.02:
            outs.append(f"{A}×{a} {f}（表 {kg}・近似 {m:.2f}・{100 * rel:+.1f}%）")
for o in outs:
    notes.append(f"片落管 {o}: 管厚と長さは画像で読み直して表のとおり。参考質量だけが近似から外れる（質量は展開に使わない）")
check("片落管 参考質量が近似の ±4% に入る", wr <= 0.04, f"最大 {100 * wr:.2f}%")
print(f"  {len(S['reducer']['rows'])} 行。参考質量と近似（直管2つ＋円すい台）の差は最大 {100 * wr:.2f}%（±2% を外れる行 {len(outs)}）")

# ---------- らっぱ口（表12） ----------
print("== らっぱ口（表12） ==")
wb = 0
for r in rows(S["bellmouth"]):
    a = r["A"]
    for f in ("F15", "F25", "F29"):
        T, kg = r["T_" + f], r["kg_" + f]
        check(f"らっぱ口 {a}A {f} 管厚＝曲管の表", T == BT[a][f], f"{T}")
        if T is None:
            continue
        m = RHO * T * (PI * (OD[a] - T) * r["L"] + frustum_area(OD[a], r["Dm"], r["H"], T))
        rel = (kg - m) / m
        wb = max(wb, abs(rel))
        check(f"らっぱ口 {a}A {f} 質量", abs(rel) <= 0.02, f"表 {kg}・近似 {m:.2f}")
print(f"  {len(S['bellmouth']['rows'])} 行。参考質量と近似（直管＋円すい台）の差は最大 {100 * wb:.2f}%")

# ---------- 排水T字管（表13） ----------
print("== 排水T字管（表13） ==")
DC = S["drain_tee"]["cols"]
for f in ("F12", "F15", "F20", "F25", "F29"):
    wd, outs = 0, []
    for r in rows(S["drain_tee"][f], DC):
        A, a = r["A"], r["a"]
        rule = (tA(A), tA(a)) if f in ("F12", "F15") else (t29(A), t29(a))
        check(f"排水T字管 {f} {A}×{a} 管厚の並び", (r["T"], r["t"]) == rule, f"{(r['T'], r['t'])}（並びは {rule}）")
        D, d = OD[A], OD[a]
        m = tee_mass(D, r["T"], d, r["t"], r["H"], r["I"], r["t1"], r["B"], e=(D - d) / 2)
        if r["rt"]:
            m += RHO * r["rt"] * 3 * r["rb"] * r["rh"] / 2
        rel = (r["kg"] - m) / m
        wd = max(wd, abs(rel))
        if abs(rel) > 0.05:
            outs.append(f"{A}×{a}（表 {r['kg']}・近似 {m:.1f}・{100 * rel:+.1f}%）")
    check(f"排水T字管 {f} 参考質量が近似の ±5% に入る（補強板の形は近似）", not outs, "・".join(outs))
    print(f"  {f}: {len(S['drain_tee'][f]['rows'])} 行。参考質量と近似（本管・片寄せの枝・リブ3枚・補強板）の差は最大 {100 * wd:.2f}%")

print("== 表の照らしの結果 ==")
for n in notes:
    print("  " + n)
print(f"  NG {ng} 件")


# ======================================================================
# 確かめの例（表から埋めた値での展開の期待値）
# ======================================================================
def bend_piece(D, t, R, ang, joints, cl, lo, up, allow=4.0, M=3600):
    # shared.js bendPiece と同じ取り方: 周は板厚の中心、振れ幅は外径、余長は両端に半分ずつ（継ぎ目の位置は面積・長さに効かない）
    r = D / 2 - t / 2
    k = (D / 2) * math.tan(math.radians(ang / joints / 2))
    C, h = 2 * PI * r, allow / 2
    low = [(C * i / M, -h - lo * k * math.cos(2 * PI * i / M)) for i in range(M + 1)]
    upp = [(C * i / M, cl + h + up * k * math.cos(2 * PI * i / M)) for i in range(M + 1)]
    pts = low + upp[::-1]
    return {"C": C, "k": k, "long": cl + allow + (lo + up) * k, "short": cl + allow - (lo + up) * k, "area": shoelace(pts), "pts": pts}


def flare(D1, D2, L, t, n=2):
    # shared.js flarePiece と同じ: 板厚の中心、n 枚割りの1枚（扇形の一部）
    rs, rl = min(D1, D2) / 2 - t / 2, max(D1, D2) / 2 - t / 2
    sl = math.hypot(L, rl - rs)
    sa = (rl - rs) / sl
    Rl, Rs, ang = rl / sa, rs / sa, 2 * PI * sa / n
    return {"rs": rs, "rl": rl, "sl": sl, "Rl": Rl, "Rs": Rs, "ang": ang, "chord": 2 * Rl * math.sin(ang / 2), "height": Rl - Rs * math.cos(ang / 2),
            "area": (Rl * Rl - Rs * Rs) * ang / 2, "arc_l": Rl * ang, "arc_s": Rs * ang}


def find(tab, **kw):
    for r in rows(tab):
        if all(r[k] == v for k, v in kw.items()):
            return r


print()
print("== 例1 90°曲管 600A・F29（表5）→ えび曲げ ==")
r = find(S["bend"]["90"], A=600)
D, T = OD[600], BT[600]["F29"]
print(f"  埋める値: D {D}・t {T}・R {r['R']}・angle 90・joints 3・start_straight＝end_straight {r['l3']}（照らす値 start_center＝end_center {r['l1']}・mid_center {r['l2']}）")
h = r["R"] * tan(15)
ends, mid = bend_piece(D, T, r["R"], 90, 3, r["l3"] + h, 0, 1), bend_piece(D, T, r["R"], 90, 3, 2 * h, 1, 1)
ok("端の片の中心の長さ l3＋R·tan15°", r["l3"] + h, r["l1"], 0.1)
ok("中間の片の中心の長さ 2R·tan15°", 2 * h, r["l2"], 0.15)
ok("周の長さ π(D−t)", ends["C"], 1896.27, 0.01)
ok("切り口の振れ幅 (D/2)·tan15°", ends["k"], 81.67, 0.01)
print(f"  端の片（2枚）: 長い側 {ends['long']:.2f}・短い側 {ends['short']:.2f}（余長 4 を含む）")
print(f"  中間の片（2枚）: 長い側 {mid['long']:.2f}・短い側 {mid['short']:.2f}")
ok("端の片の面積（多角形）＝周×(中心の長さ＋余長)", ends["area"], ends["C"] * (r["l3"] + h + 4), 1.0)
mass = RHO * T * ends["C"] * (2 * (r["l3"] + h) + 2 * 2 * h)
ok("4枚の重さ（余長なし）≒表の参考質量 129", mass, r["kg_F29"], 0.6)
lim = bend_piece(D, T, r["R"], 1e-9, 3, 500, 1, 1)
ok("極限: 角度→0 で切り口が平ら（円筒 500 の四角）", lim["long"] - lim["short"], 0.0, 1e-6)

print("== 例2 T字管 1000×600・F12（表10）→ 本管・枝管 ==")
r = dict(zip(TC, next(x for x in S["tee"]["F12"]["rows"] if x[0] == 1000 and x[1] == 600)))
D, T, d, t = OD[1000], r["T"], OD[600], r["t"]
print(f"  埋める値: 本管 D {D}・t {T}・L 2H＝{2 * r['H']}・branch_x H＝{r['H']}・branch_D {d}・branch_t {t}／枝管 D {d}・t {t}・H（表の I）{r['I']}／補強板なし（表の t1・B が「－」）")
Ro, Ri, Rm, ro, ri, rm = D / 2, D / 2 - T, D / 2 - T / 2, d / 2, d / 2 - t, d / 2 - t / 2
print(f"  本管の板 {2 * PI * Rm:.2f}×{2 * r['H']}・穴（本管の内面で枝の内径に合わせ、板厚の中心で測る）周方向 {2 * Rm * math.asin(ri / Ri):.2f}×軸方向 {2 * ri:.2f}・穴の中心は管端から {r['H']}")
print(f"  枝の板の幅 2π·rm {2 * PI * rm:.2f}")
# 総当たり: 穴の縁を枝の内面の母線と本管の内面の交わりから出し、周方向の全幅を測る
best = max(abs(Rm * math.atan2(ri * math.sin(2 * PI * k / 7200), math.sqrt(Ri * Ri - (ri * math.sin(2 * PI * k / 7200)) ** 2))) for k in range(7200))
ok("総当たり: 穴の周方向の全幅", 2 * best, 2 * Rm * math.asin(ri / Ri), 1e-6)
for dg in (0, 22.5, 45, 67.5, 90):
    th = math.radians(dg)
    y = cut_y(Ro, ri, ro, 0, th)
    # 総当たり: 枝の内面の母線（本管の中心線から高さ z）を上から下ろし、本管の外面に当たる高さを二分法で
    xx, lo_, hi_ = ri * math.cos(th), 0.0, Ro
    yy = ri * math.sin(th)
    for _ in range(80):
        z = (lo_ + hi_) / 2
        lo_, hi_ = (z, hi_) if yy * yy + z * z < Ro * Ro else (lo_, z)
    print(f"  θ={dg:>5}°  当たる高さ {y:.2f}（二分法 {lo_:.2f}）  枝の管端から切り口まで {r['I'] - y:.2f}")
    check(f"二分法と式 θ={dg}", abs(lo_ - y) < 1e-6)
m = tee_mass(D, T, d, t, r["H"], r["I"])
ok("重さ（本管の板−穴＋枝の板）≒表の参考質量 455", m, r["kg"], 455 * 0.01)

print("== 例3 片落管 1000×600・F25（表11）→ 直管・同心の円すい（二つ割）・直管 ==")
r = dict(zip(RC, next(x for x in S["reducer"]["rows"] if x[0] == 1000 and x[1] == 600)))
T, t, t2 = r["F25_T"], r["F25_t"], r["F25_t2"]
print(f"  埋める値: 大径側の管 D {OD[1000]}・t {T}・L（表の C）{r['C']}／円すい D1 {OD[1000]}・D2 {OD[600]}・L（表の W）{r['W']}・t（表の t2）{t2}・split 2（既定）／小径側の管 D {OD[600]}・t {t}・L（表の E）{r['E']}")
f = flare(OD[1000], OD[600], r["W"], t2)
print(f"  rs {f['rs']:.2f}・rl {f['rl']:.2f}・母線 {f['sl']:.3f}・扇の半径 Rl {f['Rl']:.2f}・Rs {f['Rs']:.2f}・1枚の扇の角 {math.degrees(f['ang']):.3f}°")
print(f"  1枚の外形（弦×高さ）{f['chord']:.2f}×{f['height']:.2f}（2枚）")
ok("周: 大きい弧 Rl·角＝2π·rl/2", f["arc_l"], PI * f["rl"], 1e-6)
ok("周: 小さい弧 Rs·角＝2π·rs/2", f["arc_s"], PI * f["rs"], 1e-6)
ok("面積: 扇＝π(rl+rs)·母線/2", f["area"], PI * (f["rl"] + f["rs"]) * f["sl"] / 2, 1e-3)
lim = flare(1000.0, 1000.0 - 1e-6, 700, 8)
ok("極限: 両端の口が同じ→高さは母線（円筒の四角の高さ 700）", lim["Rl"] - lim["Rs"], 700.0, 1e-3)
m = RHO * (T * PI * (OD[1000] - T) * r["C"] + t * PI * (OD[600] - t) * r["E"] + t2 * 2 * f["area"])
ok("重さ（直管2つ＋円すい2枚）≒表の参考質量 188", m, r["kg_F25"], 188 * 0.01)
print(f"  大径側の管 {PI * (OD[1000] - T):.2f}×{r['C']}・小径側の管 {PI * (OD[600] - t):.2f}×{r['E']}")

print("== 例4 らっぱ口 1600A・F29（表12）→ 円すい（二つ割）・直管 ==")
r = find(S["bellmouth"], A=1600)
T = r["T_F29"]
print(f"  埋める値: 円すい D1 {OD[1600]}・D2（表の D2'）{r['Dm']}・L（表の H）{r['H']}・t {T}・split 2／管 D {OD[1600]}・t {T}・L {r['L']}")
f = flare(OD[1600], r["Dm"], r["H"], T)
print(f"  扇 Rl {f['Rl']:.2f}・Rs {f['Rs']:.2f}・角 {math.degrees(f['ang']):.3f}°・1枚の外形（弦×高さ）{f['chord']:.2f}×{f['height']:.2f}（2枚）・管の板 {PI * (OD[1600] - T):.2f}×{r['L']}")
m = RHO * T * (PI * (OD[1600] - T) * r["L"] + 2 * f["area"])
ok("重さ ≒表の参考質量 945", m, r["kg_F29"], 945 * 0.01)

print("== 例5 排水T字管 600×200・F12（表13）→ 読み取りの値（J_drain と同じになるか） ==")
r = dict(zip(DC, next(x for x in S["drain_tee"]["F12"]["rows"] if x[0] == 600 and x[1] == 200)))
D, d = OD[600], OD[200]
want = {"D": 609.6, "t": 6.0, "L": 1500, "branch_D": 216.3, "branch_t": 5.8, "H": 500, "branch_x": 750, "e": 196.65, "rib_t": 6.0, "rib_b": 80, "rib_h": 60}
got = {"D": D, "t": r["T"], "L": 2 * r["H"], "branch_D": d, "branch_t": r["t"], "H": r["I"], "branch_x": r["H"], "e": round((D - d) / 2, 2), "rib_t": r["rt"], "rib_b": r["rb"], "rib_h": r["rh"]}
for k in want:
    ok(f"{k}", got[k], want[k], 1e-9)
print("  e_side 180（地）・rib_n1 1・rib_n2 2・補強板なし（F12）。この読み取りの切図の期待値は readtest/J_drain_expected.md")

# ======================================================================
# 6つの K_std 読み取りに、規格の表から埋まる値の期待値（spec_stdtable.md 3章の対応で別に出す）
# 1行＝ 読み取り・部材・値の名前・期待値・表の欄。図面に既に値がある所は埋めない（対象外）
# ======================================================================
print()
print("== K_std 読み取りに規格の表から埋まる値（読み取り・部材・key・期待値・表の欄） ==")
RDIR = os.path.join(here, "..", "..", "design-demos", "assets", "kirizu", "readings")
BEND_ANG = ["90", "45", "22.5", "11.25", "5.625"]
bendAng = {k: {r["A"]: r for r in rows(S["bend"][k])} for k in BEND_ANG}
teeIdx = {c: {(r["A"], r["a"]): r for r in rows(S["tee"][c], TC)} for c in ("F12", "F15")}
redIdx = {(r["A"], r["a"]): r for r in rows(S["reducer"], RC)}
bellIdx = {r["A"]: r for r in rows(S["bellmouth"])}
drainIdx = {c: {(r["A"], r["a"]): r for r in rows(S["drain_tee"][c], DC)} for c in ("F12", "F15", "F20", "F25", "F29")}


def pval(p, k):
    q = (p.get("params") or {}).get(k)
    return q.get("value") if q else None


def dump_fills(fn, fills_of):
    # fills_of(part, rd) → { key: (値, 表の欄) }。図面に値が無い（null）所だけを出す
    rd = json.load(open(os.path.join(RDIR, fn), encoding="utf-8"))
    for p in rd["parts"]:
        for k, (val, cell) in fills_of(p, rd).items():
            if pval(p, k) is None and val is not None:
                print(f"  {fn}  部材{p['no']}  {k}  {val}  （{cell}）")


def head_of(rd):
    return next(p for p in rd["parts"] if p.get("std"))


def gv(rd, k):
    return next((pval(p, k) for p in rd["parts"] if pval(p, k) is not None), None)


# 直管（表7）。材質が STW400 で A種・B種が決まらなければ板厚は埋めない（std_grade）
def pipe_fills(p, rd):
    A = gv(rd, "nom")
    if A is None or A not in P:
        return {}
    mat = "".join((r.get("material") or "") for r in rd.get("material_table", [])).upper().replace(" ", "").replace("-", "")
    f = {"D": (OD[A], f"pipe {A}A D")}
    g = "STW290" if "STW290" in mat else "STW370" if "STW370" in mat else "STW400A" if "STW400A" in mat else "STW400B" if "STW400B" in mat else None
    if g and P[A]["t_" + g] is not None:
        f["t"] = (P[A]["t_" + g], f"pipe {A}A {g}")
    return f


def bend_fills(p, rd):
    A, pc, ang = gv(rd, "nom"), gv(rd, "pclass"), pval(head_of(rd), "angle")
    key = next((k for k in BEND_ANG if ang is not None and abs(float(k) - ang) < 1e-6), None)
    if A is None or pc is None or key is None or A not in bendAng[key]:
        return {}
    cls = "F" + str(pc)
    r, j = bendAng[key][A], S["bend"][key]["joints"]
    T = BT[A][cls]
    f = {"D": (OD[A], f"pipe {A}A D"), "t": (T, f"bend.T {A} {cls}"), "R": (r["R"], f"bend.{key} {A} R"),
         "joints": (j, f"bend.{key} 図の片の数"), "start_straight": (r["l3"], f"bend.{key} {A} l3"), "end_straight": (r["l3"], f"bend.{key} {A} l3")}
    if j > 1:
        f.update({"start_center": (r["l1"], f"bend.{key} {A} l1"), "end_center": (r["l1"], f"bend.{key} {A} l1"), "mid_center": (r["l2"], f"bend.{key} {A} l2")})
    else:
        f.update({"start_center": (r["L"], f"bend.{key} {A} L"), "end_center": (r["L"], f"bend.{key} {A} L")})
    return f


def tee_fills(p, rd):
    A, a, pc = gv(rd, "nom"), gv(rd, "nom2"), gv(rd, "pclass")
    if None in (A, a, pc) or (A, a) not in teeIdx["F" + str(pc)]:
        return {}
    r = teeIdx["F" + str(pc)][(A, a)]
    D2, d2 = OD[A], OD[a]
    if p["shape"] == "tee_main":
        return {"D": (D2, f"tee {A}×{a} D2"), "t": (r["T"], "T"), "L": (2 * r["H"], "2H"), "branch_x": (r["H"], "H"), "branch_D": (d2, "d2"), "branch_t": (r["t"], "t")}
    if p["shape"] == "tee_branch":
        return {"D": (d2, "d2"), "t": (r["t"], "t"), "main_D": (D2, "D2"), "main_t": (r["T"], "T"), "H": (r["I"], "I")}
    if p["shape"] == "tee_pad" and r["t1"] is not None:
        return {"t": (r["t1"], "t1"), "pad_B": (r["B"], "B"), "main_D": (D2, "D2"), "branch_D": (d2, "d2")}
    return {}


def reducer_fills(p, rd):
    A, a, pc = gv(rd, "nom"), gv(rd, "nom2"), gv(rd, "pclass")
    if None in (A, a, pc) or (A, a) not in redIdx:
        return {}
    r, cls = redIdx[(A, a)], "F" + str(pc)
    D2, d2 = OD[A], OD[a]
    if p["shape"] == "cone":
        return {"D1": (D2, "D2"), "D2": (d2, "d2"), "L": (r["W"], "W"), "t": (r[cls + "_t2"], cls + "_t2")}
    if p["shape"] == "cylinder":
        # 大径側か小径側か: 図の外径、無ければ名前で決める
        d = pval(p, "D")
        big = (d is not None and abs(d - D2) <= 0.5) or (d is None and "大" in (p.get("name") or ""))
        if big:
            return {"D": (D2, "D2"), "t": (r[cls + "_T"], cls + "_T"), "L": (r["C"], "C")}
        return {"D": (d2, "d2"), "t": (r[cls + "_t"], cls + "_t"), "L": (r["E"], "E")}
    return {}


def bell_fills(p, rd):
    A, pc = gv(rd, "nom"), gv(rd, "pclass")
    if None in (A, pc) or A not in bellIdx:
        return {}
    r, cls = bellIdx[A], "F" + str(pc)
    T = r["T_" + cls]
    if p["shape"] == "cone":
        return {"D1": (OD[A], f"pipe {A}A D"), "D2": (r["Dm"], "D2'"), "L": (r["H"], "H"), "t": (T, "T_" + cls)}
    if p["shape"] == "cylinder":
        return {"D": (OD[A], f"pipe {A}A D"), "t": (T, "T_" + cls), "L": (r["L"], "L")}
    return {}


def drain_fills(p, rd):
    A, a, pc = gv(rd, "nom"), gv(rd, "nom2"), gv(rd, "pclass")
    if None in (A, a, pc) or (A, a) not in drainIdx["F" + str(pc)]:
        return {}
    r = drainIdx["F" + str(pc)][(A, a)]
    D2, d2 = OD[A], OD[a]
    e = round((D2 - d2) / 2, 2)
    hasDir = any(pval(q, "dir") is not None for q in rd["parts"])
    es = {"e_side": (180, "図（管底にそろう）")} if hasDir else {}
    if p["shape"] == "tee_main":
        return dict({"D": (D2, "D2"), "t": (r["T"], "T"), "L": (2 * r["H"], "2H"), "branch_x": (r["H"], "H"), "branch_D": (d2, "d2"), "branch_t": (r["t"], "t"), "e": (e, "(D2−d2)/2")}, **es)
    if p["shape"] == "tee_branch":
        return dict({"D": (d2, "d2"), "t": (r["t"], "t"), "main_D": (D2, "D2"), "main_t": (r["T"], "T"), "H": (r["I"], "I"), "e": (e, "(D2−d2)/2")}, **es)
    if p["shape"] == "tee_rib" and r["rt"] is not None:
        f = {"t": (r["rt"], "t'1"), "rib_b": (r["rb"], "b"), "rib_h": (r["rh"], "h"), "main_D": (D2, "D2"), "branch_D": (d2, "d2"), "e": (e, "(D2−d2)/2")}
        if sum(1 for q in rd["parts"] if q["shape"] == "tee_rib") == 1:
            f.update({"rib_n1": (1, "図の上のリブ"), "rib_n2": (2, "図のわきのリブ")})
        return f
    if p["shape"] == "tee_pad" and r["t1"] is not None:
        return dict({"t": (r["t1"], "t1"), "pad_B": (r["B"], "B"), "main_D": (D2, "D2"), "branch_D": (d2, "d2"), "e": (e, "(D2−d2)/2")}, **es)
    return {}


for fn, fo in [("K_std_pipe.json", pipe_fills), ("K_std_bend.json", bend_fills), ("K_std_tee.json", tee_fills),
               ("K_std_reducer.json", reducer_fills), ("K_std_bellmouth.json", bell_fills), ("K_std_drain.json", drain_fills)]:
    dump_fills(fn, fo)

print()
print(f"全部で NG {ng} 件" if ng else "全部合った")
