# 円すいを n 枚に割ったときの期待値を、画面の計算（shared.js）と別に出す。python verify_cone_split.py
import math, sys
import numpy as np
sys.stdout.reconfigure(encoding="utf-8")
# 同心: 大径 1200・小径 800・長さ 600（板厚ほぼ0）
R1, R2, H = 600.0, 400.0, 600.0
sl = math.hypot(H, R1 - R2); sinA = (R1 - R2) / sl; L1, L2 = R1 / sinA, R2 / sinA
for n in (2, 3, 4):
    ang = 2 * math.pi * sinA / n
    print(f"同心 n={n}: 1枚の角 {math.degrees(ang):.3f}°  弦 {2*L1*math.sin(ang/2):.2f}  高さ {L1 - L2*math.cos(ang/2):.2f}  外の弧 {L1*ang:.2f}（π·D1/n={math.pi*2*R1/n:.2f}）")
# 偏心: 大径 1200・小径 800・長さ 700・片寄せ 200（平らな側を継ぎ目の始まりに）
def oblique(R1, R2, H, off, n=36000):
    phi = np.linspace(0, 2 * np.pi, n + 1); k = R2 / R1
    A = np.array([off, 0, H]) / (1 - k)
    P1 = np.stack([R1 * np.cos(phi), R1 * np.sin(phi), 0 * phi], 1)
    U = P1 - A; U = U / np.linalg.norm(U, axis=1)[:, None]
    psi = np.concatenate([[0], np.cumsum(np.arccos(np.clip(np.sum(U[1:] * U[:-1], 1), -1, 1)))])
    return phi, psi
phi, psi = oblique(600, 400, 700, 200)
for n in (2, 3, 4):
    idx = [j * (len(phi) - 1) // n for j in range(n + 1)]
    print(f"偏心 n={n}: 各片の展開角 {[round(float(np.degrees(psi[idx[j+1]] - psi[idx[j]])), 3) for j in range(n)]}")
