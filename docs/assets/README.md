# docs/assets

The README's pictures and the images the forges show. Every SVG here is
the output of one of two scripts in `tools/`, never drawn or edited by
hand. Both run from the repo root, with the IBM Plex fonts installed
(`/usr/share/fonts/truetype/ibm-plex/`, the `fonts-ibm-plex` package,
6.1.1 when these were drawn) and fontTools, which `uv` fetches:

```bash
uv run --no-project --with fonttools==4.60.1 python3 tools/gresil_logo.py \
  crates/gresil/tests/traces/feedforward-8.trace docs/assets
uv run --no-project --with fonttools==4.60.1 python3 tools/gresil_hero.py \
  crates/gresil/tests/traces/feedforward-8.trace \
  evidence/esp32c3-bringup/board-r67-head.log docs/assets
```

Run again, the two write every SVG here byte for byte, so `git status`
stays clean. The second reads the lockup the first writes, to strip its
background.

## The logo, frozen

`gresil_logo.py` draws the feed-forward cascade of `feedforward-8.trace`
at steps 60 to 62: neurons 0 to 2 fire at 60, 3 to 5 at 61, 6 and 7 at
62. Rows are neurons, columns are steps, and a lit dot is a spike. The
wordmark and the gloss are IBM Plex Sans (OFL) turned into outlines, so
no file needs a font; the accent that makes the e an é is one dot in
the accent colour. Ink `#1c2140` on light and `#f2f5fb` on dark, the
accent `#1f8fe0` and `#5cc8ff`.

- `lockup-{light,dark}.svg`: the mark and the wordmark, 900 × 300.
- `icon-square-{light,dark}.svg`: 8 steps around the cascade, 256 ×
  256, for the favicon and the avatars.
- `icon-wide-{light,dark}.svg`: 16 steps, 256 × 256.
- `banner-{light,dark}.svg`: 1280 × 640, for GitHub's social preview:
  40 steps, three input bursts and the cascade, the wordmark and the
  gloss under them.

It was drawn on 2026-09-16 from that trace, whose rows have not moved
since `c52ed85`, and it stays as drawn: a trace that moves after alpha.9
redraws the hero, never the logo. The forges take a PNG, not an SVG,
for the avatars and the social preview; it is exported at upload time
and not kept here.

## The hero, redrawn when the traces move

`gresil_hero.py` reads the host's frozen trace and the board's log of
the same case, and stops with exit 1 unless the board printed the
host's rows byte for byte; when it did, the script prints
`rows equal: 150`. It then draws steps 44 to 75 in the logo's style:
one dot per neuron per step, its size the membrane's charge from reset
to threshold, a full dot a spike, the spikes passed down the layers in
the accent colour.

- `hero-{light,dark}.svg`: the host's field over the chip's, labelled,
  the README's figure.
- `field-{light,dark}.svg`: one field, no label, for a link card.
- `lockup-{light,dark}-clear.svg`: the lockup without its background,
  the README's header.

Their backgrounds are transparent, and the README picks the light or
the dark file by the reader's theme. When the traces move, a board run
of the moved trace gives the new log, which takes the place of
`board-r67-head.log` in the command. The hero's label takes its row
count from the trace's header, but the root README's caption under the
hero writes it by hand, "the same 150 rows": a redraw with another count
changes that line too.

## The scope, recorded

`scope.gif` is a recording, not a drawing: `gresil-scope` from
`cargo run -p gresil-scope --release` at its defaults, 128 neurons, the
balanced topology, a 600 pA drive, recorded on 2026-10-19 by
Spectacle's window recording. Three seconds of it, from 10 s in,
became the GIF: the window's title bar cropped to its 900 × 640
content, 20 frames a second, 640 px wide, a palette of 32 colours made
from those frames, no dither, under 2 MB:

```bash
vf="crop=900:640:0:28,fps=20,scale=640:-1:flags=bilinear"
ffmpeg -ss 10 -t 3 -i recording.webm -vf "$vf,palettegen=max_colors=32:stats_mode=full" palette.png
ffmpeg -ss 10 -t 3 -i recording.webm -i palette.png \
  -lavfi "$vf[x];[x][1:v]paletteuse=dither=none:diff_mode=rectangle" \
  -loop 0 -fflags +bitexact docs/assets/scope.gif
```
