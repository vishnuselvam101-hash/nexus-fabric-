#!/usr/bin/env python3
"""Keeps only the Noto Sans JP subsets that cover a character the film can set, and rewrites fonts/fonts.css.

  python3 tools/prune_fonts.py      (run after any change of on-screen text; then compare a Japanese still)

Characters: every character above U+2E7F in script/onscreen4.js, data/film4-data.js and film/*.js, plus common
punctuation. Inter (Latin) and JetBrains Mono are always kept. Text inside the captured footage is an image.
"""
import os, re, glob
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE); FD = os.path.join(ROOT, 'fonts')
text = ''.join(open(p, encoding='utf-8').read() for p in [os.path.join(ROOT, 'script', 'onscreen4.js'), os.path.join(ROOT, 'data', 'film4-data.js')] + glob.glob(os.path.join(ROOT, 'film', '*.js')))
need = set(ch for ch in text if ord(ch) > 0x2E7F) | set('・：、。「」（）→·—–')
def ranges(spec):
    out = []
    for part in spec.split(','):
        part = part.strip().removeprefix('U+')
        a, _, b = part.partition('-'); out.append((int(a, 16), int(b.removeprefix('U+') or a, 16)))
    return out
css = open(os.path.join(FD, 'fonts.css'), encoding='utf-8').read()
head = css.split('@font-face')[0]
blocks = re.findall(r'@font-face\s*{[^}]*}', css)
keep, drop = [], []
for b in blocks:
    m = re.search(r"url\(\./([^)]+)\)", b); rng = re.search(r'unicode-range:\s*([^;]+);', b)
    if 'Noto Sans JP' not in b or not rng: keep.append(b); continue
    rs = ranges(rng.group(1))
    (keep if any(any(a <= ord(ch) <= z for a, z in rs) for ch in need) else drop).append(b)
for b in drop:
    f = re.search(r"url\(\./([^)]+)\)", b).group(1); p = os.path.join(FD, f)
    if os.path.exists(p): os.remove(p)
open(os.path.join(FD, 'fonts.css'), 'w', encoding='utf-8').write(head + '\n'.join(keep) + '\n')
print(f'needed characters {len(need)}; kept {len(keep)} faces, removed {len(drop)}')
