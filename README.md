# rhodopsin

Convert PPTX slides to PNG or searchable PDF with a standalone CLI or Rust library.

**😎 No Office, LibreOffice, browser, Python, or Node runtime required.**

Early implementation: rendering is approximate, not fully PowerPoint-compatible.

## Quick start

```
rhodopsin <INPUT> [--format png|pdf] [--out PATH] [--width N] [...]
```

Requires Rust and Cargo. Install from this checkout:

```sh
cargo install --path . --locked
rhodopsin slides.pptx
rhodopsin --help
```

Writes `slides.rhodopsin/slide-001.png`, etc., and `manifest.json`.
Use `--out images` to choose a destination; existing directories are never overwritten.

For one PDF per deck, use `rhodopsin slides.pptx --format pdf --out slides.pdf`.
PDF output preserves text and supported vector shapes using the existing rendering
dependencies. Its initial [supported subset](docs/rendering.md#pdf-output) is narrower
than PNG; unsupported PDF features fail explicitly. Existing files are never replaced.

## XLSX to PDF (under development)

The workspace also contains `rhodopsin-xlsx`, a separate crate and CLI that renders
the printed pages of an `.xlsx` workbook to PDF. It was imported from the former
xlsx2pdf project and is not connected end to end yet: the CLI validates the input
and loads fonts, then stops with `rendering_not_implemented`.
See [crates/xlsx/README.md](crates/xlsx/README.md) and [docs/xlsx](docs/xlsx/).

## Fonts and warnings

- Supply missing fonts with `--font-file /path/to/font.ttf` (repeatable).
- Font-family substitution requires `--font-fallback`.
- Individual missing glyphs use available fallback fonts and produce warnings.
  If no font can render a character, conversion fails.
- `--strict` stops on any warning before writing output.

Success returns YAML on stdout; failures return YAML on stderr.
Warnings and rendering diagnostics are retained in the PNG manifest or PDF result
document on stdout.

[CLI, fonts, and Rust API](docs/usage.md) ·
[Supported features and limitations](docs/rendering.md) ·
[Development and provenance](docs/development.md) ·
[Rendering comparison notes](docs/rendering-notes.md)
