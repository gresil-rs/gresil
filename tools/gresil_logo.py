#!/usr/bin/env python3
"""gresil's logo, drawn from a real trace (docs/assets/README.md).

  uv run --no-project --with fonttools==4.60.1 python3 tools/gresil_logo.py \
      crates/gresil/tests/traces/feedforward-8.trace docs/assets

Reads a neuralos-trace v1 file, takes the spike column, and draws the
raster as a dot field (lit = spiked, unlit = faint). The wordmark and
the tagline are IBM Plex Sans converted to path outlines with
fontTools, so the SVG needs no font. The accent on the é is a raster
dot in the accent colour, the one dot that fell out of the field.

The logo is frozen at the alpha.9 trace: it is drawn again only from
that trace, never from one that moved (docs/assets/README.md).
"""
import sys, pathlib
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

TRACE = pathlib.Path(sys.argv[1]); OUT = pathlib.Path(sys.argv[2]); OUT.mkdir(parents=True, exist_ok=True)
FONTS = "/usr/share/fonts/truetype/ibm-plex/"
SEMI = TTFont(FONTS + "IBMPlexSans-SemiBold.ttf"); REG = TTFont(FONTS + "IBMPlexSans-Regular.ttf")

# ---- trace -> spikes[step] = set(neuron) -------------------------------
hdr = TRACE.read_text().splitlines()
meta = dict(kv.split("=") for kv in hdr[0].split() if "=" in kv)
N = int(meta["n"]); spikes = {}
for line in hdr[1:]:
    step, col, _ = line.split("|", 2)
    spikes[int(step.split()[0])] = {int(x) for x in col.split()}

PAL = {
  "light": dict(bg="#ffffff", ink="#1c2140", accent="#1f8fe0", off="#1c2140", off_op=0.12),
  "dark":  dict(bg="#0e1220", ink="#f2f5fb", accent="#5cc8ff", off="#f2f5fb", off_op=0.16),
}

def word_paths(font, text, x, baseline, size):
    """(svg path d-strings, advance, {index: x-center}) in user units"""
    gs = font.getGlyphSet(); cmap = font.getBestCmap(); k = size / font["head"].unitsPerEm
    paths = []; centers = {}; pen_x = x
    for i, ch in enumerate(text):
        g = gs[cmap[ord(ch)]]
        pen = SVGPathPen(gs); g.draw(TransformPen(pen, (k, 0, 0, -k, pen_x, baseline)))
        d = pen.getCommands()
        if d: paths.append(d)
        centers[i] = pen_x + g.width * k / 2
        pen_x += g.width * k
    return paths, pen_x - x, centers

def group(paths, fill, op=1.0):
    return f'<g fill="{fill}" fill-opacity="{op}">' + "".join(f'<path d="{d}"/>' for d in paths) + "</g>"

def raster(x0, y0, cell, steps, rows=None):
    rows = range(N) if rows is None else rows
    lit, off = [], []
    for ci, s in enumerate(steps):
        for ri, r in enumerate(rows):
            cx = x0 + (ci + 0.5) * cell; cy = y0 + (ri + 0.5) * cell
            (lit if r in spikes.get(s, ()) else off).append((cx, cy))
    return lit, off

def circles(pts, rad, fill, op=1.0):
    return "".join(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{rad:.2f}" fill="{fill}" fill-opacity="{op}"/>' for x, y in pts)

def svg(w, h, body, bg):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">'
            f'<rect width="{w}" height="{h}" fill="{bg}"/>{body}</svg>')

def accent(centers, base, size, fill):
    """the acute over the e as one dot: Plex's i-dot weight, just above the x-height"""
    return circles([(centers[2] + size * 0.01, base - size * 0.66)], size * 0.062, fill)

CASCADE = 60
WIN16 = list(range(CASCADE - 7, CASCADE + 9))     # lockup + wide icon
WIN8  = list(range(CASCADE - 3, CASCADE + 5))     # square icon
WIN40 = list(range(CASCADE - 19, CASCADE + 21))   # banner: three input bursts and the cascade

for name, p in PAL.items():
    # ---- lockup: mark left, wordmark right ----
    W, H = 900, 300; cell = 18; mx, my = 60, (H - N * cell) / 2
    lit, off = raster(mx, my, cell, WIN16)
    body = circles(off, cell * 0.16, p["off"], p["off_op"]) + circles(lit, cell * 0.32, p["ink"])
    size = 150; tx = mx + len(WIN16) * cell + 50; base = H / 2 + size * 0.36
    paths, adv, centers = word_paths(SEMI, "gresil", tx, base, size)
    body += group(paths, p["ink"]) + accent(centers, base, size, p["accent"])
    (OUT / f"lockup-{name}.svg").write_text(svg(W, H, body, p["bg"]))

    # ---- icons: wide (16 steps, banded) and square (8 steps, filled) ----
    for tag, win in (("wide", WIN16), ("square", WIN8)):
        I = 256; icell = I / len(win); iy = (I - N * icell) / 2
        lit, off = raster(0, iy, icell, win)
        last = max(lit, key=lambda c: (c[0], c[1]))
        body = (circles(off, icell * 0.16, p["off"], p["off_op"]) + circles(lit, icell * 0.32, p["ink"])
                + circles([last], icell * 0.32, p["accent"]))
        (OUT / f"icon-{tag}-{name}.svg").write_text(svg(I, I, body, p["bg"]))

    # ---- banner 1280x640: 40-step window drawn large, wordmark and meaning under ----
    W, H = 1280, 640; cell = W / len(WIN40); by = 48
    lit, off = raster(0, by, cell, WIN40)
    body = circles(off, cell * 0.14, p["off"], p["off_op"]) + circles(lit, cell * 0.30, p["ink"])
    size = 160; _, adv, _ = word_paths(SEMI, "gresil", 0, 0, size)
    tx = (W - adv) / 2; base = by + N * cell + 150
    paths, adv, centers = word_paths(SEMI, "gresil", tx, base, size)
    body += group(paths, p["ink"]) + accent(centers, base, size, p["accent"])
    tag = "grésil, n.: fine hail  ·  grésiller, v.: to crackle"; ts = 28
    _, tadv, _ = word_paths(REG, tag, 0, 0, ts)
    tpaths, _, _ = word_paths(REG, tag, (W - tadv) / 2, base + 84, ts)
    body += group(tpaths, p["ink"], 0.72)
    (OUT / f"banner-{name}.svg").write_text(svg(W, H, body, p["bg"]))
print("wrote", sorted(f"{kind}-{name}.svg" for name in PAL for kind in ("lockup", "icon-wide", "icon-square", "banner")))
