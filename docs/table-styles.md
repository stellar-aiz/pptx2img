# Table styles

Resolve embedded styles through the presentation tableStyles relationship and
tableStyleId. Combine wholeTbl, row/column bands, first/last rows and columns,
and corner cells in that order; explicit cell formatting wins last. Supported
properties include fills, borders, text color, bold, italic, and fontRef.
Text uses the existing layout path, retaining merged-cell span validation and
omission of continuation cells.

Unresolved IDs (including built-in styles absent from the package), style fillRef/
line references, and unsupported whole-table formatting retain diagnostics.
This is not complete support for every table style.

Implemented independently from ECMA-376 Part 1 DrawingML table styles.
No ibis-office code, tests, or assets were reused.

Generate the independent fixture with:
`python tests/visual/make_table_styles.py /tmp/tables.pptx`.
Styled and equivalent explicitly formatted PNGs matched byte-for-byte. The fixture
includes a merged header, alternating bands, explicit end-cell color, borders,
and text. Historical comparison against LibreOffice 26.2.5.2 gave MAE 0.012951
and changed pixels 3.133% on both pages, within unchanged thresholds. Text positions
account for most remaining differences.
