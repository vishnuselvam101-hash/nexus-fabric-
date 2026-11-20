#!/usr/bin/env python3
"""Pack captured frames for the film page.

  python capture/pack.py <lang> [--frames <dir>] [--fmt webp|png|avif] [--tile 240] [--stats] [--verify]

Reads   capture/frames/<lang>/<take>/NNNNN.png + manifest.json + job.json + downloads/   (or --frames <that dir>)
Writes  capture/footage/<lang>/a<k>.<fmt>   atlases of the unique 240 px tiles (lossless WebP by default)
        capture/footage/<lang>/footage.js   window.NF_FOOTAGE[<lang>] = {manifest, tile table, job data, DXF text}

A product frame is never altered: it is cut into tiles on a fixed grid, identical tiles are stored once, and the
film page puts the tiles back at their own positions. With lossless WebP the reassembled frame is bit-identical
to the capture (pack.py --verify checks this).
"""
import hashlib, json, os, sys
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
lang = sys.argv[1]
opt = lambda k, d=None: (sys.argv[sys.argv.index("--" + k) + 1] if "--" + k in sys.argv else d)
FMT = opt("fmt", "webp"); TS = int(opt("tile", 240)); ATLAS = 16            # atlas = 16 x 16 tiles
SRC = os.path.abspath(opt("frames", os.path.join(HERE, "frames", lang))); DST = os.path.join(HERE, "footage", lang)
man = json.load(open(os.path.join(SRC, "manifest.json"), encoding="utf-8"))
# the form job (number 5) as the product returned it: from the product bundle the capture wrote (job.json is no longer written)
job = json.load(open(os.path.join(SRC, "product.json"), encoding="utf-8"))["jobs"]["5"]
W, H = man["viewport"][0] * man["dsf"], man["viewport"][1] * man["dsf"]
assert W % TS == 0 and H % TS == 0, "tile size must divide the frame"
GX, GY = W // TS, H // TS

ids = {}; tiles = []                       # hash -> id ; id -> array
def tile_ids(img):
    a = np.asarray(img.convert("RGB")); out = []
    for gy in range(GY):
        for gx in range(GX):
            t = a[gy * TS:(gy + 1) * TS, gx * TS:(gx + 1) * TS]
            h = hashlib.blake2b(t.tobytes(), digest_size=12).digest()
            if h not in ids:
                ids[h] = len(tiles); tiles.append(t.copy())
            out.append(ids[h])
    return out

frames_out = {}; total = 0
for name, take in man["takes"].items():
    rows = []; prev = None
    for fr in take["frames"]:
        t = tile_ids(Image.open(os.path.join(SRC, fr["f"]))); total += 1
        # store a frame as the list of (grid index, tile id) that differ from the previous frame of the take
        rows.append(t if prev is None else [v for k, (p, q) in enumerate(zip(prev, t)) if p != q for v in (k, q)]); prev = t
    frames_out[name] = rows
print(f"{lang}: {total} frames, {len(tiles)} unique tiles of {total * GX * GY} ({100 * len(tiles) / (total * GX * GY):.2f} %)")
if "--stats" in sys.argv:
    for name, rows in frames_out.items():
        print(f"  {name:8s} frames {len(rows):4d}  changed tiles after the first frame: {sum(len(r) // 2 for r in rows[1:])}")

os.makedirs(DST, exist_ok=True)
for f in os.listdir(DST):
    os.remove(os.path.join(DST, f))
