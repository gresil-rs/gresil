#!/usr/bin/env python3
"""gresil's hero, from a frozen trace and the board's capture of it.

  uv run --no-project --with fonttools==4.60.1 python3 tools/gresil_hero.py \
      <host.trace> <board.log> <outdir>

Reads the host's frozen trace and the same case's block in a board log,
checks the board printed the host's rows byte for byte, then draws a
window of the run in the logo's style (gresil_logo.py): one dot per
neuron per step, its size the membrane's charge from reset to threshold,
a full dot a spike, the spikes the network passed down its layers in the
accent colour. Labels are IBM Plex outlines (fontTools): no font needed.

Writes hero-{light,dark}.svg (the host's field over the chip's,
labelled), field-{light,dark}.svg (one field, no label, for the link
card) and lockup-{light,dark}-clear.svg (the lockup, background removed,
from the lockup-{light,dark}.svg tools/gresil_logo.py wrote into the
same <outdir>; docs/assets/README.md has both commands).
Backgrounds are transparent: a README picks the file per theme.
"""
import sys, pathlib
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

HOST, BOARD, OUT = (pathlib.Path(a) for a in sys.argv[1:4]); OUT.mkdir(parents=True, exist_ok=True)
FONTS = "/usr/share/fonts/truetype/ibm-plex/"
SEMI = TTFont(FONTS + "IBMPlexSans-SemiBold.ttf"); REG = TTFont(FONTS + "IBMPlexSans-Regular.ttf")
MONO = TTFont(FONTS + "IBMPlexMono-Regular.ttf")

# ---- the two sources, and the check that they are one -----------------
def check(ok, why):   # never `assert`: `python -O` strips it, and this check is the point
    if not ok: sys.exit(f"gresil_hero: {why}")

host = HOST.read_text().splitlines(); head = host[0]
check(head.startswith("# neuralos-trace v1 case="), f"not a trace header: {head}")
block, on = [], False
for line in BOARD.read_text().replace("\r", "").splitlines():
    if line == head: on = True
    elif on and line.startswith("# neuralos-trace"): break
    if on: block.append(line)
check(block == host, f"the board's {len(block)} lines differ from the host's {len(host)}")
meta = dict(kv.split("=") for kv in head.split() if "=" in kv)
N = int(meta["n"]); check(meta["res"] == "cmV" and meta["rows"] == "all", f"unexpected header {meta}")
spikes, volt = {}, {}
for line in host[1:]:
    left, col, vs = line.split("|")
    s = int(left.split()[0]); spikes[s] = {int(x) for x in col.split()}; volt[s] = [int(v) for v in vs.split()]
check(len(spikes) == int(meta["steps"]), f"{len(spikes)} rows for {meta['steps']} steps")

# The case: cases.rs feedforward_8, layers [3, 3, 2]. Types as network.rs
# makes them: the first trunc(n * 0.8) excitatory, the rest inhibitory.
# Thresholds -55 / -50 mV and reset -80 mV (lif_neuron.rs), on the centi grid.
LAYERS = [3, 3, 2]; check(sum(LAYERS) == N, f"layers {LAYERS} for {N} neurons")
check(meta["case"] == "feedforward-8", f"layers {LAYERS} are feedforward-8's, not {meta['case']}'s")
LAYER = [li for li, k in enumerate(LAYERS) for _ in range(k)]
EXC = int(N * 0.8); THR = [-5500 if i < EXC else -5000 for i in range(N)]; RESET = -8000
for s in spikes:   # the encoding holds on every row: under threshold unless spiking, reset on a spike
    for i in range(N):
        ok = (volt[s][i] == RESET) if i in spikes[s] else (volt[s][i] < THR[i])
        check(ok, f"step {s}, neuron {i}: {volt[s][i]} breaks the encoding")

WIN = list(range(44, 76))   # 32 steps: the input bursts at 49, 60 and 71, the cascade at 60, 61, 62 in the middle, the refill after
                            # (48 steps from 36 until 2026-10-19; narrowed so the dots read at README width)

PAL = {
  "light": dict(ink="#1c2140", accent="#1f8fe0"),
  "dark":  dict(ink="#f2f5fb", accent="#5cc8ff"),
}

