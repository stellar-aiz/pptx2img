# Text rendering experiments and historical results

These records were moved from the former README. They describe measurements at
the time of implementation, not a claim that current CI or full Office compatibility
has been verified. Unless stated otherwise, measurements below used macOS,
LibreOffice 26.2.5.2, Hiragino Sans, and unchanged comparison thresholds.

## Text and theme layout (2026-10-15)

| Fixture / page | Changed pixels, before -> after | MAE, before -> after |
|---|---|---|
| basic.pptx | 4.21% -> 3.91% | 0.01694 -> 0.01472 |
| text-theme.pptx / 1 | 1.49% -> 1.31% | 0.00633 -> 0.00511 |
| text-theme.pptx / 2 | 4.31% -> 3.30% | 0.02256 -> 0.01508 |

Comparisons used identical inputs and reference images before and after changes.
Fixed-line first-line placement and inherited hanging punctuation improved the
second page sufficiently to meet the existing thresholds. Both text-theme pages
and theme.pptx are regression fixtures.

```sh
python tests/visual/compare.py --binary target/release/pptx2img \
  --fixture examples/text-theme.pptx --out target/visual-tests/text-theme-001
```

Widths use nominal advances. Vertical placement prefers Windows OS/2 cell metrics,
falling back to typographic/hhea metrics. Per-character x positions improved
consistency, but complex shaping and exact font fallback remain incomplete.

## Explicit character spacing

DrawingML [`spc`](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.textcharacterpropertiestype.spacing)
is read in 1/100pt. Negative values tighten spacing and positive values expand it,
without horizontally scaling glyphs. Spacing applies only between characters in
the same style span, not at line ends or style boundaries; LibreOffice PDF positions
confirmed this behavior. Excessively negative spacing warns and clamps advance to 0.

An initial fixture without `spc` did not compress punctuation even in narrow boxes;
changing `kern` had no effect. Later experiments reproduced compression by changing
LibreOffice's Asian Layout configuration. This observation is specific to those
fixtures and environments. Default rendering still preserves unspaced advances.

At 1200px width on 2026-10-15, the five-page fixture produced:

| Case | MAE before | MAE after |
|---|---:|---:|
| Punctuation spacing | 0.03574 | 0.01840 |
| Whole-string spacing | 0.03869 | 0.01716 |
| Zero-spacing control | 0.01842 | 0.01842 |
| Wrapping | 0.03328 | 0.01880 |
| Inheritance and overrides | 0.02380 | 0.01243 |

All pages met both existing thresholds after the change. Tests also check advances,
wrapping, style boundaries, inheritance, explicit zero, and SVG positions. Fixtures
include left/center/right alignment, tight boxes, positive spacing, and 18pt/24pt.

```sh
python tests/visual/make_character_spacing.py examples/character-spacing.pptx
```

## Automatic punctuation compression

`--punctuation-compression` is an experimental rendering policy, not detection of
a setting inside PPTX. It defaults to false, is stored in the manifest, and emits
an approximation warning when enabled.

