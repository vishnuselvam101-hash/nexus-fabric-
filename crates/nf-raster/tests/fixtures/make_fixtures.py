#!/usr/bin/env python3
"""Makes the PDF fixtures of nf-raster from the synthetic sample pages of nf-reader-data
(clean pages, generated, no partner drawings).  Usage:
  make_fixtures.py <reader-sample dir> <out dir>
Needs Pillow with libtiff.  Fixtures are committed; this script documents how they were made."""
import io, os, struct, sys, zlib
from PIL import Image

SRC, OUT = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)


class Pdf:
    def __init__(self):
        self.objs = []  # bytes bodies, object number = index + 1

    def add(self, body):
        self.objs.append(body)
        return len(self.objs)

    def stream(self, d, data):
        return self.add(("<< %s /Length %d >>\nstream\n" % (d, len(data))).encode() + data + b"\nendstream")

    def build(self, root, extra_trailer=""):
        out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
        offs = []
        for i, b in enumerate(self.objs):
            offs.append(len(out))
            out += ("%d 0 obj\n" % (i + 1)).encode() + b + b"\nendobj\n"
        x = len(out)
        out += ("xref\n0 %d\n0000000000 65535 f \n" % (len(self.objs) + 1)).encode()
        for o in offs:
            out += ("%010d 00000 n \n" % o).encode()
        out += ("trailer\n<< /Size %d /Root %d 0 R %s >>\nstartxref\n%d\n%%%%EOF\n" % (len(self.objs) + 1, root, extra_trailer, x)).encode()
        return bytes(out)


