---
kind: guide
---

# Table styles

Embedded styles resolve through the presentation `tableStyles` relationship and `tableStyleId` (`src/pptx/table_styles.rs`). Layers apply in the order wholeTbl, row/column bands, first/last rows and columns, corner cells, and explicit cell formatting wins last. Supported properties: fills, borders, text color, bold, italic, and `fontRef`. Text uses the existing layout path, keeping merged-cell span validation and omission of continuation cells.

Unresolved IDs (including built-in styles absent from the package), style `fillRef`/line references, and unsupported whole-table formatting keep diagnostics; a missing part records `Table style part is missing; using explicit cell formatting`. This is not complete support for every table style.

Implemented independently from ECMA-376 Part 1 DrawingML table styles; no ibis-office code was reused. Generator: `tests/visual/make_table_styles.py`; `tests/table_styles.rs` checks that a styled table and its explicitly formatted equivalent render identically.