def word_paths(font, text, x, baseline, size):
    gs = font.getGlyphSet(); cmap = font.getBestCmap(); k = size / font["head"].unitsPerEm
    paths = []; pen_x = x
    for ch in text:
        g = gs[cmap[ord(ch)]]
        pen = SVGPathPen(gs); g.draw(TransformPen(pen, (k, 0, 0, -k, pen_x, baseline)))
        d = pen.getCommands()
        if d: paths.append(d)
        pen_x += g.width * k
    return paths, pen_x - x

def text(font, s, x, baseline, size, fill, op=1.0, anchor="start"):
    _, adv = word_paths(font, s, 0, 0, size)
    x0 = {"start": x, "middle": x - adv / 2, "end": x - adv}[anchor]
    paths, _ = word_paths(font, s, x0, baseline, size)
    return f'<g fill="{fill}" fill-opacity="{op}">' + "".join(f'<path d="{d}"/>' for d in paths) + "</g>", adv

def dot(x, y, r, fill, op=1.0):
    o = "" if op == 1.0 else f' fill-opacity="{op:.3f}"'
    return f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{r:.2f}" fill="{fill}"{o}/>'

def field(x0, y0, cell, gap, p):
    out = []
    for ci, s in enumerate(WIN):
        cx = x0 + (ci + 0.5) * cell
        for i in range(N):
            cy = y0 + (i + 0.5) * cell + gap * LAYER[i]
            if i in spikes[s]:
                out.append(dot(cx, cy, cell * 0.34, p["accent"] if LAYER[i] else p["ink"]))
            else:
                f = (volt[s][i] - RESET) / (THR[i] - RESET)
                out.append(dot(cx, cy, cell * (0.07 + 0.15 * f ** 0.5), p["ink"], 0.14 + 0.30 * f))
    return "".join(out)

def svg(w, h, body, label):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h:.0f}" viewBox="0 0 {w} {h:.0f}" '
            f'role="img" aria-label="{label}">{body}</svg>')

HERO_ALT = ("The same eight neurons on an x86_64 host and on an ESP32-C3: two identical dot fields, "
            "dot size the membrane charge, full dots the spikes")
FIELD_ALT = f"Eight neurons over {len(WIN)} steps: dot size the membrane charge, full dots the spikes"

for name, p in PAL.items():
    # ---- hero 1280 wide: the host's field over the chip's -------------
    W = 1280; fx = 180; cell = (W - fx - 30) / len(WIN); gap = cell * 0.6
    fh = N * cell + gap * (len(LAYERS) - 1); top = 36; mid = 76
    ya, yb = top, top + fh + mid; H = yb + fh + top
    body = field(fx, ya, cell, gap, p) + field(fx, yb, cell, gap, p)
    for y, nm, tgt in ((ya, "host", "x86_64"), (yb, "ESP32-C3", "riscv32imc")):
        body += text(SEMI, nm, 0, y + 30, 30, p["ink"])[0] + text(MONO, tgt, 0, y + 62, 19, p["ink"], 0.62)[0]
    cy = ya + fh + mid / 2; cx = fx + (W - 30 - fx) / 2
    t, adv = text(REG, f"the chip printed the same {meta['steps']} rows, bit for bit", cx, cy + 7, 20, p["ink"], 0.62, "middle")
    pad = 18
    body += (f'<path d="M{fx:.2f} {cy:.2f}H{cx - adv / 2 - pad:.2f}M{cx + adv / 2 + pad:.2f} {cy:.2f}H{W - 30:.2f}" '
             f'stroke="{p["ink"]}" stroke-opacity="0.2" stroke-width="1.5"/>' + t)
    (OUT / f"hero-{name}.svg").write_text(svg(W, H, body, HERO_ALT))

    # ---- one field, full width, for the link card ------------------------
    cell = W / len(WIN); gap = cell * 0.7; fh = N * cell + gap * (len(LAYERS) - 1)
    (OUT / f"field-{name}.svg").write_text(svg(W, fh, field(0, 0, cell, gap, p), FIELD_ALT))

    # ---- the lockup without its background rectangle --------------------
    lk = (OUT / f"lockup-{name}.svg").read_text()
    a = lk.index("<rect "); b = lk.index("/>", a) + 2
    (OUT / f"lockup-{name}-clear.svg").write_text(lk[:a] + lk[b:])

print("rows equal:", len(host) - 1, "· wrote",
      sorted(f"{kind}-{name}{tail}.svg" for name in PAL for kind, tail in (("hero", ""), ("field", ""), ("lockup", "-clear"))))
