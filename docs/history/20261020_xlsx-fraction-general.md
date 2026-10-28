---
date: 2026-10-20
kind: decision
principles: [fidelity, transparency]
---

# Fraction and General number formats: decimal arithmetic, width-dependent General, and the unresolved Excel comparison

**Date**: 2026-10-20 (written before the import into rhodopsin; dates come from the notes themselves)
**Status**: Accepted and implemented in the library; not verified against Excel.
**Canonical code**: `crates/xlsx/src/format/fraction.rs` and `format/mod.rs` (built-in ids 12 and 13), `crates/xlsx/src/text_layout.rs` (`measure_general_number`), `crates/xlsx/src/cell_text.rs` (General hook), `crates/xlsx/tests/format.rs` (`組み込み分数の分母制限と桁合わせを保持する`), `crates/xlsx/tests/general_number.rs`, `crates/xlsx/tests/cell_text.rs` (`分数の桁合わせと幅不足をpdf抽出へ接続する`), `crates/xlsx/examples/make-fraction-probe.rs`, `crates/xlsx/examples/make-general-probe.rs`.

## Decision

- Built-in format 12 resolves to `# ?/?` and 13 to `# ??/??`. The stored decimal lexeme is split into integer and fractional parts and the fraction is the last continued-fraction convergent whose denominator does not exceed 9 or 99. Arithmetic is on decimal digits; there is no conversion to binary floating point. Carry into the integer part, negative sections, colors, and a required integer `0` are handled.
- Numerators are right-aligned and denominators left-aligned; the padding for missing `?` digits and the slash of an integer-only row are real fragments measured in the actual font, not fixed-point widths. Fixed denominators, improper fractions, denominators of three or more digits, and combinations with percent are rejected.
- General is resolved only where a width is known. `measure_general_number` generates candidates in decimal and measures them in the real font: fixed notation from 10 down to 0 decimals with trailing zeros stripped, limited to 11 characters excluding the sign; scientific notation with a mantissa, `E`, an exponent sign, and at least two exponent digits, carrying the exponent when rounding makes the mantissa 10. Scientific is tried only when fixed does not fit, or when the exponent is below -3 and fixed notation loses significant digits. The sign is excluded from the character limit but included in the measured width. A non-zero value is never rounded to zero to fit. When neither notation fits, the existing `#` fill and width warning apply; a width that cannot hold one `#` stops rather than truncating.
- The width-less `format::cell_with_context` and `NumberFormat::parse` do not fix a General string; they return `general_number_layout` so that no caller gets a guessed text. `[Red]General`, conditions, and DBNum combinations are out of scope.

## Why

- ECMA-376 §18.8.30 and Microsoft's numFmt documentation describe the 11-character fixed limit and the switch to scientific notation by available width, but not the exact selection order or rounding boundaries. Those remain to be confirmed against an Excel-saved PDF, so they stay outside the default CLI path.
- The fraction rule was compared with the public SheetJS SSF test set (23 cases for the two formats): 21 matched text and alignment spaces, two did not. `0.3` gives `1/3` here and `2/7` upstream; `2.3` gives `2 1/3` and `2 2/7`. Excel's internal rule (possibly binary representation effects) is unconfirmed. The two values are not special-cased and the comparison is not marked passing; a public test set is not a substitute for an Excel reference.
- Decimal arithmetic was chosen because binary round trips would make decimal boundaries indistinguishable (`0.5` versus the nearest double), which the tests exercise.
- Observation workbooks: `make-fraction-probe` writes 16 rows (zero, integers, signed decimals, the two mismatch values, carry boundaries) in columns B/D (format 12) and C/E (format 13) at widths 16 and 4, print area `A1:E16`, A4 portrait; `make-general-probe` writes 18 values (0, ±1, 10-12 digit integers, rounding carries, negative exponents, precision cases, 1E±20) at widths 3, 8, 16, 28, print area `A1:E18`. Both take the output path and the font name, refuse to overwrite, and produce byte-identical output for the same arguments. Their stdout JSON records the input hash and initializes the Excel and LibreOffice references as `unobserved`; generating them is not obtaining a reference. Neither has been opened in Excel yet.
