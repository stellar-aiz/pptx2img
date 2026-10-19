# Preset geometry and comparison evidence

Supported presets here are homePlate, upArrow, rightArrow, rightBrace, and
wedgeRoundRectCallout. Build outlines and text rectangles in local coordinates,
then apply existing rotation, flipping, and group transforms.

Formulas and paths in DrawingMLGeometries from the
[ECMA-376 Part 1 fifth-edition distribution](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
were used as specifications for an independent Rust implementation. upArrow uses
coordinate exchange from rightArrow. No ibis-office code, tests, or assets were
incorporated. Numeric `avLst` `val` adjustments are supported; unknown formulas
warn and use defaults. Unsupported presets still produce omitted diagnostics.

Generate independent fixtures without source-document assets:

```sh
python tests/visual/make_preset_shapes.py /tmp/presets.pptx
python tests/visual/compare.py --fixture /tmp/presets.pptx \
  --binary target/debug/pptx2img --out /tmp/presets-comparison --width 1600
```

On 2026-10-18 with LibreOffice 26.2.5.2, Hiragino Sans, and punctuation compression
OFF, the defaults page had MAE 0.000521/changed pixels 0.313%; the adjustments,
rotation, and flip page had 0.001012/0.473%. Thresholds remained MAE 0.02, changed
pixels 5%, and channel tolerance 16. Text-layout and theme-background diagnostics
remain; this does not establish complete Office compatibility.

## Lines and connectors

Support includes straightConnector1, bentConnector2, 11 prstDash patterns,
five headEnd/tailEnd shapes, cap, and join. Line width sets dash and head sizes.
Heads overlap shafts, leaving known differences at small arrows and stealth joins;
approximation diagnostics remain. custDash and other unsupported features warn.

Fixture: `python tests/visual/make_lines.py /tmp/lines.pptx`.
Historical three-page LibreOffice comparison: MAE 0.004429/0.005158/0.001855,
changed pixels 0.764%/2.025%/0.827%, within unchanged thresholds. Joins are not
proven identical.

## Theme line references

Resolve one-based lnRef `idx` against lnStyleLst and replace phClr with the
reference color. Apply theme lines, inherited lines, then nearest direct
properties. Resolve width, cap, compound attributes, fill, dash, join, and each
end separately. Direct noFill, end `type="none"`, and `prstDash="solid"` cancel
inherited values. Partial end definitions retain unspecified attributes.
`idx=0` means no line, with direct properties taking precedence. Unresolved indices,
custDash, compound lines, and other unsupported features still warn.

Independent implementation references:
[LineReference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.linereference?view=openxml-3.0.1),
[LineStyleList](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.linestylelist?view=openxml-3.0.1),
[Outline](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outline?view=openxml-3.0.1).

```sh
python tests/visual/make_theme_lines.py /tmp/theme-lines.pptx
python tests/visual/compare.py --fixture /tmp/theme-lines.pptx \
  --binary target/release/pptx2img --out /tmp/theme-lines-comparison --width 1600
```

The three pages cover theme styles, equivalent direct styles, and partial overrides
at 0.25pt. Regression tests check pixel identity, overrides, resets, and diagnostics.
Head joins remain approximate. Font substitution residuals are tracked separately
in [Issue #25](https://github.com/stellar-aiz/pptx2img/issues/25).

Independent measurements of 27 combinations (0.25/0.5/0.75/1/1.5/2/3/4/6pt and
sm/med/lg) against LibreOffice 26.2.5.2 showed constant head size through 2pt and
proportional size above 2pt. Head reference width is therefore approximated as
max(line width, 2pt), without thickening the shaft. Diagnostics explicitly describe
this approximation; it is not complete PowerPoint compatibility.
Reproduce with `python tests/visual/make_arrow_sizes.py /tmp/arrow-sizes.pptx`.

## Saved bent routes and connection sites

ECMA-376 Part 1 DrawingMLGeometries defines bentConnector2 as top-left -> top-right
-> bottom-right. Flip before rotating, following
[OOXML Primer sections 5.7.3 and 5.7.6](https://download.microsoft.com/download/e/1/4/e14fb96f-83b8-4a2a-84db-7fa8acbe061a/Office%20Open%20XML%20Part%203%20-%20Primer.pdf).
A horizontal flip followed by 270-degree rotation gives top-left -> bottom-left
-> bottom-right with a rightward head. Pixel regression tests cover this path.

Even with stCxn/endCxn, use saved geometry and transforms. Rerouting from connection
sites is unsupported and produces a targeted diagnostic. LibreOffice may change
routes depending on those references; distinguish reference behavior from static
specification agreement. Do not change transform order or thresholds to hide it.

`python tests/visual/make_connector_routing.py /tmp/routing.pptx` generates paired
controls differing only in connection references, showing LibreOffice's moved
start point versus our preserved route.
