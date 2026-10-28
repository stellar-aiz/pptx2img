---
kind: guide
---

# Preset geometry, lines, and connectors

Preset outlines and text rectangles are built in local coordinates, then the existing rotation, flip, and group transforms apply (`src/pptx/geometry.rs`, `src/pptx/geometry/guides.rs`, `src/pptx/lines.rs`). Formulas and paths were transcribed independently from the DrawingMLGeometries part of [ECMA-376 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/); no ibis-office code, tests, or assets were incorporated. Fixture generator: `tests/visual/make_preset_shapes.py` (ten pages: hand-written presets, guide presets at defaults and adjusted/rotated/flipped, connectors as `p:cxnSp` and as `p:sp`).

## Presets

Three groups share one definition table (50 entries) and one guide evaluator:

- Hand-written outlines: homePlate, upArrow (coordinate exchange from rightArrow), rightArrow, rightBrace, wedgeRoundRectCallout.
- Evaluated from transcribed definitions: chevron, triangle, downArrow, leftArrow, leftRightArrow, upDownArrow, diamond, trapezoid, pie, star5, cloud, round2SameRect, wedgeEllipseCallout, flowChartAlternateProcess, flowChartMultidocument, and the connectors bentConnector3, bentConnector4, curvedConnector2, curvedConnector3.
- Also evaluated: rtTriangle, foldedCorner, borderCallout1, borderCallout2, wedgeRectCallout, cloudCallout, upArrowCallout, downArrowCallout, rightArrowCallout, flowChartDocument, flowChartMagneticDisk, flowChartTerminator, flowChartExtract, flowChartConnector, blockArc, donut, arc, can, snip1Rect, mathPlus, mathMultiply, plus, leftBracket, rightBracket, leftBrace, bentUpArrow, curvedDownArrow, circularArrow, irregularSeal2, star7, parallelogram.

Presets outside the table (for example heart) produce `Unsupported preset shape` and are omitted. Numeric `avLst` adjustments are supported; unknown formulas warn and use defaults.

The evaluator implements the ST_GeomGuideFormula operators (`val`, `*/`, `+-`, `+/`, `?:`, `abs`, `at2`, `cat2`, `cos`, `max`, `min`, `mod`, `pin`, `sat2`, `sin`, `sqrt`, `tan`), the built-in guides (`w`, `h`, `l`, `t`, `r`, `b`, `hc`, `vc`, `ss`, `ls`, `wd*`, `hd*`, `ssd*`, `cd*`), and the path commands `moveTo`, `lnTo`, `arcTo`, `cubicBezTo`, `quadBezTo`, `close`, with per-path `w`/`h` spaces and `fill="none"`/`stroke="false"`.

Rules that are deliberate rather than transcribed:

- `arcTo` angles are visual angles on the ellipse (clockwise, y down), converted to parametric angles and emitted in quarter-turn segments.
- The published pie text rectangle reads `l=il t=ir r=it b=ib`; the guide names describe the inscribed square, so `il it ir ib` is used.
- Unfilled paths (connectors, the cloud's inner arcs, multidocument outlines) are stroked only, and only those open paths keep line-end markers; closed outlines never draw arrowheads.
- Connectors still draw when their width or height is zero; other presets with a zero extent draw nothing.
- Guides that evaluate to non-finite values omit the outline with a warning.
- The `approx <note>` directive records one approximation warning per shape: lighten/darken path shading (foldedCorner's flap, can's top face, curvedDownArrow's inner face) is drawn with the normal fill; cloudCallout's trailing bubbles are placed along the tail by a fitted ratio rather than the published guide chain.
- Callouts keep the whole box as the text rectangle; brackets and braces inset it by the corner arc; arrow callouts use the box part only.

## Text rectangles of rect, roundRect, and ellipse

These three are drawn as SVG primitives, but their text is laid out in the ECMA-376 text rectangle, not the whole box, so text that does not fit records `text_overflow` instead of spilling outside a circle.

| Preset | Text rectangle |
|---|---|
| `rect` | The whole box |
| `roundRect` | Inset by `0.29289 * r` on every side, where `r = min(w, h) * adj / 100000` and `adj` is pinned to 0-50000 |
| `ellipse` | The inscribed rectangle: inset by `w * (1 - cos 45°) / 2` horizontally and `h * (1 - cos 45°) / 2` vertically |

The `roundRect` adjustment diagnostic ("Rounded rectangle adjustment formulas are unsupported") is unchanged. Generator: `tests/visual/make_ellipse_text.py`.

## Lines and connectors

straightConnector1, bentConnector2, the preset dash patterns, headEnd/tailEnd shapes, cap, and join are supported; `custDash`, compound lines, and other unsupported features warn. Line width sets dash and head sizes. Head reference width is `max(line width, 2pt)` and the shaft is not thickened: 27 width/size combinations measured against LibreOffice showed a constant head size through 2 pt and proportional size above it. Heads overlap shafts, so small arrows and stealth joins keep known differences, and approximation diagnostics remain. Generators: `tests/visual/make_lines.py`, `tests/visual/make_arrow_sizes.py`.

## Theme line references

A one-based `lnRef idx` resolves against `lnStyleLst` with `phClr` replaced by the reference color. Theme lines apply first, then inherited lines, then the nearest direct properties, each of width, cap, compound, fill, dash, join, and the two ends resolved separately. Direct `noFill`, an end with `type="none"`, and `prstDash="solid"` cancel inherited values; partial end definitions keep unspecified attributes. `idx=0` means no line, with direct properties taking precedence. Unresolved indices and unsupported features warn. Generator: `tests/visual/make_theme_lines.py` (theme styles, equivalent direct styles, and partial overrides at 0.25 pt; regression tests check pixel identity between the first two). Font substitution residuals on these fixtures are tracked in [issue #25](https://github.com/stellar-aiz/rhodopsin/issues/25).

References: [LineReference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.linereference?view=openxml-3.0.1), [LineStyleList](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.linestylelist?view=openxml-3.0.1), [Outline](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outline?view=openxml-3.0.1).

## Saved bent routes and connection sites

bentConnector2 is top-left -> top-right -> bottom-right; flips apply before rotation ([OOXML Primer 5.7.3 and 5.7.6](https://download.microsoft.com/download/e/1/4/e14fb96f-83b8-4a2a-84db-7fa8acbe061a/Office%20Open%20XML%20Part%203%20-%20Primer.pdf)), so a horizontal flip followed by 270-degree rotation gives top-left -> bottom-left -> bottom-right with a rightward head. Pixel regression tests cover this path.

Static rendering uses the saved geometry and transforms even when `stCxn`/`endCxn` references are present. One `approximation` diagnostic per rendered slide states that live rerouting is not evaluated and retains the first connector target; the reference alone no longer produces a per-connector `unsupported`.

Why this is the policy: a paired fixture (`tests/visual/make_connector_routing.py`) was opened in PowerPoint for Mac 16.105.3, the target of the attached connector was moved, and the copy was saved. PowerPoint stored the updated route in the connector's own `a:off`/`a:ext`; re-rendering the saved copy reproduced the moved route (issue #74). The authoring application therefore keeps saved geometry current, and a static renderer can trust it. This verifies the route for bentConnector2 in that fixture, not every preset or files with stale saved geometry.

LibreOffice reroutes `p:cxnSp` bent and curved connectors with its own router (for example starting bentConnector3 vertically) even without connection sites, while it follows the preset path for the same geometry as `p:sp`. The `p:cxnSp` page of the preset fixture and slide 2 of the routing fixture therefore fail the default thresholds against LibreOffice and are kept as reproduction material. Do not change transform order or thresholds to hide this.