def page_pdf(images, mediabox, rotate=0, cropbox=None, extra_trailer="", content_extra=b"", resources_extra=""):
    """images: list of (xobject dict string, data bytes, (a,b,c,d,e,f) placement)."""
    p = Pdf()
    p.add(b"<< /Type /Catalog /Pages 2 0 R >>")
    p.add(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    page_no = p.add(b"")  # patched below
    xnames, content = [], b""
    for i, (d, data, m) in enumerate(images):
        n = p.stream(d, data)
        xnames.append("/Im%d %d 0 R" % (i, n))
        content += b"q %s cm /Im%d Do Q\n" % (" ".join("%g" % v for v in m).encode(), i)
    content += content_extra
    c = p.stream("", content)
    box = "/MediaBox [%s]" % " ".join("%g" % v for v in mediabox)
    if cropbox:
        box += " /CropBox [%s]" % " ".join("%g" % v for v in cropbox)
    if rotate:
        box += " /Rotate %d" % rotate
    p.objs[page_no - 1] = ("<< /Type /Page /Parent 2 0 R %s /Contents %d 0 R /Resources << /XObject << %s >> %s >> >>" % (box, c, " ".join(xnames), resources_extra)).encode()
    return p.build(1, extra_trailer)


def write(name, data):
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(data)
    print(name, len(data))


gray = Image.open(os.path.join(SRC, "png", "sample-00000.png")).convert("L")
W, H = gray.size                      # 2351 x 1662 at 200 dpi
PW, PH = W * 72 / 200, H * 72 / 200
full = (PW, 0, 0, PH, 0, 0)
mb = (0, 0, PW, PH)
bw = gray.point(lambda v: 255 if v >= 128 else 0).convert("1")


def jpeg_bytes(im, q=85):
    b = io.BytesIO(); im.save(b, "JPEG", quality=q); return b.getvalue()


def tiff_strip(im, comp, options=None):
    """CCITT data of a bilevel image as one TIFF strip, made with ImageMagick/libtiff."""
    import subprocess, tempfile
    with tempfile.TemporaryDirectory() as d:
        src, dst = os.path.join(d, "a.png"), os.path.join(d, "a.tif")
        # ImageMagick writes the CCITT runs with the opposite polarity to the PDF default: negate.
        from PIL import ImageOps
        ImageOps.invert(im.convert("L")).save(src)
        cmd = ["convert", src, "-type", "Bilevel", "-compress", comp]
        if options is not None:
            cmd += ["-define", "tiff:group3-options=%d" % options]
        subprocess.run(cmd + [dst], check=True)
        t = Image.open(dst)
        offs, cnts = t.tag_v2[273], t.tag_v2[279]
        if isinstance(offs, int):
            offs, cnts = (offs,), (cnts,)
        assert len(offs) == 1, "more than one strip"
        raw = open(dst, "rb").read()
        return raw[offs[0]:offs[0] + cnts[0]]


# ---- JBIG2 (embedded organisation), generic region, arithmetic coding, template 0 ----
QE = [(0x5601,1,1,1),(0x3401,2,6,0),(0x1801,3,9,0),(0x0AC1,4,12,0),(0x0521,5,29,0),(0x0221,38,33,0),
(0x5601,7,6,1),(0x5401,8,14,0),(0x4801,9,14,0),(0x3801,10,14,0),(0x3001,11,17,0),(0x2401,12,18,0),
(0x1C01,13,20,0),(0x1601,29,21,0),(0x5601,15,14,1),(0x5401,16,14,0),(0x5101,17,15,0),(0x4801,18,16,0),
(0x3801,19,17,0),(0x3401,20,18,0),(0x3001,21,19,0),(0x2801,22,19,0),(0x2401,23,20,0),(0x2201,24,21,0),
(0x1C01,25,22,0),(0x1801,26,23,0),(0x1601,27,24,0),(0x1401,28,25,0),(0x1201,29,26,0),(0x1101,30,27,0),
(0x0AC1,31,28,0),(0x09C1,32,29,0),(0x08A1,33,30,0),(0x0521,34,31,0),(0x0441,35,32,0),(0x02A1,36,33,0),
(0x0221,37,34,0),(0x0141,38,35,0),(0x0111,39,36,0),(0x0085,40,37,0),(0x0049,41,38,0),(0x0025,42,39,0),
(0x0015,43,40,0),(0x0009,44,41,0),(0x0005,45,42,0),(0x0001,45,43,0),(0x5601,46,46,0)]


class MQ:
    def __init__(self):
        self.a, self.c, self.ct = 0x8000, 0, 12
        self.out = bytearray([0])  # out[-1] is B
        self.st = {}

    def byteout(self):
        o = self.out
        if o[-1] == 0xFF:
            o.append((self.c >> 20) & 0xFF); self.c &= 0xFFFFF; self.ct = 7
        elif self.c < 0x8000000:
            o.append((self.c >> 19) & 0xFF); self.c &= 0x7FFFF; self.ct = 8
        else:
            o[-1] += 1
            if o[-1] == 0xFF:
                self.c &= 0x7FFFFFF
                o.append((self.c >> 20) & 0xFF); self.c &= 0xFFFFF; self.ct = 7
            else:
                o.append((self.c >> 19) & 0xFF); self.c &= 0x7FFFF; self.ct = 8

    def renorm(self):
        while True:
            self.a <<= 1; self.c <<= 1; self.ct -= 1
            if self.ct == 0: self.byteout()
            if self.a & 0x8000: break

    def encode(self, cx, d):
        i, mps = self.st.get(cx, (0, 0))
        qe, nmps, nlps, sw = QE[i]
        if d == mps:
            self.a -= qe
            if not (self.a & 0x8000):
                if self.a < qe: self.a = qe
                else: self.c += qe
                self.st[cx] = (nmps, mps); self.renorm()
            else:
                self.c += qe
        else:
            self.a -= qe
            if self.a < qe: self.c += qe
            else: self.a = qe
            if sw: mps = 1 - mps
            self.st[cx] = (nlps, mps); self.renorm()

    def flush(self):
        t = self.c + self.a
        self.c |= 0xFFFF
        if self.c >= t: self.c -= 0x8000
        self.c <<= self.ct; self.byteout()
        self.c <<= self.ct; self.byteout()
        data = bytes(self.out[1:])
        if data and data[-1] == 0xFF: data = data[:-1]
        return data + b"\xff\xac"


def jbig2_generic(im1, mmr_data=None):
    w, h = im1.size
    px = im1.load()
    def black(x, y):
        return 1 if (0 <= x < w and 0 <= y < h and px[x, y] == 0) else 0
    seg = lambda num, typ, data: struct.pack(">IBBBI", num, typ, 0, 1, len(data)) + data
    page = seg(0, 48, struct.pack(">IIIIBH", w, h, 0, 0, 0, 0))
    region = struct.pack(">IIIIB", w, h, 0, 0, 0)
    if mmr_data is not None:
        gen = region + b"\x01" + mmr_data
    else:
        mq = MQ()
        for y in range(h):
            for x in range(w):
                cx = 0
                for dx, dy in ((-1,0),(-2,0),(-3,0),(-4,0),(-2,-1),(-1,-1),(0,-1),(1,-1),(2,-1),
                               (-1,-2),(0,-2),(1,-2),(3,-1),(-3,-1),(2,-2),(-2,-2)):
                    cx = (cx << 1) | black(x + dx, y + dy)
                mq.encode(cx, black(x, y))
        at = struct.pack(">bbbbbbbb", 3, -1, -3, -1, 2, -2, -2, -2)
        gen = region + b"\x00" + at + mq.flush()
    return page + seg(1, 38, gen)


def dct_dict(w, h, cs="/DeviceGray"):
    return "/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace %s /BitsPerComponent 8 /Filter /DCTDecode" % (w, h, cs)


def ccitt_dict(w, h, parms, extra=""):
    return "/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /CCITTFaxDecode /DecodeParms << %s >> %s" % (w, h, parms, extra)


# 1-2: DCT
write("scan_dct_gray.pdf", page_pdf([(dct_dict(W, H), jpeg_bytes(gray), full)], mb))
rgb = Image.merge("RGB", (gray, gray.point(lambda v: min(255, v + 12)), gray.point(lambda v: max(0, v - 12))))
write("scan_dct_rgb.pdf", page_pdf([(dct_dict(W, H, "/DeviceRGB"), jpeg_bytes(rgb), full)], mb))
# 3-5: CCITT G4, G3 1-D, G3 2-D
g4 = tiff_strip(bw, "Group4")
write("scan_g4.pdf", page_pdf([(ccitt_dict(W, H, "/K -1 /Columns %d /Rows %d" % (W, H)), g4, full)], mb))
try:
    g3 = tiff_strip(bw, "Fax")
    write("scan_g3_1d.pdf", page_pdf([(ccitt_dict(W, H, "/K 0 /Columns %d /Rows %d /EndOfLine true" % (W, H)), g3, full)], mb))
except Exception as e:
    print("no g3 1d:", e)
# 6-7: Flate 8-bit gray with PNG predictor, Flate 1-bit with inverted Decode
rows = [bytes([2]) + bytes((a - b) & 255 for a, b in zip(gray.crop((0, y, W, y + 1)).tobytes(), gray.crop((0, y - 1, W, y)).tobytes() if y else bytes(W))) for y in range(H)]
write("scan_flate_gray.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /DecodeParms << /Predictor 15 /Colors 1 /Columns %d /BitsPerComponent 8 >>" % (W, H, W), zlib.compress(b"".join(rows), 6), full)], mb))
inv = bw.point(lambda v: 255 - v).convert("1")
write("scan_flate_1bit.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace /DeviceGray /BitsPerComponent 1 /Decode [1 0] /Filter /FlateDecode" % (W, H), zlib.compress(inv.tobytes(), 6), full)], mb))
# 8: Indexed 4-bit palette, Flate
q = gray.quantize(16, dither=Image.Dither.NONE)
pal = q.getpalette()[:48]
packed = bytearray()
qpx = q.tobytes()
for y in range(H):
    r = qpx[y * W:(y + 1) * W]
    packed += bytes((r[i] << 4) | (r[i + 1] if i + 1 < W else 0) for i in range(0, W, 2))
