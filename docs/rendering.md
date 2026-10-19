# Rendering support and limitations

This is an early, approximate renderer, not a complete PowerPoint implementation.
Unsupported content is reported where detected; detection itself is incomplete.
Strict mode gates known warnings, not all incompatibilities.

## Supported subset

- Transitional OOXML PPTX, using presentation-defined slide order.
- Explicit positions, sizes, rotation, and flips for basic shapes and selected
  [presets and connectors](preset-shapes.md), including chevrons, triangles,
  arrows, pie, star5, cloud, callouts, flowchart shapes, and bent/curved connectors
  evaluated from DrawingML shape guides, and [freeform shapes](#freeform-shapes)
  (`a:custGeom`).
- Solid backgrounds, fills, and lines using RGB, system `lastClr`, and theme
  colors; selected [color transforms and theme fills](colors.md), including
  translucent (`alpha`) fills, lines, and text, and `tint`/`shade`/`satMod`;
  linear and radial [gradient fills](colors.md#gradient-fills) on shapes and
  backgrounds.
- Slide -> layout -> master -> theme relationships, color mappings, and major/
  minor Latin and Japanese theme fonts, including the Jpan supplemental font.
- Explicit font, size, color, bold/italic, and left/center/right paragraph alignment.
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
and margins. Table-cell/vertical autofit and horizontal expansion without wrapping
are unsupported and warned about. Shape text areas are remeasured at their final
size, with diagnostics for expansion limits.

Fixture: `examples/text-autofit.pptx`; generator:
`tests/visual/make_text_autofit.py`. LibreOffice 26.2.5.2 recalculates this fixture's
saved shrink ratios and does not expand its saved boxes, so pixel differences
remain. Thresholds have not been relaxed.

- [NormalAutoFit](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit)
- [Office spAutoFit interpretation](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/19d31037-28bf-4693-85be-3a37bfe82d51)

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

## Known limitations

- Complete layout/master property and text-style inheritance; `themeOverride` and
  full theme style matrices (individual fill/line references have partial support).
- Full Japanese line-breaking rules, punctuation compression, all hanging symbols,
  complex shaping/ligatures/kerning, and dynamic autofit ratio calculation.
- Identical baselines and fallback behavior across all fonts and Office renderers.
- Charts, SmartArt, group-fill inheritance, freeform path fill modes other than
  normal and none, and many presets.
- Full table theme-style coverage, theme/line/table-cell/text gradients,
  inner shadows/glow, theme effects,
  and image tiling/effects/fill rectangles. Selected color alpha and outer shadows
  are supported; see the feature-specific notes for their limits.
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
