# usage: python3 tools/sheet.py <framesdir> <lang> <times> <out.jpg> [cols=4] [tile_w=480]
#   <times> is a range "t0-t1" (one tile per whole second) or a comma list "t1,t2,...". Contact sheet, labelled with film time.
import sys, glob, os
from PIL import Image, ImageDraw, ImageFont
d, lang, spec, out = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
cols = int(sys.argv[5]) if len(sys.argv) > 5 else 4; tw = int(sys.argv[6]) if len(sys.argv) > 6 else 480
th = tw * 9 // 16; lab = 30
if ',' in spec or '-' not in spec: times = [float(x) for x in spec.split(',')]
else:
    a, b = spec.split('-'); times = [float(a) + i for i in range(int(round(float(b) - float(a))) + 1)]
rows = (len(times) + cols - 1) // cols
sheet = Image.new('RGB', (cols * (tw + 8) + 8, rows * (th + lab + 8) + 8), (40, 40, 40))
try: font = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf', 20)
except Exception: font = ImageFont.load_default()
for i, t in enumerate(times):
    f = glob.glob(os.path.join(d, f'{lang}-{t:.2f}.*'))
    x, y = 8 + (i % cols) * (tw + 8), 8 + (i // cols) * (th + lab + 8)
    dr = ImageDraw.Draw(sheet); dr.text((x + 4, y + 4), f'{t:.2f} s', fill=(255, 230, 0), font=font)
    if f:
        im = Image.open(f[0]).convert('RGB').resize((tw, th), Image.LANCZOS); sheet.paste(im, (x, y + lab))
sheet.save(out, quality=84)
print('sheet', out, sheet.size, len(times), 'tiles')