write("scan_indexed_4bit.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace [/Indexed /DeviceRGB 15 <%s>] /BitsPerComponent 4 /Filter /FlateDecode" % (W, H, bytes(pal).hex()), zlib.compress(bytes(packed), 6), full)], mb))
# 9-10: JBIG2 arithmetic and MMR (small page: crop, to keep the pure-Python encoder quick)
cw, ch = 1176, 831  # half size crop at 200 dpi
small = bw.crop((0, 0, cw, ch))
jb = jbig2_generic(small)
jbm = jbig2_generic(small, tiff_strip(small, "Group4"))
smb = (0, 0, cw * 72 / 200, ch * 72 / 200); sfull = (cw * 72 / 200, 0, 0, ch * 72 / 200, 0, 0)
jdict = "/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /JBIG2Decode" % (cw, ch)
write("scan_jbig2_arith.pdf", page_pdf([(jdict, jb, sfull)], smb))
write("scan_jbig2_mmr.pdf", page_pdf([(jdict, jbm, sfull)], smb))
write("scan_g4_small.pdf", page_pdf([(ccitt_dict(cw, ch, "/K -1 /Columns %d /Rows %d" % (cw, ch)), tiff_strip(small, "Group4"), sfull)], smb))
# 11: two images side by side (left DCT, right G4), composited
lw = W // 2
left = gray.crop((0, 0, lw, H)); right = bw.crop((lw, 0, W, H))
lpt = lw * 72 / 200; rpt = (W - lw) * 72 / 200
write("scan_multi.pdf", page_pdf([
    (dct_dict(lw, H), jpeg_bytes(left), (lpt, 0, 0, PH, 0, 0)),
    (ccitt_dict(W - lw, H, "/K -1 /Columns %d /Rows %d" % (W - lw, H)), tiff_strip(right, "Group4"), (rpt, 0, 0, PH, lpt, 0))], mb))
