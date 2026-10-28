---
kind: guide
---

# Static OLE previews and hidden-object diagnostics

OLE payloads are never executed (`src/pptx/objects.rs`). Only internal PNG/JPEG, EMF, and placeable WMF previews enter the image renderer (metafiles through the [EMF and WMF translation](rendering.md#emf-and-wmf-pictures)); external references are not fetched. The `graphicFrame` determines display coordinates; the nested `pic` transform is not reapplied. For `mc:AlternateContent`, exactly one OLE is selected from `Fallback` rather than relying on `Choice` extensions, and hidden status on either the frame or the preview is preserved.

Even a displayed preview records a static-substitution `approximation` naming the format (`Rendered embedded PNG/JPEG preview ...`, `Rendered embedded EMF/WMF preview ...`). Missing previews, parts, or relationships, unsupported formats, and external references record `omitted` with type and reason. Interpretation of OLE binaries is outside scope.

## Hidden OLE keeps its diagnostic

Hidden non-OLE elements are skipped silently (see [hidden elements](rendering.md#hidden-elements)). A hidden OLE frame, or a frame whose preview picture is hidden, is the deliberate exception: it is not drawn and records `omitted` with `OLE (<progId>) is hidden; OLE is never executed`. The diagnostic states that OLE content was never executed, which is useful information in itself (issue #37). Real decks were observed to carry hidden, sub-pixel OLE frames whose `Choice` has no preview and whose `Fallback` carries a WMF; only the selected `Fallback` is diagnosed.

## Fixtures

`tests/visual/make_ole.py` generates two pages comparing project-generated PNG/JPEG previews with regular images; the OLE payload is intentionally invalid data, demonstrating that execution is unnecessary. `tests/visual/audit_ole.py input.pptx` extracts only structural information from a deck. `tests/ole.rs` checks single rendering with translucent pixels, PNG/JPEG, `Fallback` selection, hidden status, and targeted reasons for missing, unsupported, and external previews; `tests/fixtures/ole-preview.jpg` is a project-generated 2x2 JPEG.
