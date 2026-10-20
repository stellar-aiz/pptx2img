# Global, local, and diagnostic comparisons

`compare.py` uses LibreOffice as a reference, not PowerPoint ground truth.
Reference rendering may itself contain overlaps or differences. Passing thresholds
does not prove compatibility. Inspect both images when investigating differences:
source pages 1/20 in the 2026-10-18 local comparison also had reference-side overlaps.
Source documents and images remain local.

For environment setup and whole-page metrics, see
[development](../../docs/development.md).

## Regions

`--regions regions.json` adds per-page local comparisons. `slide` is one-based,
`name` is unique within a page, and `rect` is normalized `[x, y, width, height]`
in 0–1. Out-of-bounds rectangles, zero area, nonexistent pages, and duplicate names
are execution errors.

```json
{
  "regions": [
    {"slide": 1, "name": "small-shape", "rect": [0.1, 0.1, 0.02, 0.02]}
  ]
}
```

Floor the top-left and ceil the bottom-right to obtain the pixel region.
`pixel_rect` records left/top/right/bottom, with right/bottom exclusive.
Local regions use the same thresholds as whole pages: MAE 0.02, changed pixels
0.05, and channel tolerance 16. No per-region thresholds hide differences.

| Report field | Meaning |
|---|---|
| `slides[].passed` / `global_passed` | Whole-page pixel verdict |
| `slides[].regions` | Metrics and verdict per requested region |
| `regions_passed` | Combined region verdict; `null` if none requested |
| `diagnostic_summary` | Counts by diagnostic category and `has_omissions` |
| `slides[].renderer_diagnostics` | Original diagnostics with part/object/location |
| `status` | `difference_exceeded` if global or local thresholds are exceeded |

Diagnostic summaries exist globally and per page. Old manifests without diagnostic
fields report `available: false` and `null` omission status; renderer_warnings are
retained. Exit codes are 0 for both global/local passing, 1 for either exceeding,
and 2 for execution errors.

Diagnostics are independent of pixel verdicts: `global_passed: true` can coexist
with `has_omissions: true`. Local metrics detect differences, not their causes or
object identities. An absence of omitted diagnostics does not prove compatibility.

## Independent small-omission fixture

Generate a small chart surrounded by whitespace. Only fixture generation needs
python-pptx; no runtime dependency is added. Chart omission is a known renderer
limitation, not fixed by this comparator.

```sh
cargo build --locked
.venv-visual/bin/python tests/visual/make_local_omission.py /tmp/local-omission.pptx
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/debug/pptx2img \
  --fixture /tmp/local-omission.pptx --regions /tmp/local-omission.json \
  --width 1600 --out target/visual-tests/local-omission
```

Expect global passing, local failure, omission diagnostics, and exit 1. Without
`--regions`, only whole-page metrics apply and exit is 0, but omission diagnostics
remain in the report.

## Independent unsupported-image fixture

Generate a deck whose slide and shared layout each place a minimal EMF picture
(written byte by byte by the script). pptx2img omits both pictures with diagnostics
and renders every slide; LibreOffice draws the EMF rectangles, so the difference is
a known limitation, not a regression.

```sh
.venv-visual/bin/python tests/visual/make_unsupported_images.py /tmp/unsupported-images.pptx
target/debug/pptx2img /tmp/unsupported-images.pptx --out /tmp/unsupported-images --width 1600 --font-fallback
```

## Independent oversized-image fixture

Generate a deck with a 4400x4400 PNG and a 5800x3456 JPEG, plus a same-layout
baseline deck without pictures for peak-memory comparison. pptx2img downscales both
pictures (longest side 2048 px) with `approximation` diagnostics.

```sh
.venv-visual/bin/python tests/visual/make_oversized_images.py /tmp/oversized-images.pptx
/usr/bin/time -l target/release/pptx2img /tmp/oversized-images.pptx --out /tmp/oversized --width 1600 --font-fallback
/usr/bin/time -l target/release/pptx2img /tmp/oversized-images-baseline.pptx --out /tmp/oversized-baseline --width 1600 --font-fallback
```

## Hidden slides

LibreOffice's PDF export skips hidden slides. pptx2img does too by default, so pages pair
one to one; the comparator additionally drops manifest entries with `hidden: true` before
pairing (so a deck rendered with `--include-hidden` elsewhere still lines up) and records
`hidden_slides` in `report.json`. A renderer older than manifest schema 3 draws hidden
slides, and the comparator then stops with `Page counts differ`. In the report, `slides[].slide` is the
PDF page number and `slides[].deck_slide` is the position in the deck.

