# 見直し（docs/reviews/REVIEW_2026-11-24_shapes.md）の直し用の確かめ。python3 -I verify_review_fixes.py
# test_build.mjs に足す期待値（符号付きの頂点の距離・負の周の長さ）と、止め方の境目を別に出す。
# 寸法は spec_yfork.md の例4（本管 1016×12・枝 610×9・挟む角 60°・交点から小さい口まで 900）と
# 見直しの edge case（t=300 内側・branch_t=700・枝が本管より太い形）。実在の図面の寸法は使っていない。
import math

NG = 0


def chk(name, cond, info=""):
    global NG
    if not cond:
        NG += 1
    print(("OK " if cond else "NG ") + name + ("  " + info if info else ""))


def psi_of(D, d, sr):
    # 交点を中心とする本管の外半径 Rs の球に接する円すいの片側の角（閉じた式）
    Rs, r = D / 2, d / 2
    hy = math.sqrt(sr * sr + r * r)
    return math.asin(Rs / hy) - math.atan2(r, sr)


def psi_bisect(D, d, sr):
    # 独立な確かめ: 頂点が小さい口の外側にある円すい（頂点の距離 Rs/sinψ）が
    # 小さい口の縁（半径 r・距離 sr）を通る方程式 (Rs/sinψ - sr)·tanψ = r を二分法で解く
    Rs, r = D / 2, d / 2
    f = lambda p: (Rs / math.sin(p) - sr) * math.tan(p) - r
    # f は ψ が小さいと +（頂点が遠く縁が大きい）、大きいと - になる（単調なので根は1つ）
    a, b = 1e-9, math.pi / 2 - 1e-9
    fa = f(a)
    while b - a > 1e-14:
        m = (a + b) / 2
        if (f(m) > 0) == (fa > 0):
            a, fa = m, f(m)
        else:
            b = m
    return (a + b) / 2


print("=== M5 二股の円すい: 小さい口が頂点の内側に入る境目（wyc_t300_inner） ===")
D, d, sr, W = 1016.0, 610.0, 900.0, 60.0
Rs, r = D / 2, d / 2
psi = psi_of(D, d, sr)
psi2 = psi_bisect(D, d, sr)
chk("ψ 閉じた式 ＝ 二分法", abs(psi - psi2) < 1e-12, f"{math.degrees(psi):.6f}° / {math.degrees(psi2):.6f}°")
print(f"  ψ = {math.degrees(psi):.6f}°  Rs = {Rs}  r = {r}  sr = {sr}")

# 板厚の基準 inner（o = t）。t = 300
t, o = 300.0, 300.0
sp, cp = math.sin(psi), math.cos(psi)
sab = (Rs - o) / sp          # 頂点から交点までの距離（基準の面）
lb_sr = (sab - sr) / cp      # 頂点から小さい口までの距離（符号付き）
rEnd = (Rs - sr * sp - o) / cp  # 小さい口の基準の面の半径
chk("t=300 内側: 頂点の距離 sab < 小さい口の距離 sr", sab < sr, f"sab = {sab:.6f} < sr = {sr}")
chk("t=300 内側: 符号付きの頂点の距離 lb(sr) < 0", lb_sr < 0, f"lb(sr) = {lb_sr:.6f}")
chk("小さい口の半径 rEnd < 0（扇が頂点の裏側で交わる）", rEnd < 0, f"rEnd = {rEnd:.6f}")
chk("lb(sr)·sinψ ＝ rEnd（恒等式）", abs(lb_sr * sp - rEnd) < 1e-9, f"{lb_sr * sp:.9f} / {rEnd:.9f}")
print(f"  期待値（test_build の符号付き apex）: {lb_sr:.6f}")

# 今の有効な例（t=12・基準 mid・o=6）は符号付きでも正（既存の期待値 1271.33334 が変わらない）
o12 = 6.0
sab12 = (Rs - o12) / sp
lb12 = (sab12 - sr) / cp
chk("t=12 中心: lb(sr) > 0（既存の期待値 1271.33334）", lb12 > 0 and abs(lb12 - 1271.33334) < 0.01, f"lb(sr) = {lb12:.6f}")

