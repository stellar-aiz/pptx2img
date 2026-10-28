---
kind: guide
---

# XLSX to PDF: design rules and readiness

`crates/xlsx` holds `rhodopsin-xlsx`, the library that will render the printed pages of an `.xlsx` workbook to PDF. The root CLI routes `.xlsx` input to it (`rhodopsin book.xlsx --format pdf`, see [usage](usage.md#xlsx-input)); the crate has no binary of its own. The requirements it works toward are in [xlsx-requirements.md](xlsx-requirements.md), and the comparison method in [xlsx-validation.md](xlsx-validation.md).

**Status: no workbook is converted yet.** `convert_with_options` validates the package, workbook, styles, and fonts, then stops with `UNSUPPORTED_FEATURE` / `rendering_not_implemented` (`crates/xlsx/src/lib.rs`). Inputs that fail earlier report their own reason code. XLSX input therefore always behaves as `--strict`. The library's development API (`convert_with_policy`, `PreparedWorkbook::render_with_policy`, `render_with_cell_overflow`, `render_with_borders` in `crates/xlsx/src/render.rs`) renders a workbook when the caller supplies the layout policy that Excel decides for itself (content rectangles, line pitch, clip rules); that policy is what remains to be derived and verified.

## Design rules

These are the decisions that cannot be read from the code. Each is enforced by a reason code and fixed by tests under `crates/xlsx/tests/` (test names are Japanese).

- **Stop, never approximate silently.** Anything the renderer cannot reproduce stops with a located reason instead of a guess: a missing glyph is not patched from another font, a locale is not inferred, a font family is not inferred from a file name. Per-element approximate rendering with warnings is planned but not implemented.
- **Diagnostics carry position, count, and kind only.** No cell values, strings, formulas, sheet names, paths, or raw library errors reach a diagnostic (`crates/xlsx/src/diagnostic.rs`; the CLI adds a silent panic hook for the same reason, `src/cli_xlsx.rs`).
- **Formulas are never recalculated.** The stored cached value and its decimal lexeme are used; a missing cache stops with `formula_cached_value_missing`.
- **Decimal arithmetic on stored lexemes.** Dates, times, exponents, fractions, and General candidates are computed from the decimal text, never through an `f64` round trip (`format/`, `text_layout.rs`).
- **Number format resolution.** An explicit `formatCode` beats `numFmtId`. Built-in date/time ids 14-22 and 45-47 resolve only when the context declares en-US (14, 22, 47 follow the MS-OI29500 Excel differences); the Japanese ids 27-36 and 50-58 resolve only under a declared ja locale; otherwise the id is rejected rather than guessed. Ids 5-8, 23-26, 41-44, and 63-66 require the stored `formatCode`, else `locale_number_format_missing`.
- **General is width dependent.** `text_layout::measure_general_number` chooses between fixed notation (10 down to 0 decimals, trailing zeros stripped, 11 characters excluding the sign) and scientific notation by measuring candidates in the real font; the width-less `format` API returns `general_number_layout` instead of fixing a string. A non-zero value is never rounded to zero; when nothing fits, the `#` fill and width warning apply. See [the fraction and General record](history/20261020_xlsx-fraction-general.md).
- **DBNum** converts only non-negative integer digits under a declared ja locale; General's 万/億 units are never substituted by digit conversion. `parse_with_locale` passes the caller's locale; an in-section `[$-409]` is not treated as Japanese.
- **Indent** is 3 spaces of the Normal font, found through `cellStyles builtinId=0`, scaled by the print scale.
- **Underline** uses the `post` table top and thickness; the top is not the line centre. Only single underline is supported; double and accounting underlines stop (`underlined_cell_text`, `formatted_text_underline`) until Excel is measured ([record](history/20261020_xlsx-underline.md)).
- **Bidi** text is rejected with `text_direction` when any R, AL, AN, or directional control character appears, so no run is drawn in a wrong single direction.
- **Multi-glyph clusters** are accepted only when each source character maps to one glyph in order; substitution or reordering stops with `pdf_multi_glyph_cluster` (`pdf/text_mapping.rs`).
- **Shared-edge borders** that disagree in style or color stop with `border_conflict`, and partial overlaps with `border_partial_overlap`; the thicker or later cell is never picked because Excel's precedence is unverified (`page_borders.rs`). A merged range whose outer edge disagrees stops with `merged_border_style_conflict`.
- **OS font probing** reads only sfnt directories and name tables (64 MiB total, 1 MiB per table) and loads the bodies of files that contain a requested family, through the same handle, so a path swapped after indexing still reads the original file (`fonts.rs`, `system_fonts.rs`). Relative `HOME`/data directories stop with `invalid_system_font_directory`.
- **Limits are best effort**, applied per layer (package, fonts, layout, scene, PDF). They are not a strict process peak-memory guarantee.
- **Output is never overwritten**; the PDF is written to a temporary file in the output directory and published exclusively (`output.rs`).
- **Nothing external at runtime**: no network, no child process, no Excel, LibreOffice, or Python.

## Evidence

Fixtures under `crates/xlsx/tests/fixtures/` and the workbooks built inside tests are synthetic; `tests/support/mod.rs` generates minimal sfnt fonts with empty outlines. These prove control flow (selection, measurement, rejection), not appearance. No Excel-generated reference workbook or PDF exists yet, so no claim of page-level agreement with Excel has been verified, and none should be stated. The probe generators `cargo run -p rhodopsin-xlsx --example make-fraction-probe|make-general-probe|make-underline-probe -- OUT.xlsx 'Font Name'` write deterministic observation workbooks for the three open measurement questions; they are inputs for an Excel observation, not fixtures.

## Not finished

| Requirement | Current evidence | Remaining |
|---|---|---|
| One CLI call produces a PDF and a success document | Every workbook stops with `rendering_not_implemented` | Connect verified default layout, PDF output, and the success report into one path |
| Same page as Excel | No Excel reference PDF | Obtain references under recorded conditions; compare per [xlsx-validation.md](xlsx-validation.md) |
| General number format | Width-dependent selection connected to cell layout | Verify selection, rounding, and exponent boundaries against Excel |
| DBNum1-3 | Non-negative integer digits with declared ja locale | Units, decimals, signs, dates |
| Borders | Candidates, shared-edge coalescing, merged outer edges, explicit-policy drawing (`render_with_borders`); the default path still rejects used borders | Line widths, dashes, double lines, junctions, precedence |
| Underline | Single underline only | Double and accounting underline geometry after measurement |
| Wrapping and alignment | `render_with_policy` takes content rectangles and line pitch from the caller | Excel's padding, line pitch, and break rules as the default |
| Text overflow | `render_with_cell_overflow` clips by occupancy | Verify Excel's boundary rules |
| Merged cells | Top-left cell content and fill | Merges across page breaks, split text, edge rules |
| Text shaping and extraction | TrueType/CFF subsetting with ToUnicode; one-to-one clusters | Substituting clusters, script splitting |
| Target OS and sandbox | CI runs natively on ubuntu, macOS, and Windows; one macOS Seatbelt denied-read test (`tests/xlsx_cli.rs`) | Strict read permissions; AppContainer and bwrap are not exercised anywhere in this repository |
| Three-way comparison | `compare-pages`, `compare-rasters`, `check-reference`, and `tools/compare-document.py` exist with synthetic tests | A real Excel reference; cell-level, position, amount, and embedding checks |
| Dependency licences, SBOM, NOTICE, distribution bundle | `tools/dependency-artifacts.py` and `tools/distribution-bundle.py` were written for the standalone repository and fail in the workspace (the root crate has no `license` field; `resolve.root` is `rhodopsin-xlsx`) | Rewire the tools once the root LICENSE is decided; see the [dependencies](history/20261020_xlsx-dependencies.md) and [distribution](history/20261020_xlsx-distribution.md) records |

The CLI also differs from the requirements in two places that are tracked as issues rather than hidden: `.xls` and `.xlsm` input falls through to the PPTX pipeline instead of a reasoned rejection (the library itself rejects them with `input_extension_not_xlsx` / `encrypted_or_legacy_workbook`), and font substitution (`fallback_family`) and font directories exist in `ConversionOptions` but are not exposed by the CLI.

## Working on the crate

```sh
cargo test --locked -p rhodopsin-xlsx
cargo test --locked --test xlsx_cli
cargo clippy --workspace --locked --all-targets -- -D warnings
python3 -m unittest discover -s crates/xlsx/tools -p 'test_*.py'
```

`cargo run --example` and `cargo test --example` need `-p rhodopsin-xlsx`; the workspace `target/` is at the repository root. Primary sources consulted for OOXML attributes (PageSetup, ST_Xstring, CellFormat, NumberingFormat, the MS-OI29500 built-in format differences, Excel date systems, Japanese era boundaries, OpenType `name` and `post`, the PDF ToUnicode specification) are linked from the code where they were applied; none of them substitutes for a comparison with Excel output. Convergence with the PPTX side (fonts, then the PDF writer) is recorded in [the import decision](history/20261022_xlsx-import.md).
