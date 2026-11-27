"""Compare verify_miterbend.py with ZK's mitered bend (black box through node). Lists differences; fixes neither side.
usage: python3 -I compare_zk_bend.py /root/zumen-kirizu"""
import importlib.util, json, math, os, subprocess, sys
here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("vm", os.path.join(here, "..", "verify_miterbend.py"))
vm = importlib.util.module_from_spec(spec); spec.loader.exec_module(vm)
zk = sys.argv[1]
RHO = 7.85e-6
cases = []
for (D, t, R, A, J, s1, s2) in [(1016, 12, 1500, 90, 2, 0, 0), (1016, 12, 1500, 90, 3, 0, 0), (1016, 12, 1500, 90, 1, 0, 0), (609.6, 9, 914.4, 45, 2, 300, 250),
                                (2641.6, 16, 2600, 90, 3, 300, 300), (508, 8, 400, 30, 1, 0, 100), (1524, 12, 1500, 60, 2, 500, 400), (500, 6, 750, 15, 1, 0, 0), (800, 10, 640, 120, 4, 0, 0)]:
    for basis in ("mid", "inner", "outer"):
        for allow in (0, 4):
            cases.append(dict(D=D, t=t, R=R, A=A, J=J, s1=s1, s2=s2, basis=basis, allow=allow, amp="outer"))
o = subprocess.run(["node", os.path.join(here, "zk_bend_dump.mjs"), zk, json.dumps(cases)], capture_output=True, text=True)
if not o.stdout: print(o.stderr); sys.exit(1)
res = json.loads(o.stdout)
SWING = sys.argv[2] if len(sys.argv) > 2 else "outer"  # bend_swing_radius of the spec side: basis | outer (ZK = outer)
diffs = {"w": [], "h": [], "kg": [], "refusal": [], "pieces": []}
n_cmp = 0
for r in res:
    c = r["case"]; n = c["J"] + 1
    if r["needs"]:
        diffs["refusal"].append((c, r["needs"])); continue
    b = vm.Bend(c["D"], c["t"], c["R"], c["A"], n, c["basis"], c["s1"], c["s2"], c["allow"], vm.SEAM_DEG, SWING)
    # ZK reports one row per piece kind (start / middle xN / end)
    mine = [(1, "始点側", 1), (2, "中間", max(0, n - 2)), (n, "終点側", 1)]
    mine = [m for m in mine if m[2] > 0] if n > 2 else [(1, "始点側", 1), (n, "終点側", 1)]
    if len(mine) != len(r["pieces"]): diffs["pieces"].append((c, len(mine), len(r["pieces"]))); continue
    for (k, label, cnt), p in zip(mine, r["pieces"]):
        n_cmp += 1
        w = 2 * math.pi * b.r
        hmax = b.gen_len(k, 0.0)
        kg = w * b.centre_length(k) * c["t"] * RHO * cnt
        if abs(p["w"] - w) > 0.1: diffs["w"].append((c["basis"], c["D"], c["t"], label, round(w, 1), p["w"]))
        hamp = b.centre_length(k) + c["D"] / 2 * (b.cuts(k)[0] + b.cuts(k)[1])
        if abs(p["h"] - hmax) > 0.1: diffs["h"].append((c["basis"], c["D"], c["t"], c["allow"], label, round(hmax, 1), p["h"], round(p["h"] - hmax, 1)))
        if abs(p["kg"] - kg) > max(0.05, 5e-4 * kg): diffs["kg"].append((c["basis"], c["D"], c["t"], c["allow"], label, round(kg, 2), p["kg"]))
print(f"spec side bend_swing_radius = {SWING}; seam {vm.SEAM_DEG} deg, allow per piece {vm.ALLOW_PER_PIECE_MM} mm (compare sets allow 0 and 4)")
print(f"compared {n_cmp} pieces in {len(res)} cases (width, max generatrix, mass)")
for k, v in diffs.items(): print(f"differences {k}: {len(v)}")
for k in ("w", "h", "kg"):
    for d in diffs[k][:10]: print(" ", k, d)
for d in diffs["refusal"]: print("  ZK refuses:", d[0], d[1])
for d in diffs["pieces"]: print("  piece count differs:", d)

# cut-curve parity: ZK's points (lower and upper edge of a middle piece) against the spec sheet at the default seam
o = subprocess.run(["node", os.path.join(here, "zk_bend_curve.mjs"), zk, json.dumps([dict(D=1016, t=12, R=1500, A=90, J=2, basis=bs) for bs in ("mid", "inner", "outer")])], capture_output=True, text=True)
if not o.stdout: print(o.stderr); sys.exit(1)
for item in json.loads(o.stdout):
    c = item["case"]; pts = item["pts"]; n = c["J"] + 1
    for swing in ("basis", "outer"):
        b = vm.Bend(c["D"], c["t"], c["R"], c["A"], n, c["basis"], allow=0.0, seam_deg=vm.SEAM_DEG, swing=swing)
        lower, upper = pts[:361], pts[361:722]
        err = 0.0
        for x, y in lower:
            err = max(err, abs(y - b.edge_curves(2, b.seam + x / b.r)[0]))
        for x, y in upper:
            err = max(err, abs(y - b.edge_curves(2, b.seam + x / b.r)[1]))
        print(f"curve parity basis {c['basis']} swing {swing}: max |ZK - spec| = {err:.3g} mm over {len(pts)} points")
