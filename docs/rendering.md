# Rendering support and limitations

This is an early, approximate renderer, not a complete PowerPoint implementation.
Unsupported content is reported where detected; detection itself is incomplete.
Strict mode gates known warnings, not all incompatibilities.

## Supported subset

- Transitional OOXML PPTX, using presentation-defined slide order.
- Explicit positions, sizes, rotation, and flips for basic shapes and selected
  [presets and connectors](preset-shapes.md).
- Solid backgrounds, fills, and lines using RGB, system `lastClr`, and theme
  colors; selected [color transforms and theme fills](colors.md).
- Slide -> layout -> master -> theme relationships, color mappings, and major/
  minor Latin and Japanese theme fonts, including the Jpan supplemental font.
- Explicit font, size, color, bold/italic, and left/center/right paragraph alignment.
- Nominal font-advance wrapping, basic Japanese prohibited breaks, explicit line
  breaks and empty lines, paragraph spacing, point/percentage line spacing, and
  top/center/bottom vertical anchoring with overflow warnings.
- `hangingPunct` inherited from paragraph, list, master, and presentation defaults;
  one trailing punctuation character (`、。，．`) may hang outside the box.
- Explicit character spacing `spc`, including negative values, positive values,
  inherited defaults, and explicit zero; optional automatic punctuation compression.
- Property-by-property inheritance from runs, paragraph defaults, and list levels.
- Embedded PNG/JPEG placement and `srcRect` cropping.
- Basic placeholder inheritance and shared master/layout shapes and solid backgrounds.
- Nested group transforms, explicit table cell formatting, rectangular merged
  cells, selected [table styles](table-styles.md), [outer shadows](shadows.md), and
  [static OLE previews](ole-preview.md).

## Inheritance

Placeholders match by `idx` from slide to layout and by type from layout to master.
Template sample text is not rendered; inherited properties supplement slide text.
Explicit slide properties win. `showMasterSp="0"` hides shared ancestor shapes.
This covers transforms, solid fills, and basic text styles, not every property.

Ordinary text boxes (`txBox=1`) use `presentation/defaultTextStyle`. Placeholders
inherit layout/master styles, while text inside shapes uses `otherStyle`.
Run, paragraph, and list values override defaults. Empty paragraphs use the size
and font metrics in `endParaRPr`.

The independently generated `examples/text-inheritance.pptx` compares explicit
and inherited 18pt Arial. Both resolve to 18pt in LibreOffice, but vertical position
and line-spacing differences exceed existing image thresholds. Overflow is
reported rather than silently discarded.
[Specification entry point](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-presentations).

## Saved autofit

`normAutofit/fontScale` (default 100%) applies to measurement, wrapping, and drawing
of all runs. `lnSpcReduction` (default 0%) subtracts from percentage line spacing:
120%-20%=100%. Fixed-point line spacing is unchanged. Child `noAutofit` cancels
inherited autofit. The renderer does not search for a new shrink ratio.

`spAutoFit` expands height for fixed-width horizontal text, preserving anchoring
and margins. Table-cell/vertical autofit and horizontal expansion without wrapping
are unsupported and warned about. Shape text areas are remeasured at their final
size, with diagnostics for expansion limits.

Fixture: `examples/text-autofit.pptx`; generator:
`tests/visual/make_text_autofit.py`. LibreOffice 26.2.5.2 recalculates this fixture's
saved shrink ratios and does not expand its saved boxes, so pixel differences
remain. Thresholds have not been relaxed.

- [NormalAutoFit](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit)
- [Office spAutoFit interpretation](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/19d31037-28bf-4693-85be-3a37bfe82d51)

## Bullets and indentation

`buChar` and `buAutoNum` are placed separately from text. Bullet position is
`marL+indent`; body and wrapped lines start at `marL`; `marR` defines the right
margin. Paragraph/list/master settings, bullet font/size/color, and `buNone`
overrides are supported. Bullet fonts use the same resolution rules as body text.

Automatic numbering supports Arabic, alphabetic, and Roman sequences with periods,
closing parentheses, paired parentheses, or no decoration. Counters are per level.
A direct paragraph `startAt` restarts numbering; an inherited start applies only
at the first entry. Picture bullets are unsupported. Other numbering formats and
non-left-aligned bullet positions produce approximation warnings.

Fixture: `examples/text-bullets.pptx`; generator: `tests/visual/make_text_bullets.py`.
Numbers, levels, and wrapping were checked against LibreOffice, but vertical
position and line-spacing differences still exceed existing image thresholds.

## Known limitations

- Complete layout/master property and text-style inheritance; `themeOverride` and
  full theme style matrices (individual fill/line references have partial support).
- Full Japanese line-breaking rules, punctuation compression, all hanging symbols,
  complex shaping/ligatures/kerning, and dynamic autofit ratio calculation.
- Identical baselines and fallback behavior across all fonts and Office renderers.
- Charts, SmartArt, group-fill inheritance, freeform shapes, and many presets.
- Full table theme-style coverage, gradients, inner shadows/glow, theme effects,
  and image tiling/effects/fill rectangles. Selected color alpha and outer shadows
  are supported; see the feature-specific notes for their limits.
- SVG/EMF/WMF images, external links, animations, Strict OOXML, legacy `.ppt`,
  and encrypted files.

Visual output depends on available fonts. Ordinary CI covers Windows/Linux/macOS;
recorded visual comparison measurements are from macOS.

## Resource limits

Input size: 128 MiB. Total uncompressed data: 256 MiB. Per part: 32 MiB.
Maximum slides: 200. Group nesting: 64 levels. Tables: 100000 cells.
Output and embedded images also have pixel-count limits.

ZIP entries are read without extracting files to the filesystem. External images
are not fetched. Recursive conversion uses an 8 MiB thread stack so OS main-stack
size differences do not cause parser failures. Tests require a normal error exit
and no output for groups deeper than 64, rather than accepting a crash as rejection.
Additional robustness validation is needed before exposing a service to arbitrary
untrusted input.

For font requirements and diagnostics, see [usage](usage.md). For measurement
models, sources, and historical comparison evidence, see [rendering notes](rendering-notes.md).
