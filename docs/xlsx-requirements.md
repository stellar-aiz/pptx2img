---
kind: guide
---

# XLSX to PDF: requirements

This page holds the caller's requirements for the spreadsheet renderer. It is not a description of what is implemented; [xlsx.md](xlsx.md) tracks that, including where the current CLI still deviates, and completion needs implementation and verification evidence for every item.

## Purpose and integration

- Turn a workbook with saved print settings (estimates, forms) into a PDF with the same pages as Excel itself.
- Stop rather than approximate when any element cannot be reproduced.
- The caller invokes the CLI once; print-setting checks and PDF validation stay with the caller.
- Run inside macOS Seatbelt, Windows AppContainer, and Linux bwrap.
- No Office, LibreOffice, browser, Python, or Node at runtime.

## Input, output, and diagnostics

- One `.xlsx` input. `.xls`, `.xlsm`, and encrypted input are rejected with a reason.
- One PDF output. Existing files are never overwritten, and no partial PDF remains after a failure.
- Success on stdout, failure on stderr, one YAML document each in the rhodopsin CLI result format. Success includes page count, page sizes, sheet order and ranges, fonts and embedding state, and warnings.
- Failure returns a category, a reason, and a location (one-based sheet order, cell reference). When a cell cannot be identified, the `cell` key is omitted; when no location is known, `location` is omitted.
- Diagnostics never contain cell values, strings, or sheet names: position, count, and kind only.
- Exit codes follow the rhodopsin CLI: 0 success, 1 conversion failure, 2 invalid arguments. The failure category (`INVALID_INPUT`, `UNSUPPORTED_FEATURE`, `FONT_NOT_FOUND`, `LAYOUT_FAILED`, `INTERNAL_ERROR`) is the result document's `code`.

## Runtime and resources

- A single binary for macOS arm64/x86_64, Windows x86_64, and Linux x86_64.
- No network access and no child processes.
- Read only the input and the font locations; write only the output location.
- Temporary files, if needed, live in the output directory and are removed on exit.
- Limits on memory, ZIP expansion, sheet and cell counts, and image expansion.
- The same input with the same fonts gives the same PDF; dates and IDs are fixed.

## Values and number formats

- Formulas are not recalculated; the stored result is used, and a missing result stops the conversion.
- General, thousands separators, decimals, currency, accounting, and percent follow Excel.
- Positive/negative/zero/text sections, color codes, dates and times, Japanese eras, DBNum1-3, and built-in `numFmtId` values.
- A numeric cell too narrow for its text shows `###` as Excel does, with a warning carrying position and count.
- Text overflows into or is cut at neighbouring cells by Excel's rules.

## Page content

- Cell values; font family, size, bold, italic, underline, color.
- Solid fills; border style, width, color; horizontal and vertical alignment, wrapping, indent; merged cells.
- Saved column widths and row heights; column width conversion uses the real font's digit width.
- Print area, paper size, orientation, scale, fit to n by m pages, margins, horizontal and vertical centering, manual page breaks, repeated rows and columns.
- Hidden rows, columns, and sheets are not printed.
- PNG/JPEG images anchored to cells; theme colors and theme fonts.

## Rejected in the first version

Shapes, text boxes, charts, conditional formatting, table styles, gradients, vertical and rotated text, shrink to fit, headers and footers, printed comments, and sheets with differing print orientation. Each is reported with its kind and position. Scope grows one element at a time; approximation is not used to continue.

## Fonts and PDF

- No Microsoft or Apple fonts are bundled.
- Fonts come from the OS font directories (searched by default, disabled with `--no-system-fonts`) and from `--font-file`. The search locations and the sandbox's read permissions must agree.
- A missing font stops the conversion by default. Substitution is allowed only by explicit option, with a warning.
- A single undrawable character stops the conversion.
- Text is not converted to paths: the used glyphs are embedded as a subset with ToUnicode, so the caller can check every cell's text against the PDF.

## Correctness and comparison

- The oracle is Excel's own "Save as PDF". LibreOffice is for comparison only.
- From the same non-confidential `.xlsx`, compare the Excel PDF, the rhodopsin PDF, and the LibreOffice PDF. Keep the input and reference PDFs; record application versions, OS, fonts, and the procedure.
- Compare page count and size, page-break positions (first and last cell), all strings, text positions, and rasterized image differences.
- Tolerances are fixed with a stated reason and never relaxed to hide a difference.
- Known differences come with a reproducing fixture. Agreement with LibreOffice alone does not pass.
- Development-time comparison tools are not runtime dependencies of the shipped binary.
- Verdicts and records follow [xlsx-validation.md](xlsx-validation.md).

## Licensing

Dependencies are limited to permissive licences; no copyleft is added. An SBOM and NOTICE covering transitive dependencies can be generated.
