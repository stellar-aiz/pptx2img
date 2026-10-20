# Preset geometry and comparison evidence

Hand-written presets are homePlate, upArrow, rightArrow, rightBrace, and
wedgeRoundRectCallout; further presets are
[evaluated from shape guides](#presets-evaluated-from-shape-guides). Build outlines and text rectangles in local coordinates,
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

## Presets evaluated from shape guides

A second group of presets is evaluated from transcribed DrawingMLGeometries
definitions instead of hand-written outlines: chevron, triangle, downArrow,
leftArrow, leftRightArrow, upDownArrow, diamond, trapezoid, pie, star5, cloud,
round2SameRect, wedgeEllipseCallout, flowChartAlternateProcess,
flowChartMultidocument, and the connectors bentConnector3, bentConnector4,
curvedConnector2, and curvedConnector3. The [next tier](#next-tier-presets) adds 31 more.

Each definition lists the preset's adjustment defaults, guides, text rectangle,
and paths from the ECMA-376 Part 1 DrawingMLGeometries part. An independent
evaluator implements the guide operators of ST_GeomGuideFormula (`val`, `*/`,
`+-`, `+/`, `?:`, `abs`, `at2`, `cat2`, `cos`, `max`, `min`, `mod`, `pin`, `sat2`,
`sin`, `sqrt`, `tan`), the built-in guides (`w`, `h`, `l`, `t`, `r`, `b`, `hc`,
`vc`, `ss`, `ls`, `wd*`, `hd*`, `ssd*`, `cd*`), and the path commands `moveTo`,
`lnTo`, `arcTo`, `cubicBezTo`, `quadBezTo`, and `close`, including per-path
`w`/`h` coordinate spaces and `fill="none"`/`stroke="false"`.

- `arcTo` angles are visual angles on the ellipse (clockwise, y down). They are
  converted to parametric angles, and arcs are emitted in quarter-turn segments.
- The published pie text rectangle reads `l=il t=ir r=it b=ib`; the guide names
  describe the inscribed square, so `il it ir ib` is used. This is a deliberate
  deviation from the literal file.
- Paths without fill (connectors, the cloud's inner arcs, the multidocument's
  outlines) are stroked only. Only those open paths keep line-end markers; closed
  outlines never draw arrowheads.
- Connectors still draw when their width or height is zero. Other presets with a
  zero extent draw nothing, as before.
- Path fill modes (`darken`, `lighten`, …) are not used by these presets and are
  not implemented. Guides that evaluate to non-finite values omit the outline
  with a warning.

Fixture pages 3–6 of `tests/visual/make_preset_shapes.py`: page 3 has the 15 new
presets at their defaults, page 4 the same presets with adjustments, 15° rotation,
and alternating horizontal/vertical flips, page 5 the four connectors as `p:cxnSp`
with tail arrows (defaults, then adjusted), and page 6 the same connector
geometries as ordinary `p:sp` shapes.

Comparison with LibreOffice 26.2.5.2 at 1600px on 2026-10-19 (thresholds unchanged:
MAE 0.02, changed pixels 5%, channel tolerance 16):

| Page | Before (`e103c5d`) | After |
|---|---|---|
| 3 new presets, defaults | MAE 0.284966, changed 40.911% (fail: omitted) | MAE 0.002176, changed 1.115% |
| 4 adjusted, rotated, flipped | MAE 0.286900, changed 41.088% (fail: omitted) | MAE 0.002082, changed 1.016% |
| 5 connectors (`p:cxnSp`) | MAE 0.012443, changed 1.986% (omitted) | MAE 0.021766, changed 3.450% (**fail**) |
| 6 connector geometry (`p:sp`) | MAE 0.012206, changed 1.897% (omitted) | MAE 0.000529, changed 0.358% |

Page 5 is a known difference, kept as reproduction material rather than hidden:
LibreOffice reroutes `p:cxnSp` bent and curved connectors with its own connector
router (for example, it starts bentConnector3 vertically) instead of drawing the
saved preset path, even without connection sites. Page 6 draws the same geometries
as ordinary shapes, where LibreOffice follows the preset path and the pages match.
This renderer always draws the saved preset path. Before this change, page 5/6
"passed" only because the omitted thin lines are a small share of the page.

## Next tier presets

Thirty-one further presets use the same evaluator: rtTriangle, foldedCorner,
borderCallout1, borderCallout2, wedgeRectCallout, cloudCallout, upArrowCallout,
downArrowCallout, rightArrowCallout, flowChartDocument, flowChartMagneticDisk,
flowChartTerminator, flowChartExtract, flowChartConnector, blockArc, donut, arc,
can, snip1Rect, mathPlus, mathMultiply, plus, leftBracket, rightBracket, leftBrace,
bentUpArrow, curvedDownArrow, circularArrow, irregularSeal2, star7, and
parallelogram. They were transcribed independently from the ECMA-376 Part 1
DrawingMLGeometries part and checked against LibreOffice renderings. Presets
unknown to the table (for example heart) still produce `Unsupported preset shape`.

The definition format gained one directive, `approx <note>`, which records an
approximation warning once per shape:

- Lighten/darken path shading (foldedCorner's folded flap, can's top face) is not
  drawn; the shapes use the normal fill. Warning: "lighten and darken path shading
  is not drawn".
- curvedDownArrow draws the darker inner face of the band with the normal fill.
- cloudCallout draws the published cloud body; its two trailing bubbles are placed
  along the tail by a fitted ratio rather than the published guide chain, so their
  size and position are approximate.
- Callouts keep the whole box as the text rectangle; brackets and braces inset it
  by the corner arc; arrow callouts use the box part only.

Fixture pages 7-10 of `tests/visual/make_preset_shapes.py`: pages 7-8 hold the 31
presets at their defaults (18 and 13 shapes), pages 9-10 the same presets with
non-default adjustments, 15 degree rotation, and alternating flips. Pages 1-6 are
unchanged.

Comparison with LibreOffice 26.2.5.2 at 1600px on 2026-10-20 (thresholds unchanged:
MAE 0.02, changed pixels 5%, channel tolerance 16):

| Page | Before (`0f85650`) | After |
|---|---|---|
| 7 next tier, defaults (18) | MAE 0.333579, changed 48.117% (fail: omitted) | MAE 0.003503, changed 1.663% |
| 8 next tier, defaults (13) | MAE 0.188520, changed 27.271% (fail: omitted) | MAE 0.001318, changed 1.164% |
| 9 adjusted, rotated, flipped (18) | MAE 0.335346, changed 48.332% (fail: omitted) | MAE 0.005556, changed 2.562% |
| 10 adjusted, rotated, flipped (13) | MAE 0.183331, changed 26.453% (fail: omitted) | MAE 0.003309, changed 2.262% |

Pages 1-6 keep their recorded results (page 5 remains the known `p:cxnSp` routing
difference above).

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
