# rhodopsin-xlsx

Library for rendering the printed pages of an `.xlsx` workbook to PDF in pure Rust, without Excel, LibreOffice, or a browser. Part of the [rhodopsin](../../README.md) workspace, imported from the former xlsx2pdf repository.

**Under development: no workbook is converted yet.** Inputs that pass validation stop with `rendering_not_implemented`; the library's development API renders a workbook only when the caller supplies the layout policy. Status, design rules, and open items: [docs/xlsx.md](../../docs/xlsx.md).

This crate has no binary. The root CLI routes `.xlsx` input to it and reports results as its usual YAML documents ([usage](../../docs/usage.md#xlsx-input)):

```sh
rhodopsin book.xlsx --format pdf
cargo test --locked -p rhodopsin-xlsx
```

Diagnostics never include cell contents, formulas, sheet names, paths, or raw library errors. `ConversionOptions` accepts explicit font files and directories, an optional system-font search, and an explicit fallback family for missing fonts; the CLI exposes font files and the system-font switch.

Related documents: [requirements](../../docs/xlsx-requirements.md), [three-way validation](../../docs/xlsx-validation.md), and the dated records under [docs/history](../../docs/history/). Development tools (SBOM/NOTICE generation, distribution bundle, document comparison) are in `tools/`; the first two currently fail in the workspace, as recorded there. This crate is licensed under MIT (see [LICENSE](LICENSE)).