```sh
.venv-visual/bin/python tests/visual/make_hidden_slides.py /tmp/hidden-slides.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture /tmp/hidden-slides.pptx --width 1600 --out target/visual-tests/hidden-slides
```

## Deterministic reference fonts

LibreOffice resolves families that are missing from the machine (Meiryo UI, Yu Gothic,
Calibri, ...) through CoreText/fontconfig, and the result was observed to differ between
runs, which made reference images fluctuate independently of any pptx2img change. The
comparator therefore writes a replacement table (`Font/Substitution`, "Always") into the
generated LibreOffice profile. The default table, `tests/visual/font-substitution.json`,
maps those families to Hiragino Sans, the face pptx2img resolves to on macOS, so the
comparison measures layout rather than font choice. Override it with
`--font-substitution-map map.json` or `PPTX2IMG_FONT_SUBSTITUTION_MAP` (`{"families": {"Family": "Substitute"}}`).

| Report field | Meaning |
|---|---|
| `environment.libreoffice_font_substitution` | The family-to-substitute table pinned in the profile |
| `environment.font_substitution_sha256` | Hash of the copied `font-substitution.json` |
| `reference_pdf_fonts` | Fonts used by text in the reference PDF (`name`, `embedded`, `pages`) |
| `requested_families` | `a:latin` / `a:ea` typefaces declared by the deck (slides, layouts, masters, notes, theme, charts, diagrams); theme references such as `+mn-lt` are skipped because the theme part declares the faces they name |
| `unpinned_families` | The requested families the table does not list (compared case-insensitively). Their reference font is left to LibreOffice's own fallback and can differ between runs |

The table pins only the listed families. A family that is not listed, or a substitute that
is not installed, still falls back inside LibreOffice. `compare.py` therefore prints the
unpinned families as a warning and records them in `unpinned_families`; a drifting
reference for text in those families is explained by the report itself. The theme's
declared faces are listed even when no text uses them, so the list can be longer than the
set of families actually drawn. `--strict-font-pinning` turns a non-empty list into a
failure (`status` = `unpinned_families`, exit 1); `check_determinism.py` accepts the same
flag and otherwise lists the unpinned families as a warning in its summary. Compare
`reference_pdf_fonts` between runs when reference images differ.

The default tables also pin the families that fell back differently between runs in a
local corpus survey: Japanese gothic variants (HGP/HG, Meiryo, Yu Gothic Light/Medium,
BIZ UD, IPA P), Microsoft YaHei, and the Latin families Aptos, Century and Century Gothic,
which otherwise ended up on HiraMinProN, SimSong, PyosuFontFree or Liberation. The macOS
table additionally maps shipped faces (Arial, Georgia, Times New Roman, ...) to themselves
so they count as pinned without changing the reference; the Linux and Windows tables
leave those out, so Arial text is reported as unpinned there. Add a row to all three
tables when a new family shows up in `unpinned_families`. The substitution replaces font choice, not font
metrics: pinned references still differ from pptx2img where layout differs.

```sh
.venv-visual/bin/python tests/visual/make_font_substitution.py /tmp/font-substitution.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture /tmp/font-substitution.pptx --width 1600 --out target/visual-tests/font-substitution
```

`make_unpinned_font.py` writes a deck whose only text requests a family that no table
lists; `compare.py` reports it in `unpinned_families` (add `--strict-font-pinning` to make
that an error):

```sh
.venv-visual/bin/python tests/visual/make_unpinned_font.py /tmp/unpinned-font.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture /tmp/unpinned-font.pptx --width 1600 --out target/visual-tests/unpinned-font
```

### Per-OS tables in the visual CI job

Each OS pins a table whose substitute is installed on that OS, so the pinned face never
falls back inside LibreOffice:

| OS | Table | Substitute | Where the face comes from |
|---|---|---|---|
| macOS | `font-substitution.json` (default) | Hiragino Sans | Part of macOS; its hashes are checked by `check_environment.py` |
| Linux | `font-substitution.linux.json` | Noto Sans CJK JP | `NotoSansCJKjp-Regular.otf` and `-Bold.otf` from the `notofonts/noto-cjk` release `Sans2.004`, installed by the job with pinned SHA-256 values |
| Windows | `font-substitution.windows.json` | Noto Sans CJK JP | The same two files, installed into the per-user font directory |

