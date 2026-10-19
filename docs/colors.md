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
