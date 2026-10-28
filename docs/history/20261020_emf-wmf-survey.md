---
date: 2026-10-20
kind: decision
principles: [lightweightness, transparency]
---

# Decode EMF and WMF pictures independently instead of adopting a metafile crate

**Date**: 2026-10-20 (issue #78, step 1 survey; step 2 implemented the decoder)
**Status**: Accepted and implemented.
**Canonical code**: `src/pptx/metafile.rs` (decoder and limits), `tests/fixtures/metafile/` (nine fixtures from `tests/visual/make_metafile_pictures.py`), [rendering.md](../rendering.md#emf-and-wmf-pictures) (the supported subset).

## Decision

Implement the EMF and placeable-WMF subset from MS-EMF and MS-WMF, emitting SVG paths and running bitmaps through the existing PNG/JPEG size policy. Do not add `emf-core`/`wmf-core` or `emfsdk` as dependencies. Text records stay `unsupported` and are reported, not drawn. `emfsdk`'s strict validator may be used at development time to check that generated fixtures are conformant; it is not a CLI dependency.

## Why

Both crates found on crates.io were weeks old, pre-1.0, single-maintainer, and neither covered the subset the issue required.

- `emf-core` 0.1.1 ignored clipping (`SelectClipPath`, `IntersectClipRect`, `ExcludeClipRect`) and world transforms, drew bitmaps at source size instead of stretched, and rejected a whole file on a conformant 92-byte `LogFontW` text record.
- `emfsdk` 0.2.0 was raster-only (PNG, so the resolution is fixed before the slide scale is known), did not fill `StrokeAndFillPath`, and silently returned `None` for the 88- and 100-byte EMF headers that MS-EMF allows.
- Output form mattered as much as coverage: `emf-core` SVG carried bitmaps as `data:` URIs, which the SVG picture loader rejects by design because they bypass the size policy, and had no intrinsic size.
- Licences (MIT; MIT OR Apache-2.0) were not what separated the candidates.

Accepted weakness: a defect in the decoder is ours to fix, but that would have been true with either crate as well. The nine fixtures isolate record groups (shapes with each header size, paths, transforms, clipping, bitmaps, text, and placeable WMF) and were rendered by LibreOffice as the comparison reference; `ExcludeClipRect` holes follow the specification because LibreOffice does not draw them.
