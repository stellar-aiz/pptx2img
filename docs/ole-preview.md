# Static OLE previews and hidden-object diagnostics

OLE payloads are never executed. Only internal PNG/JPEG images enter the existing
image renderer; external references are not fetched. The graphicFrame determines
display coordinates, without reapplying the nested pic transform.
For mc:AlternateContent, select exactly one OLE from Fallback rather than relying
on Choice extensions. Preserve hidden status on either the frame or preview.

Even a displayed internal image produces a static-substitution approximation
diagnostic. Missing previews, parts/relationships, unsupported formats, and external
references produce omission diagnostics with type and reason. WMF/EMF rendering
and interpretation of OLE binaries are outside scope.

## Hidden OLE keeps its diagnostic

Hidden non-OLE elements (`sp`, `pic`, `cxnSp`, `grpSp`, other `graphicFrame`s) are
skipped silently (see [rendering](rendering.md#hidden-elements)). A hidden OLE frame,
or a frame whose preview picture is hidden, is the deliberate exception: it is not
drawn and emits `omitted` with `OLE (<progId>) is hidden; OLE is never executed`.
Decision recorded in Issue #37: the diagnostic states that OLE content was never
executed, which is useful information, and it keeps the earlier behaviour and
`tests/ole.rs` unchanged.

## Local source investigation

The source SHA-256 is recorded in Issue #12. Its assets, text, and images are not
published. `python tests/visual/audit_ole.py input.pptx` extracts only structural
information. Pillow identified formats from the related image bytes.

| Page | Type | Choice preview | Fallback preview | hidden | Same image used by another shape |
|---|---|---|---|---|---|
| 1 | TCLayout.ActiveDocument.1 | None | WMF / rId4 | 1 | No |
| 3 | TCLayout.ActiveDocument.1 | None | WMF / rId7 | 1 | No |
| 9 | TCLayout.ActiveDocument.1 | None | WMF / rId7 | 1 | No |
| 10 | TCLayout.ActiveDocument.1 | None | WMF / rId4 | 1 | No |
| 11 | TCLayout.ActiveDocument.1 | None | WMF / rId7 | 1 | No |
| 12 | TCLayout.ActiveDocument.1 | None | WMF / rId4 | 1 | No |
| 18 | TCLayout.ActiveDocument.1 | None | WMF / rId7 | 1 | No |

In every case Choice/Fallback share the OLE relationship and are alternate
representations, not two visible shapes. Only the selected Fallback is diagnosed.
All have position (1588,1588) and size (1227,1588) EMU: at 1600px output, about
(0.208,0.208) position and (0.161,0.208) size in pixels. Besides being hidden,
their frames are smaller than one pixel.

The top-left 40x40px of pages 3/9/11/18 matched white before/after and in the
LibreOffice reference, with zero differing pixels. This demonstrates preserving
hidden status and improving diagnostics, not recovering visible OLE content.
Diagnostics identify frame ID 6 and Fallback hidden status.

An interim "no preview" finding inspected Choice only and was corrected after
including Fallback. An interim "PNG" report was corrected to WMF after byte
inspection. The table above is the final finding.

## Independent fixtures

`tests/visual/make_ole.py` generates two pages comparing independently created
PNG/JPEG previews with regular images. The OLE payload is intentionally invalid
project-generated data, demonstrating that execution is unnecessary.
Historical LibreOffice comparisons: MAE 0.001201/0.001731 and changed pixels
0.494%/0.461%, within unchanged thresholds. Coordinates, outlines, and left/right
agreement were inspected visually.

Rust tests check single rendering with translucent pixels, PNG/JPEG, Fallback
selection, hidden status, and targeted reasons for missing/unsupported/external
previews. `tests/fixtures/ole-preview.jpg` is a project-generated 2x2 red JPEG
created with Pillow.
