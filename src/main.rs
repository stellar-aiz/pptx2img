//! PPTX → PNG 変換ライブラリの CLI。
mod cli_output;
use clap::Parser;
use pptx2img::{ConvertOptions, SlideReport, convert};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Office 非依存の PPTX → PNG 変換（初期版）")]
struct Args {
    /// 入力 PPTX
    input: PathBuf,
    /// 出力ディレクトリ（省略時は入力と同じ場所の <入力名>.pptx2img、既存は上書きしない）
    #[arg(long)]
    out: Option<PathBuf>,
    /// 出力画像の横幅
    #[arg(long, default_value_t = 1600, value_parser = clap::value_parser!(u32).range(64..=4096))]
    width: u32,
    /// 警告があれば画像を出力せず失敗する
    #[arg(long)]
    strict: bool,
    /// 不足フォント・欠字・太さや斜体の不一致を許容して代替描画する
    #[arg(long)]
    font_fallback: bool,
    /// 行幅が不足するとき句読点の余白を自動圧縮する（実験的・既定は無効）
    #[arg(long)]
    punctuation_compression: bool,
    /// システムフォントに加えて読み込むフォント（複数指定可）
    #[arg(long)]
    font_file: Vec<PathBuf>,
    /// システムフォントを読まず、指定したフォントだけを使う
    #[arg(long)]
    no_system_fonts: bool,
    /// 要求書体=代替書体 の対応（複数指定可、一段のみ）
    #[arg(long, value_parser = parse_font_mapping)]
    font_map: Vec<(String, String)>,
}

fn parse_font_mapping(value: &str) -> std::result::Result<(String, String), String> {
    let (from, to) = value
        .split_once('=')
        .ok_or("要求書体=代替書体 を指定してください")?;
    if from.trim().is_empty() || to.trim().is_empty() {
        return Err("書体名は空にできません".into());
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
                eprintln!("警告 [スライド {}]: {}", slide.slide, diagnostic.message);
            }
        }
    }
    for (requested, actual) in &fonts {
        eprintln!(
            "フォント代替: {requested} → {}",
            actual.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    if !fonts.is_empty() {
        eprintln!(
            "指定書体は --font-file で追加、代替先は --font-map '要求書体=代替書体' で指定できます。詳細は manifest.json の diagnostics.font を参照してください。"
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
            print!("{}", cli_output::completed(&out, report.slides.len()));
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