# 12: rotated page with an offset MediaBox and a CropBox
ox, oy = 30, 20
write("scan_rot90_crop.pdf", page_pdf([(dct_dict(W, H), jpeg_bytes(gray), (PW, 0, 0, PH, ox, oy))],
      (0, 0, PW + 100, PH + 100), rotate=90, cropbox=(ox, oy, ox + PW, oy + PH)))
write("scan_rot180.pdf", page_pdf([(dct_dict(W, H), jpeg_bytes(gray), (PW, 0, 0, PH, 0, 0))], mb, rotate=180))
# 13: stencil mask (ImageMask) painted black over nothing
write("scan_imagemask.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width %d /Height %d /ImageMask true /BitsPerComponent 1 /Filter /CCITTFaxDecode /DecodeParms << /K -1 /Columns %d /Rows %d >>" % (W, H, W, H), g4, full)], mb))
# 14: A3 at 200 dpi (3307 x 2339), G4 and DCT
a3 = gray.resize((3307, 2339), Image.LANCZOS)
a3b = a3.point(lambda v: 255 if v >= 128 else 0).convert("1")
a3pt = (3307 * 72 / 200, 2339 * 72 / 200)
write("scan_a3_g4.pdf", page_pdf([(ccitt_dict(3307, 2339, "/K -1 /Columns 3307 /Rows 2339"), tiff_strip(a3b, "Group4"), (a3pt[0], 0, 0, a3pt[1], 0, 0))], (0, 0) + a3pt))
write("scan_a3_dct.pdf", page_pdf([(dct_dict(3307, 2339), jpeg_bytes(a3), (a3pt[0], 0, 0, a3pt[1], 0, 0))], (0, 0) + a3pt))
# 14b: CMYK JPEG (Adobe, values stored inverted by PIL's writer; /Decode undoes it)
cm = gray.resize((W // 4, H // 4)).convert("CMYK")
write("scan_dct_cmyk.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Decode [1 0 1 0 1 0 1 0] /Filter /DCTDecode" % (W // 4, H // 4), jpeg_bytes(cm), full)], mb))
# 15: vector page from the generator (copied)
with open(os.path.join(SRC, "pdf", "sample-00000.pdf"), "rb") as f:
    write("vector_sample.pdf", f.read())
# ---- malformed / hostile ----
good = page_pdf([(dct_dict(64, 48), jpeg_bytes(gray.resize((64, 48))), (100, 0, 0, 75, 0, 0))], (0, 0, 100, 75))
write("bad_truncated.pdf", good[: len(good) // 2])
write("bad_garbage.pdf", b"not a pdf at all\n" * 50)
write("bad_jpeg_data.pdf", page_pdf([(dct_dict(64, 48), b"\xff\xd8\xff\xe0 broken", (100, 0, 0, 75, 0, 0))], (0, 0, 100, 75)))
write("bad_jpx.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width 64 /Height 48 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /JPXDecode", b"\x00\x00\x00\x0cjP  \r\n\x87\n", (100, 0, 0, 75, 0, 0))], (0, 0, 100, 75)))
write("bad_encrypted.pdf", page_pdf([], (0, 0, 100, 75), extra_trailer="/Encrypt << /Filter /Standard /V 1 /R 2 /O (x) /U (x) /P -4 >>"))
# Flate bomb: 200 MiB of zeros (about 200 KB deflated) behind a 1000 x 1000 gray image
bomb = zlib.compressobj(9)
chunk = bytes(1 << 20)
body = b"".join(bomb.compress(chunk) for _ in range(200)) + bomb.flush()
write("bad_bomb.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width 1000 /Height 1000 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", body, (100, 0, 0, 75, 0, 0))], (0, 0, 100, 75)))
# huge declared size, tiny data
write("bad_huge_dims.pdf", page_pdf([("/Type /XObject /Subtype /Image /Width 100000 /Height 100000 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", zlib.compress(b"\x00" * 100), (100, 0, 0, 75, 0, 0))], (0, 0, 100, 75)))
# page tree loop
loop = Pdf()
loop.add(b"<< /Type /Catalog /Pages 2 0 R >>"); loop.add(b"<< /Type /Pages /Kids [2 0 R] /Count 1 >>")
write("bad_page_loop.pdf", loop.build(1))
