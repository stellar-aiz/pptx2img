# Development and provenance

## Architecture

```text
PPTX (ZIP/XML)
  -> OOXML parsing, slide order, and transforms (src/pptx.rs)
  -> internal SVG
  -> resvg / usvg / tiny-skia
  -> PNG + manifest (src/lib.rs)
  ^ CLI argument parsing and messages (src/main.rs)
```

[ibis-office](https://github.com/ibisjp/ibis-office) inspired the idea of an
Office-independent renderer. Its public README and crate entry points were
consulted, but its code, tests, and assets were not copied, ported, or translated
into this project. OOXML interpretation and layout were implemented independently
using specifications, public APIs, independently generated fixtures, and rendering
comparisons with LibreOffice.

resvg, usvg, tiny-skia, and skrifa are third-party rendering and font dependencies.
Independent implementation does not mean avoiding third-party libraries. Their
licenses and attribution requirements apply. The project's public license has not
yet been selected; choose one before release.

Follow [AGENTS.md](../AGENTS.md) for language and implementation policies.
Keep private inputs and experimental output in ignored `playground/`.

## Tests

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Tests use ZIP/XML inputs, PNG pixels, and generated fonts to check slide order,
images, output protection, malformed archives, strict-mode behavior, inheritance,
font selection, layout, and diagnostics. `examples/basic.pptx` is synthetic.
Generated rectangle fonts are structural test fixtures, not evidence of Japanese
glyph fidelity. No Office or Python runtime is needed for ordinary Rust tests.

CI runs Rust checks and Python comparator tests on Windows, Linux, and macOS,
once per change: on pull requests and on pushes to `main`, skipping Markdown-only
changes. A new push to a pull-request branch cancels the superseded run; runs on
`main` are never cancelled. Cargo and pip dependencies are cached between runs.
Scheduled/manual visual CI uses LibreOffice 26.2.5.2 with a verified download
SHA-256 and checks Hiragino Sans W3/W6 hashes before comparison. Host OS images are
not fully pinned; environment information is retained in reports. Do not compensate
for environment differences by relaxing thresholds.

## Visual comparisons

LibreOffice is a development reference only, never a CLI runtime dependency.
Comparison tests are ignored by default and fail if explicitly requested without
their dependencies.

```sh
python3 -m venv .venv-visual
.venv-visual/bin/python -m pip install -r tests/visual/requirements.txt
# Install LibreOffice separately; use PATH or SOFFICE to select its executable.
RHODOPSIN_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice -- --ignored --nocapture
.venv-visual/bin/python -m unittest discover -s tests/visual -p 'test_*.py'
```

On Windows, use `.venv-visual/Scripts/python.exe` and your shell's environment
variable syntax. Recorded LibreOffice comparisons were performed on macOS;
Windows/Linux visual equivalence has not been established.

Each comparison uses a new temporary LibreOffice profile to produce PDF, then
PDFium rasterizes PNG. On macOS, `SAL_USE_VCLPLUGIN=osx` retains CoreText font
resolution. No regular user profile or Office automation service is used.

```sh
cargo build --release --locked
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/release/rhodopsin --fixture examples/basic.pptx \
  --out target/visual-tests/basic-001
```

Artifacts include input PPTX, commands, stdout/stderr, `actual/`, `reference/`
(PDF and PNG), `diff/` (red differences on black), and `report.json`. Failures
retain evidence. Use a new output directory per run.

Default thresholds are normalized RGB MAE <= 0.02 and changed-pixel ratio <= 0.05,
with a changed pixel defined by any channel differing by more than 16. Default
width is 1200px. Only a 1px rasterization-rounding discrepancy is corrected;
page count and aspect ratio mismatches fail. Exit codes: 0 within thresholds,
1 exceeded, 2 execution error. Threshold overrides are recorded in the report;
never relax them to hide a mismatch.

Reports record LibreOffice, OS, PDFium/Pillow versions and input, executable,
and font hashes. Recording an environment does not pin it: reproduce with the
same binaries, fonts, settings, and dependencies. Whole-page metrics can miss
small omissions, so inspect local regions and diagnostics as described in the
[comparator guide](../tests/visual/README.md).

LibreOffice can differ from PowerPoint too. Matching the generating tool's own
preview or passing comparison thresholds is not proof of display compatibility.
Same-font PowerPoint reference regression tests remain future work.

## Spreadsheet crate

`crates/xlsx` holds `rhodopsin-xlsx`, the XLSX-to-PDF library and CLI imported from
the former xlsx2pdf repository as a snapshot of commit fa55270 (2026-10-22), without
history. It is a separate workspace member with its own binary; the PPTX converter
does not depend on it yet. Its design notes live in [docs/xlsx](xlsx/) and are still
written in Japanese. Paths mentioned there (`src/`, `tests/`, `tools/`, `examples/`)
are relative to `crates/xlsx`, and `cargo run --example` needs `-p rhodopsin-xlsx`.

Run its checks from the repository root:

```sh
cargo test --locked -p rhodopsin-xlsx
python3 -m unittest discover -s crates/xlsx/tools -p 'test_*.py'
```

Planned convergence, in order: font discovery and loading, then the PDF writer (the
PPTX PDF backend is expected to move to the CID-font subset embedding used by
`rhodopsin-xlsx`), then a single CLI.

## Implementation references

- [Slide masters](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-slide-masters)
- [Group transforms](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.transformgroup)
- [usvg 0.48.1 FontResolver](https://docs.rs/usvg/0.48.1/usvg/struct.FontResolver.html)
- [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)

Historical results and remaining differences are preserved in
[rendering notes](rendering-notes.md) and the feature-specific documents.