per = ATLAS * ATLAS; n_atlas = (len(tiles) + per - 1) // per; size = 0
for k in range(n_atlas):
    chunk = tiles[k * per:(k + 1) * per]; rows_n = (len(chunk) + ATLAS - 1) // ATLAS
    at = np.zeros((rows_n * TS, ATLAS * TS, 3), np.uint8)
    for j, t in enumerate(chunk):
        at[(j // ATLAS) * TS:(j // ATLAS + 1) * TS, (j % ATLAS) * TS:(j % ATLAS + 1) * TS] = t
    p = os.path.join(DST, f"a{k}.{FMT}"); im = Image.fromarray(at)
    if FMT == "webp":
        im.save(p, lossless=True, quality=100, method=int(opt("webp-method", 4)))   # lossless at any method; 4 is ~20x faster than 6 on these atlases
    elif FMT == "avif":
        im.save(p, quality=int(opt("q", 92)), subsampling="4:4:4", speed=4)
    else:
        im.save(p, optimize=True)
    size += os.path.getsize(p)
print(f"  {n_atlas} atlases, {size / 1e6:.1f} MB ({FMT})")

# ---- manifest for the film: per take the marks, per frame pointer / rects / view / read-out, and the tile deltas
def slim(take, rows):
    fr = take["frames"]; keys = sorted({k for f in fr for k in (f.get("r") or {})})
    out = {"n": len(fr), "marks": take["marks"], "tiles": rows, "c": [f["c"] for f in fr], "keys": keys, "r": [], "v": [], "ro": [], "z": []}
    last = None
    for f in fr:                                           # rects change rarely: store only when they differ
        r = [f["r"].get(k) for k in keys]; out["r"].append(None if r == last else r); last = r
        out["v"].append(f.get("v")); out["ro"].append(f.get("ro")); out["z"].append(f.get("z"))
    for k in ("sketch0", "sketch1", "server_ms", "busy_frames", "hole_ci", "saddle_ci", "saddle_n", "rows", "rows_all", "iface_row", "gate_rows", "gate_rows_end", "record_count"):
        if k in take:
            out[k] = take[k]
    if any("vi" in f for f in fr):
        out["vi"] = [f.get("vi") for f in fr]
    return out

def read(p):
    return open(p, encoding="utf-8").read() if os.path.exists(p) else None

pat = {}
for p in job.get("patterns", []):
    pat[p["part"]] = {"curves": [{"name": c["name"], "layer": c.get("layer"), "length": c["length"], "points": c["points"]} for c in p["curves"]],
                      "holes": [{"name": c["name"], "layer": c.get("layer"), "length": c["length"], "points": c["points"]} for c in p.get("holes", [])],
                      "quantities": p.get("quantities"), "marking_side": p.get("marking_side"), "dxf_sha256": p.get("dxf_sha256"), "class": p.get("class"),
                      "eps_num": p.get("eps_num"), "eps_rep": p.get("eps_rep")}
counts = {}
for g in job.get("gates", []):
    counts[g["result"]] = counts.get(g["result"], 0) + 1
data = {
    "lang": lang, "html_lang": man.get("html_lang"), "lang_applied": man.get("lang_applied"), "notes": man.get("notes"), "engine": man.get("engine"), "job_id": man.get("job_id"),
    "viewport": man["viewport"], "dsf": man["dsf"], "tile": TS, "grid": [GX, GY], "atlas": ATLAS, "fmt": FMT, "atlases": n_atlas, "tiles": len(tiles),
    "takes": {name: slim(take, frames_out[name]) for name, take in man["takes"].items()},
    "job": {"request": job.get("request"), "state": job.get("state"), "engine": job.get("engine"), "gates": [{"id": g["id"], "name": g["name"], "result": g["result"]} for g in job.get("gates", [])],
            "gate_counts": counts, "checks": job.get("checks"), "records": len(job.get("records", [])), "closure_hash": job.get("closure_hash"), "replay": job.get("replay"), "patterns": pat},
    "dxf": {part: {**meta, "text": read(os.path.join(SRC, "downloads", "api_" + part + ".dxf"))} for part, meta in (man.get("dxf") or {}).items()},
    "downloads": man.get("downloads"),
}
# the product bundle written by capture.js: every job, graph, obligation, check, gate and DXF text the film reads
if os.path.exists(os.path.join(SRC, "product.json")):
    data["product"] = json.load(open(os.path.join(SRC, "product.json"), encoding="utf-8"))
js = "window.NF_FOOTAGE = window.NF_FOOTAGE || {}; window.NF_FOOTAGE[%s] = %s;\n" % (json.dumps(lang), json.dumps(data, ensure_ascii=False, separators=(",", ":")))
open(os.path.join(DST, "footage.js"), "w", encoding="utf-8").write(js)
print(f"  footage.js {len(js) / 1e6:.2f} MB; folder {sum(os.path.getsize(os.path.join(DST, f)) for f in os.listdir(DST)) / 1e6:.1f} MB")

if "--verify" in sys.argv:                                  # rebuild every frame from the atlases and compare with the capture
    atl = [np.asarray(Image.open(os.path.join(DST, f"a{k}.{FMT}")).convert("RGB")) for k in range(n_atlas)]
    def tile(i):
        a = atl[i // per]; j = i % per
        return a[(j // ATLAS) * TS:(j // ATLAS + 1) * TS, (j % ATLAS) * TS:(j % ATLAS + 1) * TS]
    worst = 0; bad = 0
    for name, take in man["takes"].items():
        cur = None
        for fr, row in zip(take["frames"], frames_out[name]):
            cur = list(row) if cur is None else cur
            if cur is not row:
                for k in range(0, len(row), 2):
                    cur[row[k]] = row[k + 1]
            src = np.asarray(Image.open(os.path.join(SRC, fr["f"])).convert("RGB")).astype(np.int16)
            for g, tid in enumerate(cur):
                gy, gx = divmod(g, GX)
                d = np.abs(src[gy * TS:(gy + 1) * TS, gx * TS:(gx + 1) * TS] - tile(tid)).max()
                worst = max(worst, int(d)); bad += d > 0
    print(f"  verify: largest pixel difference {worst}; tiles that differ {bad}")
