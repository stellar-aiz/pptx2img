# CLI and Rust API

## Installation and output

Build with `cargo build --release --locked`, or install from a checkout with
`cargo install --path . --locked`. The initial dependency download needs network
access; conversion itself is local and does not start Office or access the network.

```sh
pptx2img slides.pptx --out images --width 1600
pptx2img slides.pptx --out verified --strict
pptx2img slides.pptx --out images --font-file /path/to/font.ttf
```

Without `--out`, replace the input's final extension with `.pptx2img` beside the
input: `slides.pptx` becomes `slides.pptx2img/`. An explicit output path wins.
Existing output directories are never overwritten. Output consists of numbered
`slide-001.png` files and `manifest.json`. An I/O failure can leave partial images;
the manifest is written only after successful completion.

Output width must be 64–4096 pixels. See `pptx2img --help` for all arguments.

## Font discovery and fallback

System fonts are loaded by default. This is not an exhaustive search of the
filesystem: fonts inside application bundles or inaccessible directories may not
be found. Missing-family errors mean a family is unavailable in the loaded font
database, not necessarily absent from the machine.

Use repeatable `--font-file` arguments for fonts you can access and are entitled
to use. To isolate conversion from system fonts:

```sh
pptx2img slides.pptx --out images \
  --no-system-fonts --font-file /path/to/font.ttf
```

No fonts are downloaded or redistributed automatically. Use the same font files
when reproducing results; equal family names do not guarantee identical versions.

| Condition | Default behavior |
|---|---|
| Requested family unavailable | Fail before creating output |
| Requested weight or italic style unavailable | Fail before creating output |
| Explicit family mapping | Requires `--font-fallback` |
| Individual glyph missing, another font contains it | Continue with a warning |
| No loaded font contains the glyph | Fail even with `--font-fallback` |
| Any warning with `--strict` | Fail before creating output |

`--font-fallback` permits family substitution and style approximation. An explicit
mapping such as `--font-map 'Meiryo UI=Noto Sans CJK JP'` also requires that flag.
Mappings are repeatable, last-wins for the same requested name, and applied once;
cycles do not trigger recursive lookup. If a mapped target is unavailable,
diagnostics record it and selection returns to automatic fallback.

Selection considers character coverage, italic match, weight distance, then
family priority: Noto Sans CJK JP, Noto Sans JP, Yu Gothic, Hiragino Sans, Meiryo,
Arial Unicode MS, Arial, then other families. Ties use PostScript name, family
name, and collection face index. Coverage matters within a requested family too.
Measurement and rendering use the same face; fallback can still change widths
and line heights relative to the original font.

## CLI result documents

Success writes one YAML document to stdout. Failure writes one YAML document to
stderr. Successful conversions also write human-readable warnings to stderr;
do not merge the streams when parsing YAML. `--help` and `--version` retain their
usual text format. Exit codes are 0 for success, 1 for conversion failure, and 2
for invalid arguments.

```yaml
status: completed
output:
  created: true
  folder: images
  slides: 20
warnings:
  - code: FONT_GLYPH_MISSING
    family: Arial
    substitute_family: Hiragino Sans
    slides: [13, 15, 16, 17]
    characters:
      - character: "①"
        codepoint: U+2460
```

The optional `warnings` array summarizes successful glyph fallback by requested
and replacement family. Other rendering warnings remain on stderr and in the
manifest. Actual replacement families depend on the loaded fonts.

```yaml
status: failed
output:
  created: false
errors:
  - code: FONT_NOT_FOUND
    family: Calibri
    slides: [6, 14]
    suggestion: "Add the required font with --font-file <path>. If unavailable, ask the requester to provide it."
```

`errors` groups font problems by code and family, listing distinct slide numbers.
`FONT_GLYPH_MISSING` includes `characters` with `character` and `codepoint`.
Other codes are `FONT_STYLE_MISSING`, `FONT_MAPPING_TARGET_NOT_FOUND`,
`FONT_SUBSTITUTION_REQUIRED`, `STRICT_MODE_REJECTED`, `CONVERSION_FAILED`, and
`INVALID_ARGUMENTS`. Missing-font errors do not recommend bypassing the problem
with fallback. Treat codes and fields as structured data, not message text.

`output.created` says whether this invocation created the output directory.
A write failure may return `status: failed` with `created: true` and `folder`.
Refusing an existing directory returns `created: false`. Dynamic strings are
quoted and escaped, including newlines and punctuation in paths and font names.

## Manifest diagnostics

The manifest uses `schema_version: 2`. `slides[].warnings` contains deduplicated
messages; `slides[].diagnostics` retains separate records for different targets,
even when their warning text is the same. The slide number is on the parent entry.

```json
{
  "code": "omitted",
  "part": "ppt/slides/slide1.xml",
  "object_id": "42",
  "object_name": "Chart",
  "location": "spTree/0/2",
  "message": "Omitted unsupported element: non-table graphicFrame"
}
```

Categories are `unsupported`, `omitted`, `approximation`, `font_substitution`,
`missing_glyph`, and `text_overflow`. Missing object IDs or names are `null`.
Table locations include row and cell indices.

`fonts` records family, PostScript name, and collection face index.
`slides[].diagnostics[].font` records requested and actual families, PostScript
name, face index, and reason. Reasons are `family_missing`, `explicit_mapping`,
`mapped_family_missing`, `glyph_fallback`, and `style_approximation`.
`required_font` and `missing_character` appear where applicable. Use part,
object, location, and slide metadata to identify affected content.

Warnings do not guarantee exhaustive detection. Text currently always produces
an approximation warning; strict mode is not a PowerPoint compatibility test.

## Rust library

The package also provides a library crate. This local-path example does not
assume a crates.io release:

```toml
[dependencies]
pptx2img = { path = "../pptx2img" }
```

```rust,no_run
use pptx2img::{convert, ConvertOptions, FontFallbackError, StrictModeError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = ConvertOptions { width: 1600, ..Default::default() };
    match convert("slides.pptx", "images", &options) {
        Ok(report) => {
            for slide in &report.slides {
                for warning in &slide.warnings {
                    eprintln!("Slide {}: {warning}", slide.slide);
                }
            }
        }
        Err(error) => {
            if let Some(fonts) = error.downcast_ref::<FontFallbackError>() {
                eprintln!("Font requirements: {:?}", fonts.slides);
            }
            if let Some(strict) = error.downcast_ref::<StrictModeError>() {
                eprintln!("Strict-mode diagnostics: {:?}", strict.slides);
            }
            return Err(error.into());
        }
    }
    Ok(())
}
```

`convert` is synchronous. It writes PNG files and a manifest and returns the same
content as `ConversionReport`. The library writes no messages to stdout/stderr.
Inspect `report.slides` and the public `DiagnosticCode` enum for diagnostics.
`ConvertOptions.font_fallback` defaults to `false`; `font_mapping` holds mappings.

`StrictModeError.slides` and `FontFallbackError.slides` describe pre-output
failures; their `image` values are planned filenames, not existing files.
Other errors also use `anyhow::Error`. The public API may change during this early
implementation stage.
