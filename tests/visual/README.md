# Global, local, and diagnostic comparisons

`compare.py` uses LibreOffice as a reference, not PowerPoint ground truth.
Reference rendering may itself contain overlaps or differences. Passing thresholds
does not prove compatibility. Inspect both images when investigating differences:
source pages 1/20 in the 2026-10-18 local comparison also had reference-side overlaps.
Source documents and images remain local.

For environment setup and whole-page metrics, see
[development](../../docs/development.md).

## Regions

`--regions regions.json` adds per-page local comparisons. `slide` is one-based,
`name` is unique within a page, and `rect` is normalized `[x, y, width, height]`
in 0–1. Out-of-bounds rectangles, zero area, nonexistent pages, and duplicate names
are execution errors.

```json
{
  "regions": [
    {"slide": 1, "name": "small-shape", "rect": [0.1, 0.1, 0.02, 0.02]}
  ]
}
```

Floor the top-left and ceil the bottom-right to obtain the pixel region.
`pixel_rect` records left/top/right/bottom, with right/bottom exclusive.
Local regions use the same thresholds as whole pages: MAE 0.02, changed pixels
0.05, and channel tolerance 16. No per-region thresholds hide differences.

| Report field | Meaning |
|---|---|
| `slides[].passed` / `global_passed` | Whole-page pixel verdict |
| `slides[].regions` | Metrics and verdict per requested region |
| `regions_passed` | Combined region verdict; `null` if none requested |
| `diagnostic_summary` | Counts by diagnostic category and `has_omissions` |
| `slides[].renderer_diagnostics` | Original diagnostics with part/object/location |
| `status` | `difference_exceeded` if global or local thresholds are exceeded |

Diagnostic summaries exist globally and per page. Old manifests without diagnostic
fields report `available: false` and `null` omission status; renderer_warnings are
retained. Exit codes are 0 for both global/local passing, 1 for either exceeding,
and 2 for execution errors.

Diagnostics are independent of pixel verdicts: `global_passed: true` can coexist
with `has_omissions: true`. Local metrics detect differences, not their causes or
object identities. An absence of omitted diagnostics does not prove compatibility.

## Independent small-omission fixture

Generate a small chart surrounded by whitespace. Only fixture generation needs
python-pptx; no runtime dependency is added. Chart omission is a known renderer
limitation, not fixed by this comparator.

```sh
cargo build --locked
.venv-visual/bin/python tests/visual/make_local_omission.py /tmp/local-omission.pptx
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/debug/pptx2img \
  --fixture /tmp/local-omission.pptx --regions /tmp/local-omission.json \
  --width 1600 --out target/visual-tests/local-omission
```

Expect global passing, local failure, omission diagnostics, and exit 1. Without
`--regions`, only whole-page metrics apply and exit is 0, but omission diagnostics
remain in the report.

## Independent unsupported-image fixture

Generate a deck whose slide and shared layout each place a minimal EMF picture
(written byte by byte by the script). pptx2img omits both pictures with diagnostics
and renders every slide; LibreOffice draws the EMF rectangles, so the difference is
a known limitation, not a regression.

```sh
.venv-visual/bin/python tests/visual/make_unsupported_images.py /tmp/unsupported-images.pptx
target/debug/pptx2img /tmp/unsupported-images.pptx --out /tmp/unsupported-images --width 1600 --font-fallback
```

## Independent oversized-image fixture

Generate a deck with a 4400x4400 PNG and a 5800x3456 JPEG, plus a same-layout
baseline deck without pictures for peak-memory comparison. pptx2img downscales both
pictures (longest side 2048 px) with `approximation` diagnostics.

```sh
.venv-visual/bin/python tests/visual/make_oversized_images.py /tmp/oversized-images.pptx
/usr/bin/time -l target/release/pptx2img /tmp/oversized-images.pptx --out /tmp/oversized --width 1600 --font-fallback
/usr/bin/time -l target/release/pptx2img /tmp/oversized-images-baseline.pptx --out /tmp/oversized-baseline --width 1600 --font-fallback
```

## Reproducibility

Output retains input PPTX, commands, stdout/stderr, reference PDF, both PNG sets,
global differences, regions JSON, and a copy of the comparator source. Reports
record hashes for input, renderer, LibreOffice executable, comparator, and regions;
width, thresholds, compression settings; Python/image-library/OS versions; font
paths and hashes; and renderer-resolved fonts. Reproduce with the same binaries,
fonts, OS, dependencies, and saved settings, using a new output directory.

```sh
.venv-visual/bin/python -m unittest discover -s tests/visual -p 'test_*.py'
cargo test --locked
```

Unit tests cover small omissions missed globally but caught locally, rectangle
validation, diagnostic aggregation, old manifests, and existing threshold boundaries.
Publish only independently created fixtures, not private source documents or images.

The comparator explicitly passes `--font-fallback` to measure approximate rendering;
font diagnostics remain in the manifest. The standalone CLI rejects family
substitution by default, permits successful glyph fallback with warnings, and
always rejects glyphs unavailable in every loaded font.
