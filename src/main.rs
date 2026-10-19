//! CLI for the PPTX-to-PNG conversion library.
mod cli_output;
use clap::Parser;
use pptx2img::{ConvertOptions, SlideReport, convert};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Convert PPTX to PNG without Office (early release)",
    after_help = "Fonts: system fonts are loaded by default; application bundles and inaccessible paths may not be searched. Add available font files with --font-file.
Individual missing glyphs use available fallback fonts with warnings; unrenderable glyphs fail. Family substitution requires --font-fallback.
Results: YAML on stdout for success, stderr for failure. Detailed diagnostics are saved in manifest.json on success."
)]
struct Args {
    /// Input PPTX
    input: PathBuf,
    /// Output directory (default: <input>.pptx2img beside the input; never overwritten)
    #[arg(long)]
    out: Option<PathBuf>,
    /// Output image width
    #[arg(long, default_value_t = 1600, value_parser = clap::value_parser!(u32).range(64..=4096))]
    width: u32,
    /// Fail on warnings without writing images
    #[arg(long)]
    strict: bool,
    /// Allow font-family substitution and approximate weight or italic style
    #[arg(long)]
    font_fallback: bool,
    /// Compress punctuation spacing to fit lines (experimental; disabled by default)
    #[arg(long)]
    punctuation_compression: bool,
    /// Additional font file (repeatable)
    #[arg(long)]
    font_file: Vec<PathBuf>,
    /// Load only explicitly supplied fonts, without system fonts
    #[arg(long)]
    no_system_fonts: bool,
    /// Requested=Replacement font mapping (repeatable; one step only)
    #[arg(long, value_parser = parse_font_mapping)]
    font_map: Vec<(String, String)>,
}

fn parse_font_mapping(value: &str) -> std::result::Result<(String, String), String> {
    let (from, to) = value
        .split_once('=')
        .ok_or("Specify Requested=Replacement")?;
    if from.trim().is_empty() || to.trim().is_empty() {
        return Err("Font family names must not be empty".into());
    }
    Ok((from.trim().into(), to.trim().into()))
}
fn print_warnings(slides: &[SlideReport]) {
    let mut fonts = std::collections::BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    for slide in slides {
        for diagnostic in &slide.diagnostics {
            if let Some(font) = &diagnostic.font {
                fonts
                    .entry(font.requested_family.clone())
                    .or_default()
                    .insert(format!("{} ({})", font.post_script_name, font.reason));
            } else {
                eprintln!("Warning [slide {}]: {}", slide.slide, diagnostic.message);
            }
        }
    }
    for (requested, actual) in &fonts {
        eprintln!(
            "Font substitution: {requested} -> {}",
            actual.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    if !fonts.is_empty() {
        eprintln!(
            "Add fonts with --font-file or select replacements with --font-map 'Requested=Replacement'. See diagnostics.font in manifest.json for details."
        );
    }
}
fn main() {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            error.exit()
        }
        Err(error) => {
            eprint!("{}", cli_output::invalid_arguments(&error.to_string()));
            std::process::exit(2);
        }
    };
    let out = args
        .out
        .unwrap_or_else(|| args.input.with_extension("pptx2img"));
    let existed = out.exists();
    match convert(
        &args.input,
        &out,
        &ConvertOptions {
            width: args.width,
            strict: args.strict,
            font_fallback: args.font_fallback,
            punctuation_compression: args.punctuation_compression,
            font_file: args.font_file,
            no_system_fonts: args.no_system_fonts,
            font_mapping: args.font_map.into_iter().collect(),
        },
    ) {
        Ok(report) => {
            print_warnings(&report.slides);
            print!("{}", cli_output::completed(&out, &report.slides));
        }
        Err(error) => {
            eprint!(
                "{}",
                cli_output::failed(&error, Some(&out), !existed && out.exists())
            );
            std::process::exit(1);
        }
    }
}
