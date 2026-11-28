# T字管の展開の期待値を、画面の計算（shared.js）と別に出す。python verify_tee.py
import math, sys
sys.stdout.reconfigure(encoding="utf-8")
asin, sqrt, pi = math.asin, math.sqrt, math.pi

def cut_y(Rc, ri, ro, e, th):
    # 枝の壁（ri..ro）の中で一番高く当たる点。Rc は当たる本管の面
    a, b = e + ri * math.sin(th), e + ro * math.sin(th)
    m = 0.0 if a * b <= 0 else min(abs(a), abs(b))
    return sqrt(Rc * Rc - m * m)

def show(name, v): print(f"{name}: {v:.2f}")

# 例1: 本管 外径1016・板厚12、枝 外径610・板厚9、補強板 板厚12・平面図の外径1000、H=900、本管の長さ1500
Ro, T = 508.0, 12.0; Ri, Rm = Ro - T, Ro - T / 2
ro, t = 305.0, 9.0; ri, rm = ro - t, ro - t / 2
H, L, tp, Dp = 900.0, 1500.0, 12.0, 1000.0
print("== 例1 載せる（set-on） ==")
show("枝の板の幅 2π·rm", 2 * pi * rm)
for deg in (0, 22.5, 45, 67.5, 90):
    th = math.radians(deg); y = cut_y(Ro, ri, ro, 0, th)
    print(f"  θ={deg:>5}°  当たる高さ {y:.2f}  枝の端から切り口まで {H - y:.2f}")
show("本管の穴 周方向の全幅（内面で枝の内径に合わせ、板厚中心で測る）", 2 * Rm * asin(ri / Ri))
show("本管の穴 軸方向の全幅", 2 * ri)
show("本管の板の幅 2π·Rm", 2 * pi * Rm)
Rp = Ro + tp / 2
show("補強板 外周の周方向（平面図の円を補強板の外面で）", 2 * Rp * asin((Dp / 2) / (Ro + tp)))
show("補強板 外周の周方向（補強板の内面＝本管の外面で）", 2 * Rp * asin((Dp / 2) / Ro))
show("補強板 穴の周方向（枝の外面が本管の外面に当たる線）", 2 * Rp * asin(ro / Ro))
print("== 例1 差し込む（set-in・枝の端は本管の内面にそろえる） ==")
for deg in (0, 45, 90):
    th = math.radians(deg); y = cut_y(Ri, ri, ro, 0, th)
    print(f"  θ={deg:>5}°  当たる高さ {y:.2f}  枝の端から切り口まで {H - y:.2f}")
show("本管の穴 周方向の全幅（内面で枝の外径に合わせる）", 2 * Rm * asin(ro / Ri))
print("== 例2 同径（1016・12 どうし）載せる ==")
for deg in (0, 22.5, 45, 67.5, 90):
    th = math.radians(deg); print(f"  θ={deg:>5}°  当たる高さ {cut_y(Ro, Ri, Ro, 0, th):.2f}")
print("== 例3 片寄せ e=150（例1の寸法）載せる ==")
for deg in (-90, 0, 90):
    th = math.radians(deg); print(f"  θ={deg:>5}°  当たる高さ {cut_y(Ro, ri, ro, 150, th):.2f}")
