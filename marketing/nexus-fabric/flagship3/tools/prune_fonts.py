#!/usr/bin/env python3
"""Keeps only the Noto Sans JP subsets that cover a character the film sets, and rewrites fonts/fonts.css to match.

  python3 tools/prune_fonts.py          (run after any change of on-screen text; then re-render a still to compare)

The characters are read from script/onscreen.js, data/check-labels.js and the Japanese literals of film.js. Inter (Latin and Greek)
is always kept. A face whose unicode-range covers no needed character is removed (its file too). The product's own text inside the
captured footage is an image and needs no font.
"""
import os, re, json, subprocess
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.join(HERE, '..'); FD = os.path.join(ROOT, 'fonts')
text = open(os.path.join(ROOT, 'script', 'onscreen.js'), encoding='utf-8').read() + open(os.path.join(ROOT, 'data', 'check-labels.js'), encoding='utf-8').read() + open(os.path.join(ROOT, 'film.js'), encoding='utf-8').read()
need = set(ch for ch in text if ord(ch) > 0x2E7F)                     # the CJK and kana the film may set
need |= set('0123456789.,:;-–—()[]/%·°²×≈φβ±√')
def ranges(spec):
    out = []
    for part in spec.split(','):
        part = part.strip().removeprefix('U+')
        if '-' in part:
            a, b = part.split('-'); out.append((int(a, 16), int(b.removeprefix('U+'), 16)))
        else:
            out.append((int(part, 16), int(part, 16)))
    return out
css = open(os.path.join(FD, 'fonts.css'), encoding='utf-8').read()
blocks = re.findall(r'@font-face\s*{[^}]*}', css)
keep, drop = [], []
for b in blocks:
    m = re.search(r"url\(\./([^)]+\.woff2)\)", b); rng = re.search(r'unicode-range:\s*([^;]+);', b)
    fam = re.search(r"font-family:\s*'([^']+)'", b)
    if not m:
        keep.append(b); continue
    if fam and fam.group(1) == 'Inter Variable':
        keep.append(b); continue
    spans = ranges(rng.group(1)) if rng else [(0, 0x10FFFF)]
    hit = any(lo <= ord(ch) <= hi for ch in need for lo, hi in spans)
    (keep if hit else drop).append(b)
    if not hit and os.path.exists(os.path.join(FD, m.group(1))):
        os.remove(os.path.join(FD, m.group(1)))
open(os.path.join(FD, 'fonts.css'), 'w', encoding='utf-8').write('/* pruned by tools/prune_fonts.py to the subsets the film sets; Inter, and the Noto Sans JP subsets covering its characters. SIL OFL 1.1. */\n' + '\n'.join(keep) + '\n')
print('kept', len(keep), 'faces; removed', len(drop), '; characters considered:', len(need))
