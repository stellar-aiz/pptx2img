---
date: 2026-10-21
kind: decision
principles: [fidelity, transparency]
---

# Underlines: single only, measure Excel before adopting double and accounting geometry

**Date**: 2026-10-21
**Status**: Accepted. Single underline implemented; double and accounting underline rejected pending measurement.
**Canonical code**: `crates/xlsx/src/styles.rs` (parses all five underline kinds), `crates/xlsx/src/cell_text.rs` and `cell_text/rich.rs` (reject anything but none or single with `underlined_cell_text` / `formatted_text_underline`), `crates/xlsx/src/fonts.rs` (`underline_metrics` from the `post` table), `crates/xlsx/tests/cell_text.rs`, `crates/xlsx/examples/make-underline-probe.rs`.

## Decision

- A single underline is drawn as a rectangle in the text color, using the `post` table's underline position as the top edge (not the centre) and its thickness, scaled by the print scale and clipped to the cell. Fonts without underline metrics or with a non-positive thickness are rejected. The underline counts toward the page and workbook drawing limits.
- Double underline, single and double accounting underline, and underlines under number formats that contain fill or non-printing padding characters are rejected, not approximated.
- No spacing constant for double or accounting underlines enters the product before the same 40 conditions have been measured in an Excel PDF.

## Why

- The MS-XLS underline enumeration and the Open XML `UnderlineValues` justify the kinds but say nothing about line spacing, the accounting underline's horizontal extent (glyph advance, cell width, or reserved padding), or page coordinates. A preview from any other tool cannot stand in for the Excel observation because it does not render accounting underlines the same way.
- The observation input is fixed so that the eventual measurement is reproducible: `make-underline-probe OUT.xlsx 'Font Name'` writes one sheet with columns B-F as none / single / double / single accounting / double accounting and eight rows (text at 11 pt and 20 pt bottom-aligned, number format 4, an accounting format with currency symbol and padding in positive, negative, and zero sections, an explicit line break with wrapping at row height 60 pt, and a mixed-size two-color rich text run), A4 landscape, 100%, 0.5 inch margins, column width 16, print area `A1:F9`. It refuses to overwrite, is byte-deterministic, and its stdout JSON records `cases: 40` with `font_actual: null` and both references `unobserved`.
- What to measure: baseline distance and thickness of the single line, coordinates and gap of the double line, what the accounting underline's ends follow, how negative, zero, line breaks, and mixed sizes move or split the line, plus the PDF's strings, fonts, page size, and count. The 40 cases do not cover merged cells, rotation, auto row height, scaling, or other fonts.
