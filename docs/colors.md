---
kind: guide
---

# Color transforms and theme fills

Color transforms apply in document order without rounding intermediate values to 8 bits; the final color is converted back to RGB (`src/pptx/colors.rs`).

| Transform | Formula (values in 0-1, `val / 100000`) |
|---|---|
| `lumMod`, `lumOff` | HSL luminance `L * val` / `L + val`, clamped; hue and saturation are kept, so they survive an intermediate black or white |
| `satMod` | HSL saturation `S * val`, clamped |
| `tint` | per channel in linear RGB: `c' = 1 - (1 - c) * val` (mix with white) |
| `shade` | per channel in linear RGB: `c' = c * val` (mix with black) |
| `alpha`, `alphaMod`, `alphaOff` | opacity `val`, `A * val`, `A + val`, clamped |

ECMA-376 Part 1 §20.1.2.3.34 (`tint`) and §20.1.2.3.31 (`shade`) define a mix with white and black. The mix is computed in linear RGB using the IEC 61966-2-1 sRGB transfer function and converted back to sRGB; for example `4472C4` with `tint 20000` is `E9EBF5` and with `shade 50000` is `2F528F`. All other transforms (`hueMod`, `satOff`, `gamma`, `comp`, `gray`, ...) produce `Unsupported color transform` warnings.

Opacity is emitted as `#RRGGBBAA`, quantized to 8 bits, wherever the color is used: shape and table-cell fills, lines and arrowheads, backgrounds, and text. When a theme fill uses `phClr`, the reference color's opacity carries into the style and the style's own alpha transforms then apply; shadows keep their separate opacity.

Shape `fillRef` resolves against the theme `fillStyleLst` (indices from 1) and `bgFillStyleLst` (from 1001), passing the reference color as `phClr`; explicit `noFill`/`solidFill` wins. Non-solid theme fills and unresolved references warn. Line theme references are described in [preset shapes](preset-shapes.md#theme-line-references); effect and text theme references are unsupported. Implemented independently from ECMA-376 Part 1 and the [Open XML SDK FillReference description](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.fillreference); no ibis-office code was reused. Generator: `tests/visual/make_colors.py` (luminance and `fillRef`; translucent overlays and lines, tint/shade/satMod swatches, a table with a fixture-defined "Medium Style 2 - Accent 1" style).

## Gradient fills

Explicit `gradFill` on shapes and slide backgrounds renders as an SVG gradient in local shape coordinates, so it follows rotation and flips (`src/pptx/gradients.rs`). Stops come from `gsLst` sorted by `pos`, use the transforms above, and stop alpha becomes `stop-opacity`; end colors repeat beyond the first and last stop.

- `lin ang`: a linear gradient through the box center along the angle (clockwise, y down) whose ends touch the box corners; without `lin` or `path`, angle 0.
- `lin scaled="1"`: the angle applies to a unit square stretched over the box. LibreOffice ignores `scaled`, and PowerPoint was not verified, so non-square scaled gradients record `approximation`.
- `path`: a circular radial gradient with position 0 at the center of `fillToRect` and 1 at the farthest corner. Only `circle` with a centered focus matches LibreOffice; other focus positions and `rect`/`shape` paths record `approximation`.
- `rotWithShape="0"` on rotated shapes and non-zero `tileRect` record `approximation`; the gradient still rotates with the shape and is not tiled.

Not covered: gradients reached through `fillRef` (still warned), gradient lines, table cells, and text. Generator: `tests/visual/make_gradients.py`; the scaled 45° wide box and the corner-focus radial are kept in it as reproduction material for the known differences.

## Theme backgrounds, pattern, picture, and group fills

- **Theme background references** (`p:bg/p:bgRef`): `idx` 1001 and above select `bgFillStyleLst[idx - 1001]` with the reference color as `phClr`. The nearest slide, layout, or master with a `p:bg` supplies the background. Only solid entries are resolved; gradient or picture entries keep the default white and record `Theme fill reference <idx> is unresolved or not a solid fill`.
- **Pattern fills** (`a:pattFill`) become an SVG `<pattern>` repeating an 8x8 tile of `fgClr` over `bgClr`, one tile pixel per 1/96 inch in shape coordinates so the pattern scales, rotates, and flips with the shape (`src/pptx/patterns.rs`). The tiles for `pct5`-`pct90`, `horz`/`vert` and their variants, the `upDiag`/`dnDiag` families, `cross`, `smGrid`, `lgGrid`, `diagCross`, `trellis`, `smCheck`, and `lgCheck` are this project's own drawings of what the names describe and are not verified against PowerPoint, so every pattern fill records `approximation`. Other presets draw as `pct50` with `Unsupported pattern fill preset: <prst>; drawn as pct50`. Pattern lines, table cells, and text are not covered. LibreOffice draws hatching at device resolution with one light hatch for several presets; matching it would mean copying its choice rather than the preset names, so the fills fixture page stays a known difference.
- **Picture fills on shapes and backgrounds** (`a:blipFill` in `p:spPr` or `p:bgPr`) stretch the cropped picture over the shape's box, clipped by the outline, under the same loading, size, and omission rules as pictures; an omitted picture leaves the shape unfilled. `a:tile`, picture effects, and `fillRect` insets follow the rules in [rendering.md](rendering.md#picture-effects-and-fills) and record no tiling diagnostic.
- **Group fills** (`a:grpFill`): a child uses the fill of the nearest enclosing group that declares one; a group that itself says `a:grpFill` passes its parent's fill on; a group without a fill leaves such children unfilled. Fill elements on a group produce no diagnostic (the group draws nothing); other group properties record `Group effect inheritance is unsupported`. A gradient group fill is drawn per child box and records `approximation`.

Generator: `tests/visual/make_fills.py` (master `bgRef` with a tinted accent, six pattern presets, nested `grpFill`, picture fills on an ellipse and a rounded rectangle).
