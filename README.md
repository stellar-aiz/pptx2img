# pptx2img

Convert PPTX slides to PNG with a standalone CLI or Rust library.
No Office, LibreOffice, browser, Python, or Node runtime required.

**Early implementation: rendering is approximate, not fully PowerPoint-compatible.**

## Quick start

Requires Rust and Cargo. Install from this checkout:

```sh
cargo install --path . --locked
pptx2img slides.pptx
pptx2img --help
```

Writes `slides.pptx2img/slide-001.png`, etc., and `manifest.json`.
Use `--out images` to choose a destination; existing directories are never overwritten.

## Fonts and warnings

- Supply missing fonts with `--font-file /path/to/font.ttf` (repeatable).
- Font-family substitution requires `--font-fallback`.
- Individual missing glyphs use available fallback fonts and produce warnings.
  If no font can render a character, conversion fails.
- `--strict` stops on any warning before writing images.

Success returns YAML on stdout; failures return YAML on stderr.
Warnings and rendering diagnostics are retained in the manifest.

[CLI, fonts, and Rust API](docs/usage.md) ·
[Supported features and limitations](docs/rendering.md) ·
[Development and provenance](docs/development.md) ·
[Rendering comparison notes](docs/rendering-notes.md)
