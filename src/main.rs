//! PPTX を直接解析し、SVG を介して PNG に変換する独立 CLI。
mod pptx;

use anyhow::{Context, Result, bail};
use clap::Parser;
use serde::Serialize;
use std::{fs, path::PathBuf};

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
    /// 行幅が不足するとき句読点の余白を自動圧縮する（実験的・既定は無効）
    #[arg(long)]
    punctuation_compression: bool,
    /// システムフォントに加えて読み込むフォント（複数指定可）
    #[arg(long)]
    font_file: Vec<PathBuf>,
    /// システムフォントを読まず、指定したフォントだけを使う
    #[arg(long)]
    no_system_fonts: bool,
}

#[derive(Serialize)]
struct Manifest {
    schema_version: u32,
    version: &'static str,
    status: &'static str,
    width: u32,
    height: u32,
    punctuation_compression: bool,
    system_fonts: bool,
    fonts: Vec<FontReport>,
    slides: Vec<SlideReport>,
}
#[derive(Serialize)]
struct FontReport {
    family: String,
    post_script_name: String,
    face_index: u32,
}
#[derive(Serialize)]
struct SlideReport {
    slide: usize,
    image: String,
    warnings: Vec<String>,
    diagnostics: Vec<pptx::Diagnostic>,
}

fn main() {
    if let Err(error) = run_with_stack(Args::parse()) {
        eprintln!("エラー: {error:#}");
        std::process::exit(1);
    }
}
fn run_with_stack(args: Args) -> Result<()> {
    // OOXML/SVG パーサーには再帰処理がある。OS のメインスレッド既定値に
    // 依存させず、検証済みの macOS と同じ 8 MiB を変換処理に確保する。
    // グループの 64 段制限は維持し、超過時にはクラッシュせず診断を返す。
    std::thread::Builder::new()
        .name("pptx-render".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || run(args))
        .context("変換用スレッドを作成できません")?
        .join()
        .map_err(|_| anyhow::anyhow!("変換処理が異常終了しました"))?
}
fn run(args: Args) -> Result<()> {
    let out = args
        .out
        .unwrap_or_else(|| args.input.with_extension("pptx2img"));
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    let mut options = resvg::usvg::Options::default();
    if !args.no_system_fonts {
        options.fontdb_mut().load_system_fonts();
    }
    for path in &args.font_file {
        options
            .fontdb_mut()
            .load_font_file(path)
            .with_context(|| format!("フォントを読めません: {}", path.display()))?;
    }
    // 計測した face ID を描画にも渡し、同名フォントの再選択を避ける。
    let aliases: std::collections::HashMap<_, _> = options
        .fontdb
        .faces()
        .map(|f| (format!("pptx2img-face-{}", f.id), f.id))
        .collect();
    options.font_resolver.select_font = Box::new(move |font, _| {
        font.families().iter().find_map(|family| match family {
            resvg::usvg::FontFamily::Named(name) => aliases.get(name).copied(),
            _ => None,
        })
    });
    // 代替は計測時に解決する。描画側だけで別のフォントに変えない。
    options.font_resolver.select_fallback = Box::new(|_, _, _| None);
    let deck = pptx::read(
        &args.input,
        &pptx::RenderOptions {
            fonts: &options.fontdb,
            punctuation_compression: args.punctuation_compression,
        },
    )?;
    let height = (f64::from(args.width) * deck.height / deck.width).round();
    if !(1.0..=8192.0).contains(&height) {
        bail!("出力画像の高さが上限を超えています");
    }
    let height = height as u32;
    if u64::from(args.width) * u64::from(height) > 16_777_216 {
        bail!("出力画像の画素数が上限を超えています");
    }
    if options.fontdb.is_empty() && deck.slides.iter().any(|s| s.svg.contains("<text")) {
        bail!("フォントがありません。--font-file で指定してください");
    }
    let has_warnings = deck.slides.iter().any(|s| !s.warnings.is_empty());
    for (index, slide) in deck.slides.iter().enumerate() {
        for warning in &slide.warnings {
            eprintln!("警告 [スライド {}]: {warning}", index + 1);
        }
    }
    if args.strict && has_warnings {
        bail!("未対応・近似描画を検出したため、strict モードで停止しました");
    }
    // 全スライドの構文を確認してから出力先を作る。失敗した場合は完了 manifest を残さない。
    let trees: Vec<_> = deck
        .slides
        .iter()
        .map(|slide| resvg::usvg::Tree::from_str(&slide.svg, &options))
        .collect::<Result<_, _>>()?;
    fs::create_dir(&out).context("出力ディレクトリを作成できません")?;
    let mut reports = Vec::new();
    for (index, (slide, tree)) in deck.slides.iter().zip(trees).enumerate() {
        let name = format!("slide-{:03}.png", index + 1);
        let mut pixmap = resvg::tiny_skia::Pixmap::new(args.width, height)
            .context("描画領域を確保できません")?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                args.width as f32 / deck.width as f32,
                height as f32 / deck.height as f32,
            ),
            &mut pixmap.as_mut(),
        );
        pixmap.save_png(out.join(&name))?;
        reports.push(SlideReport {
            slide: index + 1,
            image: name,
            warnings: slide.warnings.clone(),
            diagnostics: slide.diagnostics.clone(),
        });
    }
    let manifest = Manifest {
        schema_version: 2,
        version: env!("CARGO_PKG_VERSION"),
        status: if has_warnings {
            "preview_with_warnings"
        } else {
            "rendered"
        },
        width: args.width,
        height,
        punctuation_compression: args.punctuation_compression,
        system_fonts: !args.no_system_fonts,
        fonts: options
            .fontdb
            .faces()
            .filter(|f| {
                deck.slides.iter().any(|s| {
                    s.svg
                        .contains(&format!("font-family=\"pptx2img-face-{}\"", f.id))
                })
            })
            .map(|f| FontReport {
                family: f.families.first().map(|f| f.0.clone()).unwrap_or_default(),
                post_script_name: f.post_script_name.clone(),
                face_index: f.index,
            })
            .collect(),
        slides: reports,
    };
    fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    eprintln!(
        "{} 枚を出力しました。出力先: {}/",
        manifest.slides.len(),
        out.display()
    );
    Ok(())
}
