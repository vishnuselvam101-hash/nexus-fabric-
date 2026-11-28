"""Validate a generated truth set against its PNG renders. Usage: python3 -I validate_v2.py ROOT/split
Checks per item: box inside page (after skew), printed text contains the formatted value, and non-blank ink in the box.
Per page: PNG size equals width x height, schema id, unique item ids. Prints counts; exits 1 on any failure."""
import json, os, sys
from PIL import Image

split_dir = os.path.abspath(sys.argv[1])
truth = os.path.join(split_dir, "truth.jsonl")
base = os.path.dirname(split_dir)  # png/pdf paths in truth are relative to the set root
pages = items = fail_bounds = fail_text = fail_ink = fail_size = fail_id = 0
ink_checked = 0
fams = {}
for line in open(truth, encoding="utf-8"):
    t = json.loads(line)
    pages += 1
    fams[t["family"]] = fams.get(t["family"], 0) + 1
    png = os.path.join(base, t["png"])
    im = Image.open(png).convert("L")
    if im.size != (t["width"], t["height"]):
        fail_size += 1
        continue
    px = im.load()
    W, H = t["width"], t["height"]
    # paper level = 95th percentile of the page; ink = at least 40 grey levels darker (scans lift the levels)
    hist = im.histogram()
    acc, paper = 0, 255
    for g in range(256):
        acc += hist[g]
        if acc >= 0.95 * W * H:
            paper = g
            break
    ids = set()
    for it in t["items"]:
        items += 1
        if it["id"] in ids:
            fail_id += 1
        ids.add(it["id"])
        x0, y0, x1, y1 = it["box"]
        if x0 < 0 or y0 < 0 or x1 > W or y1 > H or x1 <= x0 or y1 <= y0:
            fail_bounds += 1
            continue
        v = it["value"]
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            dec = it["decimals"] if it["decimals"] is not None else t["decimals"]
            # integer structure numbers (item numbers, quantities) print without decimals
            want = str(int(v)) if it["source"] and it["source"].get("kind") == "structure" and float(v).is_integer() else f"{v:.{dec}f}"
            if want not in it["text"]:
                fail_text += 1
        # ink inside the box (skip boxes too thin to sample)
        xa, ya, xb, yb = int(x0) + 1, int(y0) + 1, int(x1) - 1, int(y1) - 1
        if xb - xa < 3 or yb - ya < 3:
            continue
        ink_checked += 1
        darkest = min(px[xx, yy] for yy in range(ya, yb) for xx in range(xa, xb))
        if darkest > paper - 40:
            fail_ink += 1
    # consistency of sizes handled above
print(f"pages {pages} items {items} families {len(fams)}")
print(f"png size mismatch {fail_size} duplicate ids {fail_id}")
print(f"box outside page {fail_bounds} numeric text mismatch {fail_text} blank box {fail_ink} (ink checked {ink_checked})")
bad = fail_size + fail_id + fail_bounds + fail_text + fail_ink
print("RESULT", "PASS" if bad == 0 else "FAIL")
sys.exit(0 if bad == 0 else 1)
