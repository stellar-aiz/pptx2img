---
kind: guide
---

# Outer shadows

`outerShdw` color, alpha, direction, distance, and blur become an SVG filter built from `SourceAlpha` and composited behind the shape (`src/pptx/effects.rs`). Blur is a Gaussian with sigma equal to half the radius and records an `approximation` diagnostic. Scaling and skew warn (`Outer shadow scaling and skew are unsupported`); `innerShdw`, glow, and other effects warn `Unsupported shape effect: <name>`; theme `effectRef` is covered by `Theme effect and text style references are unsupported`. `rotWithShape` is honoured.

Pixel tests (`tests/shadows.rs`) cover rotation, flipping, `rotWithShape`, and overlap with later shapes. Implemented independently from the [OuterShadow attributes](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outershadow); no ibis-office code was reused. Generator: `tests/visual/make_shadows.py`. Blur-kernel and rotated-frame differences against LibreOffice remain.
