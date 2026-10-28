# rhodopsin-xlsx

Library for rendering the printed pages of an `.xlsx` workbook to PDF in
pure Rust, without Excel, LibreOffice, or a browser. Imported into the rhodopsin
workspace from the former xlsx2pdf repository (commit fa55270, 2026-10-22).

**Under development. The CLI does not produce PDFs yet.** The library has a
development API that renders a workbook when the caller supplies the layout
policy (cell padding, line spacing, and so on), but the Excel-matching default
layout is not finished. Inputs that pass validation currently exit with
`rendering_not_implemented`.

This crate has no binary of its own; the root CLI routes `.xlsx` input to it:

```sh
rhodopsin book.xlsx --format pdf
cargo test -p rhodopsin-xlsx
```

Results follow the rhodopsin YAML result documents (see the XLSX input section of
`docs/usage.md`). Diagnostics never include cell contents, formulas, sheet names,
paths, or raw library errors.

`ConversionOptions` accepts explicit font files and directories, an optional
system-font search, and an explicit fallback family that applies to missing
fonts only. The CLI currently exposes font files and the system-font switch.

Design notes (Japanese; paths in them are relative to this directory):

- [Requirements](../../docs/xlsx/requirements.md)
- [Implementation status and verification plan](../../docs/xlsx/development.md)
- [Readiness against the completion criteria](../../docs/xlsx/readiness.md)
- [Three-way validation against Excel and LibreOffice](../../docs/xlsx/validation.md)
- [Dependency licenses and SBOM](../../docs/xlsx/dependencies.md)
- [Development distribution bundles](../../docs/xlsx/distribution.md)

Direct dependencies use MIT, Apache-2.0, or similarly permissive licenses; the
SBOM and NOTICE tooling is in `tools/`. This crate is licensed under MIT (see
[LICENSE](LICENSE)).