LibreOffice [Asian Layout](https://help.libreoffice.org/latest/en-US/text/shared/optionen/01150100.html)
provides no compression, punctuation only, and punctuation plus kana. Its public
[CharacterCompressionType API](https://api.libreoffice.org/docs/idl/ref/CharacterCompressionType_8idl_source.html)
uses 0/1/2. Changing
`org.openoffice.Office.Common/AsianLayout/CompressCharacterDistance` from 0 to 1
in separate temporary profiles changed tight-line wrapping but not loose lines.
This is separate from kerning; it does not establish PowerPoint defaults or whether
that setting is stored in every PPTX.

The implementation compresses horizontal wrapped text with zero/unspecified `spc`
only as far as needed, up to half-width spacing on full-width punctuation and
brackets. It adjusts space before opening brackets and after closing brackets or
punctuation, not glyph size. It does not compress kana, middle dots, exclamation/
question marks, or `wrap="none"` text. Nonzero explicit spacing takes precedence
and produces a warning. Some wrapped lines retain maximum compression, as below.

Eight identical pages are compared OFF/ON. Evidence includes temporary profile
settings (`libreoffice-profile.xcu`), commands, input hashes, and mode. Normal
comparisons explicitly disable compression too. Tests require each renderer to
change on affected pages, preserve controls, and fail when only our renderer's
compression is disabled against the enabled reference.

For dense-punctuation page 7 on 2026-10-15, MAE improved from 0.03124 to 0.00637
and changed pixels from 4.14% to 1.60%. Local differences can remain below global
thresholds, so unit tests also cover advances, positions, breaks, and empty lines.

```sh
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice automatic_punctuation -- --ignored --nocapture
python tests/visual/make_auto_punctuation.py examples/auto-punctuation.pptx
```

## Boundary widths (2026-10-15)

For zero-spacing lines, box widths are rounded to 1/100mm; cumulative candidate
line widths and per-character compression allowances are truncated in those units.
This approximates observed LibreOffice behavior, not a universal OOXML rule.
With compression ON, 24pt `あ、いう、え` wraps at 120pt but fits at 120.01pt;
at 18pt the boundary is 90.01/90.02pt.

`examples/boundary-widths.pptx` contains 162 single-text-box pages at
12/18/20/21.5/24/32pt, testing within +/-0.1pt of boundaries including 0.01pt steps.
OFF/ON gives 324 conditions. In addition to pixels, comparisons check line counts
and normalized per-line ink shares (difference <= 0.02). This is a dedicated
black-on-white regression detector, not OCR. Tests detect empty images and a
one-character move even when line count stays the same. Results are recorded in
`line-break-report.json`.

```sh
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice boundary_widths -- --ignored --nocapture
python tests/visual/make_boundary_widths.py examples/boundary-widths.pptx --include-prefix-cases
```

The initial 63-page, 126-condition experiment at 12/18/24pt reduced line-break
mismatches from 27 to 2. Whole-page metrics passed even before the fix, motivating
the line-level checks. The separate 72-page `boundary-font-metrics.pptx` preserves
all 144 OFF/ON conditions, including 12pt OFF at 59.98/59.99pt.

```sh
python tests/visual/compare_boundary_widths.py --binary target/release/pptx2img \
  --fixture examples/boundary-font-metrics.pptx --out /tmp/font-metrics-comparison
python tests/visual/make_boundary_widths.py examples/boundary-font-metrics.pptx \
  --sizes 12 20 21.5 32
```

No exceptions or relaxed thresholds hide mismatches. Results apply to single-font,
no-margin fixtures, not all mixed-font layouts. Matching line breaks does not
establish identical compression or glyph positions.

## Font width measurement (2026-10-15)

Sized glyph metrics include fixed-point rounding. Fetch advances in font units
and scale with double precision. For Impress-like measurement, round font height
to 1/100mm, then separately to integer coordinates on a 600 DPI reference device.
Measured full-width advances become 26.72px at 20pt, 28.64px at 21.5pt, and 42.72px
at 32pt. SVG glyph size remains unchanged.

Sources consulted to understand rounding, without porting their code:

- [skrifa 0.44.0 Size](https://docs.rs/skrifa/0.44.0/skrifa/instance/struct.Size.html)
- [Impress reference device](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/sd/source/ui/app/sdmod.cxx#L79)
- [Font height conversion](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/vcl/source/outdev/font.cxx#L686)
- [Measurement scaling](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/vcl/source/font/LogicalFontInstance.cxx#L115)

This independently implemented model approximates verified Impress behavior,
not every Office version or font. Complex shaping and kerning remain separate
limitations. More accurate widths exposed explicit-spacing boundary differences;
rounding each same-style span for nonzero spacing preserved existing comparisons.

In the 72-page/144-condition fixture, mismatches fell from 65 to 2. The improved
54 pages at 20/21.5/32pt entered automated boundary regression tests. The remaining
two 12pt cases were fixed by cumulative-width handling below.

`tests/fixtures/metrics.ttf` uses independently generated rectangular glyphs and
advances, without external glyphs. Six sizes test full/half-width, space, zero-width
ratios, unchanged rendering size, and missing-character warnings. Only regeneration
needs fontTools:

```sh
python tests/fixtures/make_metrics_font.py
```

## Remaining 12pt boundaries (2026-10-15)

The first five characters of 12pt `あ、いう、え` have nominal width 60pt, about
2116.67 in 1/100mm. Boxes at 59.98/59.99pt round to 2116. Rounding cumulative text
width to 2117 rejected the fifth comma and pushed out the preceding character
under prohibited-break rules.

Truncating the cumulative candidate width fixed this without changing individual
advances, drawing positions, or break rules. Truncating each character separately
would accumulate error. Explicit-spacing span rounding remains unchanged.
Controls `あいうえおか`, `あいうえお`, and `あ、いう、` confirmed the same boundary;
there is no punctuation-specific exception. Tests now include 36/48/60pt boundaries
at 12pt and final-line cases. The remaining 72-page mismatches fell from 2 to 0.

The investigation also consulted [cumulative character positions](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/vcl/source/outdev/text.cxx#L850)
and [prohibited-break processing](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/editeng/source/editeng/impedit3.cxx#L1976).
No code was ported. Exact punctuation compression and glyph positions remain a
separate concern.

## Retained compression and inline positions (2026-10-15)

At integer boundaries where a wrapped line regains its natural width, maximally
compressed positions can persist. At 24pt, `あ、いう、え` nearly restores spacing
at 119.9pt, but retains maximum compression on the first line at 119.95–120pt.
Final lines without following text compress only enough to fit.

The retention decision compares rounded natural/box widths in 1/100mm with a
one-unit boundary allowance. This allowance is exclusive to retention: line-break
and image thresholds are unchanged. [Position restoration](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/editeng/source/editeng/impedit3.cxx#L4917)
was consulted for behavior, not ported. No LibreOffice runtime dependency was added.

The 162-page `examples/punctuation-positions.pptx` covers commas, periods, and
brackets near 12/18/24pt boundaries. Across 324 OFF/ON conditions, inline-position
mismatches fell from 38 to 0; six existing LibreOffice comparison suites also passed.

In addition to pixel/line checks, normalized horizontal ink cumulative distributions
measure displacement. Both transport distance and edge offsets must be <= 2px at
1200px width (about 0.48pt in these fixtures). This detects internal movement with
unchanged outer bounds, but is not OCR or per-character coordinate matching.
Observed maxima were about 1.91px transport and 2px edge offset.

```sh
python tests/visual/compare_punctuation_positions.py --binary target/release/pptx2img \
  --fixture examples/punctuation-positions.pptx --out /tmp/punctuation-positions
python tests/visual/make_punctuation_positions.py examples/punctuation-positions.pptx
```

`position-report.json` and OFF/ON evidence retain results. The ignored Rust test
`punctuation_positions_match_both_modes` invokes this comparison too.

Nested brackets `「「あ」」い」` still differ. The known failing fixture remains
separate from passing regression cases; the following command reports failure:

```sh
python tests/visual/compare_punctuation_positions.py --binary target/release/pptx2img \
  --fixture examples/punctuation-position-nested.pptx --out /tmp/punctuation-nested
python tests/visual/make_punctuation_positions.py \
  examples/punctuation-position-nested.pptx --sizes 24 --nested
```

Mixed fonts, margins, and complex shaping are not proven compatible; approximation
warnings remain.

## Inheritance, composites, and diagnostics milestone (2026-10-15)

That milestone added shared master/layout shapes and basic placeholder inheritance;
group transforms, explicit/merged tables and image cropping; consistent measured
font IDs and fallback remeasurement; three-OS CI and pinned visual comparisons;
and schema-version-2 diagnostics by part, object, and location.

`examples/composite-shapes.pptx` compares four pages of groups, merged tables,
cropped images, and inherited titles. Regenerate with python-pptx 1.0.2:

```sh
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice composite_shapes -- --ignored --nocapture
python tests/visual/make_composite_shapes.py
```

The historical local checkpoint passed 45 Rust tests, 14 Python comparator tests,
eight LibreOffice suites, fmt, clippy, and actionlint. These are historical counts;
see current CI for current results. Pixel, line-break, and inline-position thresholds
were unchanged. PowerPoint reference-image comparison was not performed.

Current capabilities that supersede older limitations are listed in
[rendering support](rendering.md), including saved autofit, table styles, and shadows.

## Empty bullet paragraphs (2026-10-20)

PowerPoint for Mac 16.105.3 (26020123), macOS 27.0.1 (26A434), displayed
`examples/empty-bullets.pptx` at 100% zoom. Paragraph 5 contains one space and
shows a bullet both unselected and with the insertion caret in that paragraph.
Paragraphs 3, 7, and 8 contain no runs and show no marker. The recorded
LibreOffice 26.2.5.2 reference agrees on this fixture.

The marker rule now checks for a text run (`a:r` or `a:fld`) rather than a
non-whitespace glyph. Empty-line height and auto-numbering counters retain their
existing behavior. Empty-text runs, break-only paragraphs, other whitespace, and
auto-numbering across empty paragraphs were not independently checked in
PowerPoint; this fixture does not establish full bullet compatibility.

The independent fixture generator preserves the observed paragraph contents:

```sh
python tests/visual/make_empty_bullets.py examples/empty-bullets.pptx
```

The [Issue #53 observation](https://github.com/stellar-aiz/pptx2img/issues/53#issuecomment-6286206284)
records the manual check. The original was neither edited nor saved.
