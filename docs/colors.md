# Color transforms and theme fills

Apply color transforms in document order. Do not round intermediate values to
8 bits; convert the final color back to RGB.

| Transform | Formula (values in 0–1, `val / 100000`) |
|---|---|
| `lumMod`, `lumOff` | HSL luminance `L * val` / `L + val`, clamped to 0–1; hue and saturation are kept, so they survive an intermediate black or white |
| `satMod` | HSL saturation `S * val`, clamped to 0–1 |
| `tint` | per channel in linear RGB: `c' = 1 - (1 - c) * val` (mix with white) |
| `shade` | per channel in linear RGB: `c' = c * val` (mix with black) |
| `alpha`, `alphaMod`, `alphaOff` | opacity `val`, `A * val`, `A + val`, clamped to 0–1 |

ECMA-376 Part 1 §20.1.2.3.34 (`tint`) and §20.1.2.3.31 (`shade`) define a tint as
a mix with white and a shade as a mix with black. They are computed in linear RGB
using the IEC 61966-2-1 sRGB transfer function (the scRGB gamma), then converted
back to sRGB. Example: `4472C4` with `tint 20000` is `E9EBF5`; with `shade 50000`
it is `2F528F`.

Opacity is emitted as an 8-digit `#RRGGBBAA` color, so it applies wherever the
color is used: shape and table-cell fills, lines (including arrowheads), slide
backgrounds, and text. Opacity is quantized to 8 bits. When a theme fill uses
`phClr`, the reference color's opacity carries into the theme style, and the
style's own alpha transforms then apply. Shadows keep their separate opacity.
All other transforms (`hueMod`, `satOff`, `gamma`, `comp`, `gray`, …) still
produce `Unsupported color transform` warnings.

Shape fillRef resolves against theme fmtScheme fillStyleLst (indices starting at 1)
and bgFillStyleLst (starting at 1001), passing the reference color as phClr.
Explicit noFill/solidFill wins. Unknown transforms, non-solid theme fills, and
unresolved references produce warnings. This color-fill implementation does not
cover line, effect, or text theme references; line support is documented separately.

Implemented independently from ECMA-376 Part 1 DrawingML and the
[Open XML SDK FillReference description](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.fillreference).
No ibis-office code, tests, or assets were reused.

Fixture: `python tests/visual/make_colors.py /tmp/colors.pptx`. Page 1 covers
luminance and theme fillRef; page 2 covers a translucent white overlay
(`alpha 76090`) and a translucent line over colored stripes, accent tint/shade/
satMod swatches, a translucent accent fill, and a table whose tableStyles part
defines the built-in "Medium Style 2 - Accent 1" ID with tinted body bands.
The table style definition is written for the fixture, not copied from Office.

Comparison at 1600px with LibreOffice 26.2.5.2 (2026-10-19, unchanged thresholds
MAE 0.02 / changed pixels 5% / channel tolerance 16):

| Page | Before (`e103c5d`) | After |
|---|---|---|
| 1 | MAE 0.000589, changed 0.422% | MAE 0.000589, changed 0.422% |
| 2 | MAE 0.182705, changed 58.703% (fail) | MAE 0.002438, changed 0.730% |

Remaining page-2 differences are table text font substitution and anti-aliasing;
matching LibreOffice is not proof of PowerPoint compatibility.

Local inspection of source pages 6/14 found the pale blue region changed from
RGB (25,163,252) to (210,237,254), closer to reference (209,237,254).
The gray panel fill was reproduced; at that stage, surrounding shadow differences
came from unsupported effectLst. See [later shadow support](shadows.md).
Text and bullet differences are separate issues. Source images were not published.

## Gradient fills

Explicit `gradFill` on shapes (including presets and connectors' fill areas) and
slide backgrounds renders as an SVG gradient in the fill box, in local shape
coordinates so it follows rotation and flips. Stops come from `gsLst`, are sorted
by `pos`, and use the color transforms above; stop alpha becomes `stop-opacity`.
Before the first and after the last stop the end colors repeat.

- `lin ang`: a linear gradient through the box center along the angle (clockwise,
  y down) whose ends touch the box corners. Without `lin` or `path`, angle 0.
- `lin scaled="1"`: the angle applies to a unit square that is stretched over the
  box (the gradient is scaled with the shape). LibreOffice 26.2.5.2 ignores
  `scaled`, so non-square scaled gradients differ from it; they produce an
  `approximation` diagnostic because PowerPoint behavior was not verified.
- `path`: a circular radial gradient with stop position 0 at the center of
  `fillToRect` and position 1 at the farthest box corner. Only `circle` with a
  centered focus matches LibreOffice; other focus positions and `rect`/`shape`
  paths produce `approximation` diagnostics.
- `rotWithShape="0"` on rotated shapes and non-zero `tileRect` produce
  `approximation` diagnostics; the gradient still rotates with the shape and is
  not tiled.

Not covered: gradients from theme fill references (`fillRef` to a `gradFill` style,
still warned), gradient lines, gradient table cells, and gradient text.

Fixture: `python tests/visual/make_gradients.py /tmp/gradients.pptx`. Page 1 has
linear 0°/90°/45° (unscaled and scaled), three stops, a translucent stop over
stripes, and centered/corner radial gradients; page 2 has a gradient background,
a full-width gradient band, a rotated chevron, and a radial oval.

Comparison at 1600px with LibreOffice 26.2.5.2 (2026-10-19, unchanged thresholds):

| Page | Before (`e103c5d`) | After |
|---|---|---|
| 1 | MAE 0.191180, changed 48.150% (fail) | MAE 0.010892, changed 4.784% |
| 2 | MAE 0.184885, changed 76.458% (fail) | MAE 0.005589, changed 2.223% |

Most of page 1's remaining difference is in the two known-different shapes (scaled
45° on a wide box, and the corner-focus radial gradient), which are kept in the
fixture as reproduction material.