The scheduled/manual workflow (`.github/workflows/visual.yml`) runs a matrix over these
three OSes with the pinned LibreOffice 26.2.5.2 and sets `PPTX2IMG_FONT_SUBSTITUTION_MAP` to
the OS table, which `compare.py` uses as its default when `--font-substitution-map` is absent
(this is how the cargo-driven comparisons receive it). The job then runs
`check_determinism.py`, which passes `--font-substitution-map` explicitly, compares
twice, requires byte-identical reference PNGs, and appends the pinned table, the PDF fonts
actually used, and the reference hashes to the job summary. It checks
`examples/text-bullets.pptx` and a generated `make_font_substitution.py` deck, whose
families are in the table. `report.json` of every run lists the pinned table under
`environment.libreoffice_font_substitution`. Thresholds are unchanged: a failing
comparison on an OS is recorded evidence, not something to relax.

The renderer still resolves families through its own system font lookup on each OS; the
table pins only the LibreOffice reference. Differences between the two faces are part of
the recorded comparison.

## Independent table row-growth fixtures

`make_table_row_growth.py` writes a table whose stored row heights are shorter than
its wrapped text (page 1) and a table whose frame is twice as tall as its rows
(page 2). `make_table_zero_row_height.py` writes a table with an `h="0"` row. Rows
grow to fit their text and are not stretched to the frame; see
[rendering](../../docs/rendering.md#table-row-heights) for recorded metrics.

```sh
.venv-visual/bin/python tests/visual/make_table_row_growth.py examples/table-row-growth.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture examples/table-row-growth.pptx --width 1600 --out target/visual-tests/table-row-growth
```

## Independent flipped-text fixture

Generate rectangles, an arrow, and groups with `flipH`/`flipV` that all carry a
left-aligned label, plus per-cell regions. Geometry is mirrored; text is never
mirrored (`flipV` turns it 180 degrees, as in LibreOffice). Slide 2 adds one-cell tables
placed directly and inside flipped groups; LibreOffice does not mirror those tables, so
their regions differ from the reference by design (see `docs/rendering.md`).

```sh
.venv-visual/bin/python tests/visual/make_flipped_text.py examples/flipped-text.pptx
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/release/pptx2img --fixture examples/flipped-text.pptx \
  --regions examples/flipped-text.json --width 1600 --out target/visual-tests/flipped-text
```

## Independent custom-geometry fixture

Generate six `a:custGeom` freeforms without text: a python-pptx freeform builder
polygon, a cubic Bezier edge, `arcTo` corners, an open `fill="none"` curve, a `gdLst`
bracket in shape coordinates, and a `stroke="0"` fill path under an outline path,
plus per-cell regions. See [rendering](../../docs/rendering.md#freeform-shapes) for
recorded metrics.

```sh
.venv-visual/bin/python tests/visual/make_custom_geometry.py examples/custom-geometry.pptx
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/release/pptx2img --fixture examples/custom-geometry.pptx \
  --regions examples/custom-geometry.json --width 1600 --out target/visual-tests/custom-geometry
```

## Independent image-effects fixture

Generate 13 pages, each with a generated picture shown unchanged and with one `a:blip`
effect, `a:tile` fill, or `a:stretch/a:fillRect` inset, plus per-picture regions.
LibreOffice ignores picture tiles, fill rectangles, and `a14:imgProps` corrections, so
those pages are expected to differ. See
[rendering](../../docs/rendering.md#picture-effects-and-fills) for recorded metrics.

```sh
.venv-visual/bin/python tests/visual/make_image_effects.py examples/image-effects.pptx
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/release/pptx2img --fixture examples/image-effects.pptx \
  --regions examples/image-effects.json --width 1600 --out target/visual-tests/image-effects
```

## Generators documented elsewhere

Every `make_*.py` generator is described either in this file or in the feature page
that owns its behaviour.

| Generator | Described in |
|---|---|
| `make_arrow_sizes.py`, `make_connector_routing.py`, `make_lines.py`, `make_preset_shapes.py`, `make_theme_lines.py` | [preset shapes](../../docs/preset-shapes.md) |
| `make_auto_punctuation.py`, `make_boundary_widths.py`, `make_character_spacing.py`, `make_composite_shapes.py`, `make_punctuation_positions.py` | [rendering notes](../../docs/rendering-notes.md) |
| `make_colors.py`, `make_fills.py`, `make_gradients.py` | [colors](../../docs/colors.md) |
| `make_ole.py` | [OLE previews](../../docs/ole-preview.md) |
| `make_shadows.py` | [shadows](../../docs/shadows.md) |
| `make_table_styles.py` | [table styles](../../docs/table-styles.md) |
| `make_text_autofit.py`, `make_text_bullets.py` | [rendering](../../docs/rendering.md) |
| `make_vertical_text.py` | [rendering](../../docs/rendering.md#vertical-text) |

## Reproducibility

Output retains input PPTX, commands, stdout/stderr, reference PDF, both PNG sets,
global differences, regions JSON, and a copy of the comparator source. Reports
record hashes for input, renderer, LibreOffice executable, comparator, and regions;
width, thresholds, compression settings; Python/image-library/OS versions; font
paths and hashes; and renderer-resolved fonts. Reproduce with the same binaries,
fonts, OS, dependencies, and saved settings, using a new output directory.

```sh
.venv-visual/bin/python -m unittest discover -s tests/visual -p 'test_*.py'
cargo test --locked
```

Unit tests cover small omissions missed globally but caught locally, rectangle
validation, diagnostic aggregation, old manifests, and existing threshold boundaries.
Publish only independently created fixtures, not private source documents or images.

The comparator explicitly passes `--font-fallback` to measure approximate rendering;
font diagnostics remain in the manifest. The standalone CLI rejects family
substitution by default, permits successful glyph fallback with warnings, and
always rejects glyphs unavailable in every loaded font.

## Independent unresolved-picture-reference fixture

Generate a slide with four generated PNG pictures: a control, one whose relationship
type is rewritten to `oleObject`, one whose `r:embed` names a missing relationship,
and one whose crop leaves no visible area. pptx2img renders the first two and omits
the other two with `omitted` diagnostics instead of aborting the deck.

```sh
.venv-visual/bin/python tests/visual/make_image_references.py /tmp/image-references.pptx
target/debug/pptx2img /tmp/image-references.pptx --out /tmp/image-references --width 1600 --font-fallback
```

## Independent SVG-picture fixture

Generate a slide with one generated SVG icon placed three ways: SVG only (`svgBlip`
without `r:embed`), SVG with a coarser PNG fallback drawn with Pillow, and SVG only
stretched to a wide frame. pptx2img renders the SVG for the first and third pictures
and the PNG fallback for the second (LibreOffice draws the SVG there, so its edges are
smoother in the reference).

```sh
.venv-visual/bin/python tests/visual/make_svg_pictures.py /tmp/svg-pictures.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture /tmp/svg-pictures.pptx --width 1600 --out target/visual-tests/svg-pictures
```

## Independent hidden-element fixture

`make_hidden.py` writes one slide with a visible rectangle, a hidden rectangle, a
hidden group, a visible group with one hidden child, and a hidden shape on the slide
layout. Only the visible rectangles are drawn, with no diagnostic for the hidden ones.

```sh
.venv-visual/bin/python tests/visual/make_hidden.py /tmp/hidden.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture /tmp/hidden.pptx --width 1600 --out target/visual-tests/hidden
```

## Independent empty-bullet fixture

`make_empty_bullets.py` writes a bulleted list that contains empty and whitespace-only
paragraphs. They draw no bullet marker.

```sh
.venv-visual/bin/python tests/visual/make_empty_bullets.py examples/empty-bullets.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture examples/empty-bullets.pptx --width 1600 --out target/visual-tests/empty-bullets
```

## Independent embedded-media fixture

`make_embedded_media.py [size_mib]` writes a slide with an embedded video (generated
zero bytes, 40 MiB by default, above the 32 MiB part limit) and a PNG poster frame.
The deck is written outside the repository. pptx2img renders the poster and reports
`Audio and video playback is unsupported`.

```sh
.venv-visual/bin/python tests/visual/make_embedded_media.py /tmp/embedded-media.pptx
target/release/pptx2img /tmp/embedded-media.pptx --out /tmp/embedded-media --width 1600 --font-fallback
```

## Independent text-inheritance fixture

`make_text_inheritance.py` writes two slides with the same wrapped text box: one
inherits its size and font from the presentation default text style, the other sets
them explicitly. The master `otherStyle` deliberately differs (24 pt versus 18 pt), so
a wrongly applied `otherStyle` shows up as a size difference between the slides.

```sh
.venv-visual/bin/python tests/visual/make_text_inheritance.py examples/text-inheritance.pptx
.venv-visual/bin/python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture examples/text-inheritance.pptx --width 1600 --out target/visual-tests/text-inheritance
```