# 枝が本管より太い形（D=610・d=1016・t=9・基準 mid）は ψ < 0 で lb(sr) < 0 が正常。
# 小さい口の半径 rEnd > 0 なので作れる。止めは lb(sr)·ψ > 0（＝ rEnd > 0）で見る
De, de, te = 610.0, 1016.0, 9.0
psie = psi_of(De, de, sr)
spe, cpe = math.sin(psie), math.cos(psie)
oe = te / 2
sabe = (De / 2 - oe) / spe
lbe = (sabe - sr) / cpe
rEnde = (De / 2 - sr * spe - oe) / cpe
chk("太くなる円すい: ψ < 0", psie < 0, f"ψ = {math.degrees(psie):.6f}°")
chk("太くなる円すい: lb(sr) < 0 でも rEnd > 0（作れる・既存の例は止めない）", lbe < 0 and rEnde > 0, f"lb(sr) = {lbe:.6f}  rEnd = {rEnde:.6f}")
chk("太くなる円すい: lb(sr)·ψ > 0", lbe * psie > 0, f"{lbe * psie:.6f}")
chk("細くなる円すい（t=300 内側）: lb(sr)·ψ < 0（止める）", lb_sr * psi < 0, f"{lb_sr * psi:.6f}")

print()
print("=== M4 二股の枝の管: 板厚は外径の半分より小さいはず（branch_t=700 は周の長さが負になる例） ===")
bd, bt = 610.0, 700.0
ob = bt / 2  # 基準 mid
rb = bd / 2 - ob
circ = 2 * math.pi * rb
chk("branch_t=700・branch_D=610: 基準の面の半径 < 0", rb < 0, f"r = {rb}")
chk("周の長さ < 0（abs で隠れていた値）", circ < 0, f"circ = {circ:.6f}")
print(f"  期待値（test_build の負の周）: {circ:.6f}")
# 境目: branch_t = branch_D/2 で内半径（d/2 - branch_t）が 0。基準 inner なら基準の面の半径も 0
bt_edge = bd / 2
chk("境目 branch_t = branch_D/2 で内半径 0", bd / 2 - bt_edge == 0, f"ri = {bd / 2 - bt_edge}（cbSetup の S.ri >= 0 と同じ境目）")

print()
print("=== M2/M3 えび曲げ: 板厚・外径が 0 以下 ===")
t_neg = -5.0
o_neg = t_neg / 2
R_mid = 2641.6 / 2 - o_neg
chk("t=-5: 中心の半径が外半径より大きい", R_mid > 2641.6 / 2, f"中心 {R_mid} > 外 {2641.6 / 2}")
area_example = 8248.566 * 1000.0  # 周×長さの例（mm²）
mass_neg = area_example * t_neg * 7.85e-6
chk("t=-5: 重さが負", mass_neg < 0, f"例の重さ {mass_neg:.3f} kg")
D0 = 0.0
r0 = D0 / 2 - 16.0 / 2
chk("D=0: 周の長さ ≤ 0", 2 * math.pi * r0 <= 0, f"r = {r0}  circ = {2 * math.pi * r0:.3f}")

print()
print("=== H1/H2/M1/L4 切り口の数: 1 以上の整数だけ通す ===")
for J in [0, -1, 2.5, float("nan"), 1, 2, 3]:
    ok = isinstance(J, int) and J >= 1
    print(f"  joints = {J!r:>6} → {'通す' if ok else 'bend_bad（止める）'}")
chk("NaN は整数でない", not (isinstance(float("nan"), int)))

print()
print("=== L2 円すいに立つ枝: 円すいが止まれば枝も止まる（期待される片の数） ===")
print("  branch_x = -50（穴が口からはみ出す・kind mouth）:")
print("    直す前: 円すい 0 片（tee_bad）・枝 1 片 → 合計 1 片（見直しの実測）")
print("    直した後: 円すい 0 片・枝 0 片（tee_bad） → 合計 0 片")
print("  ※ 片の数は止めの論理から決まる整数。幾何の期待値ではない")

print()
print("合わない所 " + str(NG) + " 件" if NG else "全部合った")
raise SystemExit(1 if NG else 0)