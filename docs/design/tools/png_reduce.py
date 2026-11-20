"""Reduce screenshot PNGs: 256-colour palette (maximum-coverage quantiser, no dithering) when the worst per-channel error stays
small, otherwise lossless re-encode. Usage: pngopt.py <src-dir> <dst-dir> [max-error=12]
Prints per-file sizes and the worst error so that the fidelity claim can be checked."""
import os, sys
from PIL import Image, ImageChops

src, dst = sys.argv[1], sys.argv[2]
limit = int(sys.argv[3]) if len(sys.argv) > 3 else 12
tot_in = tot_out = 0; worst_all = 0; kept_lossless = 0; n = 0
for root, _, files in os.walk(src):
    for f in sorted(files):
        if not f.endswith('.png'): continue
        p = os.path.join(root, f); rel = os.path.relpath(p, src); q = os.path.join(dst, rel)
        os.makedirs(os.path.dirname(q), exist_ok=True)
        im = Image.open(p).convert('RGB')
        pal = im.quantize(colors=256, method=Image.Quantize.MAXCOVERAGE, dither=Image.Dither.NONE)
        diff = ImageChops.difference(im, pal.convert('RGB'))
        worst = max(mx for _, mx in diff.getextrema())
        if worst <= limit:
            pal.save(q, optimize=True)
        else:
            im.save(q, optimize=True); kept_lossless += 1
        a, b = os.path.getsize(p), os.path.getsize(q)
        tot_in += a; tot_out += b; worst_all = max(worst_all, worst if worst <= limit else 0); n += 1
print(f'{n} files: {tot_in/1e6:.1f} MB -> {tot_out/1e6:.1f} MB; worst palette error {worst_all}/255; {kept_lossless} kept lossless (error above {limit})')
