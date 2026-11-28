"""Downscale a v2 dataset split to a 1024 px long side. Usage: python3 -I downscale_v2.py SRC_SPLIT_DIR DST_DIR
Reads SRC/truth.jsonl and SRC/png, writes DST/truth.jsonl and DST/png/<name>.png. Item boxes and clutter boxes are
scaled; pdf and request are dropped; orig_width/orig_height/scale are recorded (the v1b downscale layout)."""
import json, os, sys
from PIL import Image

src, dst = sys.argv[1], sys.argv[2]
L = 1024
os.makedirs(dst + "/png", exist_ok=True)
with open(dst + "/truth.jsonl", "w") as out:
    for line in open(src + "/truth.jsonl"):
        r = json.loads(line)
        im = Image.open(os.path.join(os.path.dirname(src), r["png"])).convert("RGB")
        w, h = im.size
        s = L / max(w, h)
        nw, nh = round(w * s), round(h * s)
        name = os.path.basename(r["png"])
        im.resize((nw, nh), Image.LANCZOS).save(dst + "/png/" + name, optimize=True)
        r["png"] = "png/" + name
        r.pop("pdf", None)
        r.pop("request", None)
        r["orig_width"], r["orig_height"] = w, h
        r["width"], r["height"] = nw, nh
        r["scale"] = s
        for it in r["items"] + r.get("clutter", []):
            it["box"] = [round(v * s, 1) for v in it["box"]]
            it.pop("box_page", None)
        out.write(json.dumps(r, ensure_ascii=False, separators=(",", ":")) + "\n")
