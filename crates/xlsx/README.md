# rhodopsin-xlsx

Library and CLI for rendering the printed pages of an `.xlsx` workbook to PDF in
pure Rust, without Excel, LibreOffice, or a browser. Imported into the rhodopsin
workspace from the former xlsx2pdf repository (commit fa55270, 2026-10-22).

**Under development. The CLI does not produce PDFs yet.** The library has a
development API that renders a workbook when the caller supplies the layout
policy (cell padding, line spacing, and so on), but the Excel-matching default
layout is not finished. Inputs that pass validation currently exit with
`rendering_not_implemented`.

```sh
cargo run -p rhodopsin-xlsx -- input.xlsx --out output.pdf
cargo run -p rhodopsin-xlsx -- input.xlsx --out output.pdf --font-file /path/to/font.ttf
cargo test -p rhodopsin-xlsx
```

Successful conversions print one machine-readable JSON report on stdout; failures
print one JSON diagnostic on stderr with exit codes 2 to 6 by category.
Diagnostics never include cell contents, formulas, sheet names, paths, or raw
library errors.

`--font-file` and `--font-dir` are repeatable and are the only font sources by
default. `--system-fonts` adds the OS font directories. `--fallback-font NAME`
allows a substitute family for missing fonts only; the substitute must also be
supplied through `--font-file` or `--font-dir`.

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
