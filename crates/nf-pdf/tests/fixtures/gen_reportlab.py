"""Generates three small vector PDF fixtures with ReportLab (offline).

Run:  python3 -I gen_reportlab.py <output directory>
The generated files are committed next to this script; the tests in
tests/reportlab.rs convert them.
"""

import os
import sys

from reportlab.pdfgen import canvas


def lines(path):
    c = canvas.Canvas(path, pagesize=(300, 200), pageCompression=1)
    c.setLineWidth(0.5)
    c.line(10, 10, 290, 10)
    c.rect(20, 30, 100, 50, stroke=1, fill=0)
    c.setDash(3, 2)
    c.line(10, 150, 290, 150)
    c.save()


def curves(path):
    c = canvas.Canvas(path, pagesize=(300, 200), pageCompression=1)
    p = c.beginPath()
    p.moveTo(20, 20)
    p.curveTo(20, 120, 120, 120, 120, 20)
    p.close()
    c.drawPath(p, stroke=1, fill=1)
    c.save()


def text(path):
    c = canvas.Canvas(path, pagesize=(300, 200), pageCompression=1)
    c.line(10, 10, 200, 10)
    c.setFont("Helvetica", 12)
    c.drawString(20, 100, "PART 42")
    c.drawString(20, 60, "café 25 mm")
    c.save()


if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    lines(os.path.join(out, "reportlab_lines.pdf"))
    curves(os.path.join(out, "reportlab_curves.pdf"))
    text(os.path.join(out, "reportlab_text.pdf"))
