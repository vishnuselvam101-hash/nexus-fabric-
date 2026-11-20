#!/usr/bin/env python3
"""Contact sheet of chosen capture frames: sheet.py <lang> <out.png> take:idx take:idx ...  (idx may be a mark name)"""
import json, sys, os
from PIL import Image, ImageDraw
HERE = os.path.dirname(os.path.abspath(__file__))
lang, out, items = sys.argv[1], sys.argv[2], sys.argv[3:]
man = json.load(open(os.path.join(HERE, "frames", lang, "manifest.json")))
W, Hh, cols = 640, 360, int(os.environ.get("COLS", 4))
rows = (len(items) + cols - 1) // cols
sheet = Image.new("RGB", (cols * W, rows * Hh), (20, 20, 20)); d = ImageDraw.Draw(sheet)
for k, it in enumerate(items):
    take, idx = it.split(":", 1); t = man["takes"][take]
    off = 0
    if "+" in idx: idx, o = idx.rsplit("+", 1); off = int(o)
    i = (t["marks"][idx] if idx in t["marks"] else int(idx)) + off
    fr = t["frames"][min(i, len(t["frames"]) - 1)]
    im = Image.open(os.path.join(HERE, "frames", lang, fr["f"])).convert("RGB").resize((W, Hh), Image.LANCZOS)
    x, y = (k % cols) * W, (k // cols) * Hh; sheet.paste(im, (x, y))
    cx, cy = fr["c"][0] / 3, fr["c"][1] / 3
    d.ellipse([x + cx - 4, y + cy - 4, x + cx + 4, y + cy + 4], outline=(255, 60, 60), width=2)
    d.rectangle([x, y, x + 200, y + 14], fill=(0, 0, 0)); d.text((x + 3, y + 1), f"{take}:{i} {fr['c'][3]}", fill=(255, 255, 0))
sheet.save(out)