## Theme backgrounds, pattern, picture, and group fills

- **Theme background references** (`p:bg/p:bgRef`): `idx` 1001 and above select an entry
  of the theme's `bgFillStyleLst` (`idx - 1001`), and the reference's color child
  replaces `phClr`, exactly like shape `fillRef`. The nearest slide, layout, or master
  with a `p:bg` supplies the background. Only solid `bgFillStyleLst` entries are
  resolved; gradient or picture entries keep the default white and record
  `Theme fill reference <idx> is unresolved or not a solid fill`.
- **Pattern fills** (`a:pattFill`) on shapes and backgrounds become an SVG `<pattern>`
  that repeats an 8x8 tile of `fgClr` over `bgClr` (defaults black over white). One
  tile pixel is one slide unit (1/96 inch), in shape coordinates, so the pattern scales,
  rotates, and flips with the shape. The tiles for `pct5`…`pct90` (ordered dither),
  `horz`/`vert` and their `lt`/`dk`/`nar` variants, the `upDiag`/`dnDiag` families
  (`lt`, `dk`, `wd`), `cross`, `smGrid`, `lgGrid`, `diagCross`, `trellis`, `smCheck`,
  and `lgCheck` are this project's own drawings of what the names describe; they are
  not verified against PowerPoint, so every pattern fill records an `approximation`
  diagnostic. Other presets are drawn as `pct50` and record
  `Unsupported pattern fill preset: <prst>; drawn as pct50`. Pattern lines, table cells,
  and text are not covered.
- **Picture fills on shapes and backgrounds** (`a:blipFill` in `p:spPr` or `p:bgPr`)
  stretch the picture (after `srcRect` cropping) over the shape's box, and the shape
  outline clips it. The picture goes through the same loading, size, and omission
  rules as pictures; an omitted picture leaves the shape unfilled. Tiling (`a:tile`)
  and non-empty `fillRect` still record `Image tiling, effects, and fill rectangles are
  unsupported` and are drawn stretched.
- **Group fills** (`a:grpFill`): a child uses the fill of the nearest enclosing group
  that declares one; a group that itself says `a:grpFill` passes its parent's fill on,
  and a group without a fill leaves such children unfilled. Fill elements on a group
  no longer produce a diagnostic (the group itself draws nothing); other group
  properties still record `Group effect inheritance is unsupported`. A gradient group
  fill is drawn per child box rather than across the group and records an
  `approximation` diagnostic.

Fixture: `python tests/visual/make_fills.py examples/fills.pptx`. Page 1 has a master
background `bgRef idx="1001"` with a tinted accent and six pattern presets (`dkUpDiag`,
`trellis`, `ltUpDiag`, `horz`, `cross`, `pct50`); page 2 has a solid group whose
children use `grpFill` (one through a nested `grpFill` group) and a stretched picture
fill on an ellipse and a rounded rectangle.

Comparison at 1600px with LibreOffice 26.2.5.2 (2026-10-20, unchanged thresholds):

| Page | Before (`0f85650`) | After |
|---|---|---|
| 1 | MAE 0.172544, changed 72.095% (fail) | MAE 0.186010, changed 46.988% (fail, known difference) |
| 2 | MAE 0.284853, changed 100.000% (fail) | MAE 0.000829, changed 0.406% |

Page 1 is kept as a known difference. The background now matches; the remaining
difference is the hatching. LibreOffice draws patterns at device resolution (a period
of about 4 output pixels at 1600px) and draws `dkUpDiag`, `trellis`, and `ltUpDiag`
with the same light hatch, so its mean tone differs from both the named weights and
this renderer's tiles. Matching LibreOffice here would mean copying its choice rather
than the preset names, so the tiles are left as described above, and the page stays
a reproduction of the gap.
