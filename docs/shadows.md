# Outer shadows

Convert outerShdw color, alpha, direction, distance, and blur to SVG filters.
Build the shadow from SourceAlpha and composite it behind the original shape.
Pixel tests cover rotation, flipping, rotWithShape, and overlap with later shapes.
Blur uses a Gaussian with sigma equal to half the radius and emits an approximation
diagnostic. Scaling, skew, innerShdw/glow, and theme effectRef remain unsupported
and produce warnings.

Implemented independently from [OOXML OuterShadow attributes](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outershadow)
and comparisons using this project's fixtures. No ibis-office code, tests, or
assets were reused.

Fixture: `python tests/visual/make_shadows.py /tmp/shadows.pptx`.
Page 2 includes shadow parameters observed on source page 10: blurRad=50800,
dist=38100, dir=2700000, black with alpha=40000, algn=tl, rotWithShape=0.
No source shapes, text, or assets were reused.

Historical final comparison with LibreOffice 26.2.5.2 at 1600px:

| Page | MAE | Changed pixels |
|---|---:|---:|
| Directions | 0.001331 | 0.682% |
| Frames | 0.002667 | 1.070% |

Thresholds were unchanged. Local frame inspection confirmed shadow position and
darkness moved closer to the reference. Blur-kernel and rotated-frame differences
remain.
