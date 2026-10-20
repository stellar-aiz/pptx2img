# Rendering support and limitations

This is an early, approximate renderer, not a complete PowerPoint implementation.
Unsupported content is reported where detected; detection itself is incomplete.
Strict mode gates known warnings, not all incompatibilities.

## Supported subset

- Transitional OOXML PPTX, using presentation-defined slide order.
- Explicit positions, sizes, rotation, and flips for basic shapes and selected
  [presets and connectors](preset-shapes.md), including chevrons, triangles,
  arrows, pie, star5, cloud, callouts, brackets and braces, flowchart shapes, and
  bent/curved connectors
  evaluated from DrawingML shape guides, and [freeform shapes](#freeform-shapes)
  (`a:custGeom`).
- Solid backgrounds, fills, and lines using RGB, system `lastClr`, and theme
  colors; selected [color transforms and theme fills](colors.md), including
  translucent (`alpha`) fills, lines, and text, and `tint`/`shade`/`satMod`;
  linear and radial [gradient fills](colors.md#gradient-fills) on shapes and
  backgrounds; theme background references (`bgRef`), approximate pattern fills,
  stretched picture fills, and group fills
  ([details](colors.md#theme-backgrounds-pattern-picture-and-group-fills)).
- Slide -> layout -> master -> theme relationships, color mappings, and major/
  minor Latin and Japanese theme fonts, including the Jpan supplemental font.
- Explicit font, size, color, bold/italic, and left/center/right paragraph alignment.
- Rotated [vertical text](#vertical-text) (`vert`, `vert270`).
- Nominal font-advance wrapping, basic Japanese prohibited breaks, explicit line
  breaks and empty lines, paragraph spacing, point/percentage line spacing, and
  top/center/bottom vertical anchoring with overflow warnings.
- `hangingPunct` inherited from paragraph, list, master, and presentation defaults;
  one trailing punctuation character (`、。，．`) may hang outside the box.
- Explicit character spacing `spc`, including negative values, positive values,
  inherited defaults, and explicit zero; optional automatic punctuation compression.
- Property-by-property inheritance from runs, paragraph defaults, and list levels.
- Embedded PNG/JPEG placement and `srcRect` cropping.
- Basic placeholder inheritance and shared master/layout shapes and solid backgrounds.
- Nested group transforms, explicit table cell formatting, rectangular merged
  cells, selected [table styles](table-styles.md), [outer shadows](shadows.md), and
  [static OLE previews](ole-preview.md).

## Inheritance

Placeholders match by `idx` from slide to layout and by type from layout to master.
Template sample text is not rendered; inherited properties supplement slide text.
Explicit slide properties win. `showMasterSp="0"` hides shared ancestor shapes.
This covers transforms, solid fills, and basic text styles, not every property.

Ordinary text boxes (`txBox=1`) use `presentation/defaultTextStyle`. Placeholders
inherit layout/master styles, while text inside shapes uses `otherStyle`.
Run, paragraph, and list values override defaults. Empty paragraphs use the size
and font metrics in `endParaRPr`.

The independently generated `examples/text-inheritance.pptx` compares explicit
and inherited 18pt Arial. Both resolve to 18pt in LibreOffice, but vertical position
and line-spacing differences exceed existing image thresholds. Overflow is
reported rather than silently discarded.
[Specification entry point](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-presentations).

## Saved autofit

`normAutofit/fontScale` (default 100%) applies to measurement, wrapping, and drawing
of all runs. `lnSpcReduction` (default 0%) subtracts from percentage line spacing:
120%-20%=100%. Fixed-point line spacing is unchanged. Child `noAutofit` cancels
inherited autofit. The renderer does not search for a new shrink ratio.

`spAutoFit` expands height for fixed-width horizontal text, preserving anchoring
and margins. Table-cell/vertical autofit is unsupported and warned about. Shape
text areas are remeasured at their final size, with diagnostics for expansion limits.

When the body also has `wrap="none"`, the shape width is set to the widest line plus
the left and right insets (and paragraph margins), so the outline, fill, and effects
follow the text; a saved width that is too wide shrinks as well. The edge on the
first paragraph's alignment side stays fixed: the left edge for left-aligned (and
justified) text, the centre for `algn="ctr"`, the right edge for `algn="r"`. Bodies
without visible text keep their saved width. The edge is taken along the text's own
direction, so rotation and flips that turn the text (for example `rot="10800000"` or
`flipV`) keep the edge that is on the alignment side as the text reads.

Fixture: `examples/text-autofit.pptx`; generator:
`tests/visual/make_text_autofit.py`. LibreOffice 26.2.5.2 recalculates this fixture's
saved shrink ratios and does not expand its saved boxes, so pixel differences
remain. Thresholds have not been relaxed.

Page 5 holds left, centred, and right-aligned `wrap="none"` + `spAutoFit` labels
whose saved widths are too narrow (top row) and too wide (bottom row). Measured at
`--width 1600` on macOS, LibreOffice 26.2.5.2 keeps every saved width and wraps the
narrow labels inside their boxes despite `wrap="none"`, so this page passed before
(MAE 0.0159, changed 0.0273) and now exceeds the thresholds (MAE 0.0250, changed
0.1057). The difference is the width rule itself, not a measurement error; the
alignment-side anchoring follows the Office interpretation and is not confirmed by
LibreOffice. PowerPoint was not compared directly.

- [NormalAutoFit](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit)
- [Office spAutoFit interpretation](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/19d31037-28bf-4693-85be-3a37bfe82d51)

## Vertical text

`a:bodyPr@vert="vert"` and `"vert270"` are rotated horizontal text: the lines are
laid out in the box with its width and height swapped, then turned 90 degrees
clockwise (`vert`) or counter-clockwise (`vert270`) about the box. Wrapping,
alignment, anchoring, and spacing therefore work along the turned lines, so
`algn="l"` starts at the top for `vert` and at the bottom for `vert270`, and
`anchor="t"` puts the first line on the right for `vert` and on the left for
`vert270`. The insets turn with the lines: for `vert` the shape's top, bottom,
right, and left insets act as the lines' left, right, top, and bottom insets; for
`vert270` the bottom, top, left, and right insets do. Shape autofit stays
unsupported for vertical bodies and is reported.

`eaVert` (upright East Asian columns), `wordArtVert`, `wordArtVertRtl`, and
`mongolianVert` are still laid out horizontally with a `Vertical text is
unsupported` diagnostic.

Fixture: `examples/vertical-text.pptx`; generator `tests/visual/make_vertical_text.py`.
Measured at `--width 1600` on macOS against LibreOffice 26.2.5.2 with default
thresholds: page 1 (`vert270` labels with three anchors and a `vert` label) moves
from MAE 0.0145, changed 0.0237 to MAE 0.0063, changed 0.0149 and passes. Page 2
(`eaVert` bodies) is unchanged (MAE 0.0603, changed 0.0855) and fails, because
`eaVert` is not laid out as columns yet.

An `eaVert` column layout (clockwise lines with upright CJK glyphs centred in an
ideographic em box, advances from `vmtx`, sideways brackets and dashes) reached
MAE 0.0287, changed 0.0515 on page 2 and was not landed, because it stays above the
default thresholds. The same text laid out horizontally measures MAE 0.0401,
changed 0.0636 against LibreOffice, so the remainder is not specific to vertical
layout. It comes from two differences shared with horizontal Japanese text: the
line pitch (Windows cell metrics, 1.26 em for Hiragino Sans W4, against about
1.19 em in LibreOffice) and the face (W4 resolved here, W3 in LibreOffice). With
the W3 face forced, the attempt measured MAE 0.0217, changed 0.0430. LibreOffice
also adds space between Japanese and Latin runs, which this renderer does not.
Thresholds have not been relaxed.

## Table row heights

`a:tr/@h` is a minimum row height. Each cell's text is laid out first, at its merged
width and with its cell margins (`marT`/`marB`, default 0.05in), and a row grows to
the tallest cell it holds: `max(stored h, margins + content)`. Rows that fit keep
their stored height, including `h="0"` rows, which grow to their text. A row-spanning
cell adds any remaining shortfall to its last spanned row. Rows are stacked from
the top of the frame; the graphicFrame height does not stretch or squash them, so a
grown table can extend below its frame (LibreOffice 26.2.5.2 also ignores the frame
height). Columns still scale to the frame width. Cells without text keep their stored
height. Measurement uses the same approximate text layout as drawing, so grown
heights inherit its line-height differences. A zero-height row is ordinary input and
emits no diagnostic: it is not given a share of the frame height, and the earlier
"Table rows with zero height" approximation warning no longer exists.

Fixtures: `examples/table-row-growth.pptx` (generator
`tests/visual/make_table_row_growth.py`) and `examples/table-zero-row-height.pptx`
(generator `tests/visual/make_table_zero_row_height.py`). At width 1600, the
zero-height fixture and the frame-height page pass the default thresholds; the
wrapped-text page still exceeds them (MAE 0.0237, changed 6.7%) because LibreOffice
uses taller Arial line spacing and draws default white cell borders from the
unresolved table style. Thresholds have not been relaxed.

## Bullets and indentation

`buChar` and `buAutoNum` are placed separately from text. Bullet position is
`marL+indent`; body and wrapped lines start at `marL`; `marR` defines the right
margin. Paragraph/list/master settings, bullet font/size/color, and `buNone`
overrides are supported. Bullet fonts use the same resolution rules as body text.

Automatic numbering supports Arabic, alphabetic, and Roman sequences with periods,
closing parentheses, paired parentheses, or no decoration. Counters are per level.
A direct paragraph `startAt` restarts numbering; an inherited start applies only
at the first entry. Picture bullets are unsupported. Other numbering formats and
non-left-aligned bullet positions produce approximation warnings.

Fixture: `examples/text-bullets.pptx`; generator: `tests/visual/make_text_bullets.py`.
Numbers, levels, and wrapping were checked against LibreOffice, but vertical
position and line-spacing differences still exceed existing image thresholds.

A paragraph with no visible text (no runs, or only whitespace runs) draws no bullet
marker, because Office shows none; the empty line keeps its height and the paragraph
still advances the automatic-numbering counter, so numbering of later items is
unchanged. This is normal input and emits no diagnostic. Fixture:
`examples/empty-bullets.pptx` (generator `tests/visual/make_empty_bullets.py`).

## Hidden slides

`p:sld/@show` is read for every slide. Hidden slides (`0` or `false`) are not rendered
unless `--include-hidden` is passed, and the manifest records them (`hidden`,
`skipped_hidden_slides`). This matches LibreOffice's PDF export, which also skips them,
so the comparator pairs pages one to one. See [usage](usage.md#hidden-slides).

## Hidden elements

Shapes, pictures, connectors, groups, and non-OLE graphic frames marked hidden
(`p:cNvPr/@hidden`) are not drawn, on slides, layouts, and masters. A hidden group hides
all of its children, and a hidden child inside a visible group hides only itself. This
is silent: hiding is normal in real decks, so no diagnostic is emitted. A hidden OLE
object is the exception and keeps its `omitted` diagnostic; see
[OLE previews](ole-preview.md). Generator: `tests/visual/make_hidden.py`.

## Shape property extensions

An `a:extLst` in shape properties is inspected element by element. The Office 2010
extensions `a14:hiddenFill`, `a14:hiddenLine`, `a14:hiddenEffects`,
`a14:hiddenScene3d`, and `a14:hiddenSp3d` only remember the fill, line, effects, or 3-D
settings a shape had before they were switched off (for example by `a:noFill`), so they
are ignored by design and emit no diagnostic. Any other extension element is reported
once per shape as `Unsupported shape property extension: <prefix:name>`.

## Flipped shapes and groups

`flipH`/`flipV` on a shape or on any ancestor group mirror the geometry (outline,
fill, preset text rectangle, pictures) but never the text. Each flip along the
chain toggles a mirror; when the net transform is a mirror, the text is mirrored
once more around the centre of its final text rectangle. As a result `flipH`
text reads normally inside the mirrored text rectangle (for example the body of a
flipped arrow), `flipV` text is turned 180 degrees, and `flipH`+`flipV` (or a
flipped child inside a group flipped the same way) is a plain 180-degree or
0-degree rotation. This matches LibreOffice on the fixture below; PowerPoint was
not compared directly. Table cell text follows the same rule: a table inside a flipped
group (or with a flipped frame) has its grid mirrored while each cell's text is mirrored
once more around the centre of the cell, so it stays readable (`flipV` turns it 180
degrees). LibreOffice does not mirror such a table at all (it keeps the grid upright and
offsets it), so this case has no matching reference; it is covered by `tests/flips.rs`,
which compares against the unflipped table.

Fixture: `examples/flipped-text.pptx` with per-cell regions
`examples/flipped-text.json`; generator: `tests/visual/make_flipped_text.py`.
At `--width 1600` on macOS the page passes (MAE 0.0089, changed 0.0237). The
flipH, flipV, and unflipped cells pass locally; the other cells keep a
small vertical text offset and outline anti-aliasing difference that the unflipped
cell also shows (MAE 0.0198-0.0254, changed 0.047-0.059), so they still exceed
the unchanged local thresholds. Slide 2 holds the table cells; its group-flipped tables
exceed the thresholds against LibreOffice for the reason above (the unflipped table cell
stays close, MAE 0.0408, changed 0.0655).

## Freeform shapes

Custom geometry (`a:custGeom`) is drawn with the same guide evaluator as the presets.
Guides in `a:avLst` and `a:gdLst` are evaluated in document order in the shape
coordinate space (EMU), so formulas may use the built-in guides (`w`, `h`, `ss`,
`hc`, `vc`, `wd2`, `cd4`, ...) and earlier guides. Each `a:path` in `a:pathLst` is
scaled from its own `w`/`h` coordinate space to the shape extent; when `w` or `h` is
omitted or zero, the path uses shape coordinates (EMU) on that axis. Supported path
commands are `moveTo`, `lnTo`, `cubicBezTo`, `quadBezTo`, `arcTo` (ellipse arc
from the current point with `wR`/`hR`/`stAng`/`swAng`), and `close`; a path that does
not start with `moveTo` starts at the origin. `fill="none"` leaves a path unfilled
and `stroke="0"` leaves it unstroked; the shape fill and outline apply to the other
paths. An open path with fill is filled as if closed. Line-end arrowheads are kept
on open paths only. `extrusionOk`, `a:ahLst`, and `a:cxnLst` do not affect the
image. The text area is `a:rect` (evaluated through the same guides) or the whole
shape when it is absent.

Approximations are reported: the `lighten`, `lightenLess`, `darken`, and
`darkenLess` path fill modes are drawn as a normal fill (`approximation`), and an
unknown path command is skipped (`unsupported`). If a guide cannot be resolved (an
unknown name or operator) and a path or the text area becomes non-finite, the
outline is omitted with an `unsupported` diagnostic and the text uses the whole shape.

Fixture: `examples/custom-geometry.pptx` with per-cell regions
`examples/custom-geometry.json`; generator: `tests/visual/make_custom_geometry.py`.
It covers a python-pptx freeform builder polygon, a cubic Bezier edge, `arcTo`
corners, an open `fill="none"` quadratic curve, a `gdLst` bracket whose path has no
`w`/`h`, and a `stroke="0"` fill path under a `fill="none"` outline path. At
`--width 1600` on macOS the page passes against LibreOffice (MAE 0.0011, changed
0.0074; every cell MAE <= 0.0023, changed <= 0.017). Before this support the same
page measured MAE 0.1644, changed 0.4060. PowerPoint was not compared directly.

## Picture effects and fills

Children of a picture's `a:blip` are applied as one SVG filter, in document order:

| Element | Rendering | Diagnostic |
|---|---|---|
| `a:alphaModFix` | Alpha multiplied by `amt` | none |
| `a:grayscl` | Saturation 0 (Rec. 709 luma) | none |
| `a:biLevel` | Black or white by luma against `thresh` | none |
| `a:lum` | Linear brightness/contrast around mid-grey | `approximation` |
| `a:duotone` | Luma mapped between the two colors | `approximation` |
| `a:clrChange` | Pixels exactly equal to `clrFrom` become `clrTo` (with its alpha); no tolerance | `approximation` |
| `a14:saturation` (in `a14:imgProps`) | Saturation scaled by `sat` | none |
| `a14:brightnessContrast` (in `a14:imgProps`) | Same mapping as `a:lum` | `approximation` |
| `a14:backgroundRemoval`, `a14:sharpenSoften`, other `a14:imgEffect` children | Picture drawn unchanged | `unsupported`, `Unsupported picture effect: <name>` |
| Other `a:blip` children (for example `a:alphaBiLevel`, `a:hsl`, `a:tint`) | Ignored | `unsupported`, `Unsupported picture effect: <name>` |

Extensions that do not change pixels produce no diagnostic: `asvg:svgBlip` (handled
above) and `a14:useLocalDpi`. Background removal stores only marks; the removed area
is not in the package, so it is not reproduced. Any other extension element is named
in an `unsupported` diagnostic (`Unsupported picture extension: <name>`). Effects that
leave pixels unchanged (`alphaModFix` without `amt`, an empty `lum`) add no filter.

`a:tile` repeats the picture (after `srcRect` cropping) over the picture frame. The
tile size is the picture's natural size scaled by `sx`/`sy`; the natural size uses the
PNG `pHYs` or JPEG JFIF density, and 96 dpi when neither is present (EXIF resolution is
not read). `algn` places the first tile, `tx`/`ty` offset it, and `flip` mirrors every
other tile along x, y, or both. A tiled SVG picture has no natural size and uses the
frame as the tile (`approximation`). A tile so small that more than 256 tiles fall
along one axis of the frame, or a pattern cell (with flips) larger than 16 times the
frame area, is not tiled: the picture is drawn stretched with an `approximation`
diagnostic (`Picture tile size is out of range; drawn stretched: <part>`), and image
densities below 1 dpi are ignored, so a deck cannot inflate the rasterized pattern.
`a:stretch/a:fillRect` insets (`l`, `t`, `r`,
`b`) shrink or, when negative, extend the stretched picture inside the frame; the frame
still clips it, and an inset that leaves no area omits the picture (`Image fill
rectangle leaves no visible area: <part>`). These apply to `p:pic` pictures (and OLE
previews). Picture fills of shapes and backgrounds (`a:blipFill` in `p:spPr` or `p:bgPr`)
go through the same code, so `a:tile` on them repeats the picture over the shape's box
with the same tile-size rules and bounds, and records no tiling diagnostic.

Tile density is not verified against PowerPoint (unmeasured): on a self-made fixture
(a picture without `pHYs`, `sx`/`sy` of 50%) LibreOffice draws about 9 tile columns and
this renderer about 7, because the 96 dpi default above differs from LibreOffice's. The
default is kept until PowerPoint has been measured.

Fixture: `examples/image-effects.pptx` with per-picture regions
`examples/image-effects.json`; generator: `tests/visual/make_image_effects.py`. Each
of its 13 slides shows the original picture on the left and the effect on the right,
over a checkerboard. At `--width 1600` on macOS against LibreOffice 26.2:

| Slide | Before (main 0f85650): page MAE / changed, effect region | After: page MAE / changed, effect region |
|---|---|---|
| 1 alphaModFix | 0.0408 / 0.1672 fail, 0.2014 / 0.8877 | 0.0086 / 0.0262 pass, 0.0147 / 0.0718 fail |
| 2 grayscl | 0.0326 / 0.1471 fail, 0.1693 / 0.7929 | 0.0093 / 0.0384 pass, 0.0346 / 0.1640 fail |
| 3 biLevel | 0.0528 / 0.1486 fail, 0.2802 / 0.7929 | 0.0102 / 0.0179 pass, 0.0337 / 0.0364 fail |
| 4 lum | 0.0286 / 0.1548 fail, 0.1158 / 0.7958 | 0.0143 / 0.0278 pass, 0.0333 / 0.0614 fail |
| 5 a14 saturation | 0.0065 / 0.0185 pass, 0.0083 / 0.0343 | 0.0350 / 0.1502 fail, 0.1733 / 0.7967 fail |
| 6 a14 brightnessContrast | 0.0109 / 0.0245 pass, 0.0083 / 0.0343 | 0.0291 / 0.1557 fail, 0.1139 / 0.7934 fail |
| 7 duotone | 0.0417 / 0.1879 fail, 0.2019 / 0.9995 | 0.0111 / 0.0359 pass, 0.0247 / 0.1203 fail |
| 8 clrChange | 0.0246 / 0.0464 fail, 0.0756 / 0.1430 | 0.0137 / 0.0292 pass, 0.0121 / 0.0436 pass |
| 9-11 tile (9a-9c) | pass (picture stretched once, as LibreOffice draws it) | 0.0761-0.0777 / 0.178-0.182 fail, region MAE 0.40-0.42 |
| 12 fillRect (10) | pass (as LibreOffice) | 0.0609 / 0.1648 fail, 0.3090 / 0.8633 fail |
| 13 metadata + backgroundRemoval (11) | 0.0160 / 0.0311 pass | unchanged |

LibreOffice 26.2 draws `p:pic` tiles and fill rectangles as one stretched picture and
ignores `a14:imgProps` corrections, so slides 5, 6, and 9-12 differ from it by design;
the remaining region failures on slides 1-4 and 7 are edge resampling and the differing
effect curves, and are kept at the unchanged thresholds. PowerPoint was not compared
directly.

## EMF and WMF pictures survey

pptx2img does not render EMF or WMF yet: such a picture is omitted with
`Only PNG/JPEG images are supported: <part>` (see [Known limitations](#known-limitations)).
This section records the step 1 survey of issue #78 (2026-10-20), made before any
implementation. It does not change rendering.

Fixtures: `tests/visual/make_metafile_pictures.py OUT_DIR [--pptx]` writes nine
metafiles with the standard library only, from MS-EMF and MS-WMF, plus a deck with one
picture per slide. Each file isolates a record group: `emf-shapes` (also with an 88-byte
and a 108-byte header), `emf-path`, `emf-transform`, `emf-clip`, `emf-bitmap`,
`emf-text`, and the placeable WMFs `wmf-shapes` and `wmf-clip-bitmap`. All nine pass
`emfsdk`'s strict validator, which was used only as a conformance check on the fixtures.
The reference drawing is LibreOffice's rendering of the same files, a comparison
reference and not PowerPoint ground truth.

### Candidates

Versions and dates are from crates.io and GitHub on 2026-10-20. No other crate for EMF
or WMF turned up in a search of crates.io (`emf`, `wmf`, `metafile`, `windows metafile`).

| | `emf-core` 0.1.1 (+ `wmf-core` 0.1.2) | `emfsdk` 0.2.0 |
|---|---|---|
| Maintenance | First release 2026-08-03, latest 2026-09-05; repository pushed 2026-10-03 (EMF) and 2026-09-20 (WMF); 4 and 3 open issues, 4 and 6 stars; the README says some records are not yet implemented | First release 2026-07-20, no release since; repository pushed 2026-10-07; no open issues, no stars; "pre-1.0, API may change" |
| Licence | MIT (`emf-core`, `wmf-core`); the crate ships a `LICENSE` file | `MIT OR Apache-2.0`; the published package carries no licence file |
| Transitive dependencies | `snafu`, `strum`, `embedded-io`, `tracing`, `encoding_rs`, `codepage`, `unicode-*`, `base64`; all permissive | Parsing only: `bitflags`, `encoding_rs`, `thiserror`. With `render`: also `image`, `fontdb`, `ttf-parser`; all permissive |
| Output | SVG string (`converter::convert_to_svg`); WMF input falls back to `wmf-core` automatically; `Player` trait for other outputs | Typed records for parse/edit/write; the optional `render` feature produces a PNG (`decode_metafile_as_raster`), not vector data |
| SVG or pixels | SVG, but with `viewBox` only (no `width`/`height`) and bitmaps as `data:image/bmp;base64` | Pixels only; a PNG would bypass the SVG pipeline but would pass through the existing PNG/JPEG size policy |

Attribution fits the project's policy for either licence (a notice for the MIT text and
copyright is enough); licences are not what separates the candidates.

### Record coverage on the fixtures

`ok` means the drawing matches the expected shapes, `partial` means drawn with a visible
defect, `no` means the record had no effect, and `error` means the whole file failed.

| Record group (fixture) | `emf-core` / `wmf-core` (SVG) | `emfsdk` `render` (PNG) |
|---|---|---|
| Rectangle, ellipse, polygon, polyline, MoveToEx/LineTo, pens, brushes (`emf-shapes`, `wmf-shapes`) | `ok`, except `EMR_ROUNDRECT` is not drawn | `ok`, but only when the header is 108 bytes: with the 88-byte (or 100-byte) header, which MS-EMF allows, the call returns `None` without a message |
| Curves, `FillPath`, `StrokePath` (`emf-path`) | `ok` (fill and stroke) | `partial`: stroke only, `STROKEANDFILLPATH` is not filled |
| `SelectClipPath` and shapes inside a path bracket (`emf-path`) | `no`: the clip is ignored and the ellipse inside the bracket is drawn immediately | `no`: same |
| Map mode, window/viewport, `SetWorldTransform`, `ModifyWorldTransform` (`emf-transform`) | `no`: only the untransformed square is drawn | `ok` |
| `IntersectClipRect` (`emf-clip`, `wmf-clip-bitmap`) | `no`: the clip is ignored | `ok` |
| `ExcludeClipRect` (`emf-clip`) | `no` | `no` (LibreOffice does not draw the hole either, so the expectation is unverified) |
| `StretchDIBits`, `BitBlt`, WMF `StretchDIB` (`emf-bitmap`, `wmf-clip-bitmap`) | `partial`: drawn at the source size, not stretched to the destination | `ok` |
| Text: `ExtCreateFontIndirectW`, `ExtTextOutW`, WMF `TextOut` (`emf-text`, `wmf-clip-bitmap`) | EMF `error`: a conformant 92-byte `LogFontW` makes the whole file fail to parse (`elw` must be at least 320 bytes); WMF text is drawn | `ok` |
| WMF placeable header (`wmf-shapes`, `wmf-clip-bitmap`) | `ok` | `ok` |

### Recommendation

Implement the subset independently from MS-EMF and MS-WMF in step 2; do not adopt either
crate.

- Neither crate covers the subset the issue names. `emf-core` ignores clipping and world
  transforms and rejects the whole file on a common text record, while step 2 requires
  text records to be reported as `unsupported` and the rest to render. `emfsdk` is
  raster-only, does not fill paths, and silently declines two of the three header sizes.
- The output form matters as much as coverage. `emf-core` SVG carries bitmaps as `data:`
  URIs, which the SVG picture loader rejects by design (they would bypass the PNG/JPEG
  size policy), and has no intrinsic size. `emfsdk` PNG fixes the resolution before the
  slide scale is known. An independent decoder can emit paths and run bitmaps through
  the existing size policy.
- Both crates are weeks old, pre-1.0, with one maintainer and almost no issue history,
  so a defect found later would be ours to fix either way.

`emfsdk`'s strict validator is useful during development as a check that generated
fixtures are conformant; that would be a development-time tool, not a dependency of the
CLI. Text records stay `unsupported` in step 1 and step 2 either way, and OLE previews
stored as WMF/EMF go through the same decoder.

## Known limitations

- Complete layout/master property and text-style inheritance; `themeOverride` and
  full theme style matrices (individual fill/line references have partial support).
- Full Japanese line-breaking rules, punctuation compression, all hanging symbols,
  complex shaping/ligatures/kerning, and dynamic autofit ratio calculation.
- Identical baselines and fallback behavior across all fonts and Office renderers.
- Charts, SmartArt, freeform path fill modes other than
  normal and none, and many presets.
- Full table theme-style coverage, theme/line/table-cell/text gradients,
  inner shadows/glow, theme effects, and the picture effects listed as
  unsupported under [picture effects and fills](#picture-effects-and-fills).
  Selected color alpha and outer shadows are supported; see the feature-specific
  notes for their limits.
- EMF/WMF images, SVG data referenced directly from `a:blip/@r:embed`, text inside
  SVG pictures, external links, animations, Strict OOXML, legacy `.ppt`, and
  encrypted files.

SVG pictures stored the Office 2016 way (`a:blip/a:extLst/a:ext/asvg:svgBlip/@r:embed`,
as inserted icons are) are rendered as vector data, stretched to the picture frame and
cropped like other pictures. When `a:blip/@r:embed` also names a PNG/JPEG fallback,
the fallback is drawn first, so pictures that already rendered are unchanged; the SVG
is used when there is no fallback or the fallback cannot be drawn. If neither can be
drawn, the picture is omitted once with both reasons. An SVG picture is rendered only
when it is at most 4 MiB, has at most 100,000 XML nodes, parses as SVG, and references
nothing outside itself: any `href` other than a local `#id`, any `url(...)` other than
`url(#id)`, and CSS `@import` are rejected, including `data:` images, which would
otherwise bypass the PNG/JPEG size policy below. `<text>` inside an SVG picture is
reported as `unsupported` because document fonts are not resolved for it. A picture
whose `a:blip` has no `r:embed` of either kind is omitted with `Missing image
reference`; `Externally linked images are not loaded` is reserved for `r:link` and
`TargetMode="External"` relationships.

Audio and video are never played or rendered. A slide, layout, or master that has a
relationship of type `video`, `audio`, or `media` gets one `unsupported` diagnostic
per slide, `Audio and video playback is unsupported`. Media parts are not loaded, so
they do not count toward the size limits below, and a media payload larger than the
32 MiB per-part limit does not fail the deck; the poster frame, being an ordinary
picture, renders. Generator: `tests/visual/make_embedded_media.py` (writes a deck of
about 40 MiB uncompressed, so write it outside the repository).

Pictures that are not PNG/JPEG (for example EMF, WMF, SVG, or WDP), or whose
dimensions cannot be read, are omitted with an `omitted` diagnostic naming the
media part; the rest of the slide, layout, and master still renders.
The same applies to a picture whose `r:embed` names no relationship
(`Missing image relationship: <id>`), whose relationship target is absent from the
package (`Missing image part: <part>`), or whose crop (`srcRect`) leaves no visible
area (`Image crop leaves no visible area: <part>`). A part that a picture's `a:blip`
or `asvg:svgBlip` references is loaded even when its relationship type is one the
loader otherwise skips (for example `oleObject` or `hdphoto` written by third-party
tools), so the picture is rendered or omitted on its own; such parts count toward the
size limits below.

Visual output depends on available fonts. Ordinary CI covers Windows/Linux/macOS;
recorded visual comparison measurements are from macOS.

## Resource limits

Input size: 128 MiB. Total uncompressed data: 256 MiB. Per part: 32 MiB.
Maximum slides: 200. Group nesting: 64 levels. Tables: 100000 cells.
Output images have pixel-count limits. Embedded PNG/JPEG pictures are handled per
picture and never abort the deck:

- Up to 8192 px per side and 16,777,216 pixels: embedded unchanged.
- Above that, up to 32768 px per side and 67,108,864 pixels (64 Mpx): decoded and
  box-filtered so the longest side is at most 2048 px, then embedded with an
  `approximation` diagnostic that records the original and downscaled sizes. The
  2048 px target bounds memory; it is not derived from the output width, so a
  picture shown large or heavily cropped can look softer than in Office.
  Non-interlaced PNG is decoded row by row; JPEG is decoded at full size first.
- Above the ceiling, interlaced (Adam7) PNG above the first limit, or data that
  fails to decode: omitted with an `omitted` diagnostic naming the media part.

ZIP entries are read without extracting files to the filesystem. External images
are not fetched. Recursive conversion uses an 8 MiB thread stack so OS main-stack
size differences do not cause parser failures. Tests require a normal error exit
and no output for groups deeper than 64, rather than accepting a crash as rejection.
Additional robustness validation is needed before exposing a service to arbitrary
untrusted input.

For font requirements and diagnostics, see [usage](usage.md). For measurement
models, sources, and historical comparison evidence, see [rendering notes](rendering-notes.md).
