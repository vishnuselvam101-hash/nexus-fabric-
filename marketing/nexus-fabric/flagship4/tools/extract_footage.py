#!/usr/bin/env python3
"""Rebuilds single captured frames of the real application from flagship3's lossless tile atlases (read only).

  python3 tools/extract_footage.py <lang> <take>:<frame> [<take>:<frame> ...]

flagship3/capture/footage/<lang>/footage.js holds, per take, the tile ids of every captured frame (the first frame in full,
later frames as changed tiles); a<k>.webp are 16 x 16 atlases of 240 px tiles. A frame put back together from them is
bit-identical to the capture (flagship3/capture/pack.py --verify). Output: footage/<lang>/<take>-<frame>.webp, lossless,
3840 x 2160 (device scale 2 of the 1920 x 1080 application window). Pixels are not altered.
"""
import json, os, sys
from PIL import Image
HERE = os.path.dirname(os.path.abspath(__file__)); F = os.path.dirname(HERE)
SRC = os.path.join(F, '..', 'flagship3', 'capture', 'footage')
lang = sys.argv[1]
txt = open(os.path.join(SRC, lang, 'footage.js'), encoding='utf-8').read()
d = json.loads(txt[txt.index('{"lang"'):].rstrip().rstrip(';'))
TS, (GX, GY), A = d['tile'], d['grid'], d['atlas']
atl = {}
def atlas(k):
    if k not in atl: atl[k] = Image.open(os.path.join(SRC, lang, f"a{k}.{d['fmt']}")).convert('RGB')
    return atl[k]
os.makedirs(os.path.join(F, 'footage', lang), exist_ok=True)
for spec in sys.argv[2:]:
    take, f = spec.split(':'); f = int(f); rows = d['takes'][take]['tiles']
    cur = list(rows[0])
    for r in rows[1:f + 1]:
        for k in range(0, len(r), 2): cur[r[k]] = r[k + 1]
    im = Image.new('RGB', (GX * TS, GY * TS))
    per = A * A
    for g, tid in enumerate(cur):
        j = tid % per; a = atlas(tid // per)
        tile = a.crop(((j % A) * TS, (j // A) * TS, (j % A) * TS + TS, (j // A) * TS + TS))
        im.paste(tile, ((g % GX) * TS, (g // GX) * TS))
    out = os.path.join(F, 'footage', lang, f'{take}-{f}.webp')
    im.save(out, lossless=True, quality=100, method=6)
    print(out, im.size, os.path.getsize(out))
