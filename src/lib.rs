//! Office に依存せず PPTX を PNG と manifest.json に変換するライブラリ。
//!
//! ```no_run
//! use pptx2img::{convert, ConvertOptions};
//! let report = convert("slides.pptx", "images", &ConvertOptions::default())?;
//! println!("{} 枚を出力", report.slides.len());
//! # Ok::<(), anyhow::Error>(())
//! ```
mod pptx;

use anyhow::{Context, Result, bail};
pub use pptx::{Diagnostic, DiagnosticCode, FontResolution};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// CLI と共通の変換設定。
#[derive(Clone, Debug)]
pub struct ConvertOptions {
    /// 出力幅。64〜4096 ピクセル。
    pub width: u32,
    /// 警告がある場合、ファイルを出力せず StrictModeError を返す。
    pub strict: bool,
    /// 実験的な句読点の自動圧縮。
    pub punctuation_compression: bool,
    /// 追加で読み込むフォントファイル。
    pub font_file: Vec<PathBuf>,
    /// システムフォントを読み込まない。
    pub no_system_fonts: bool,
    /// 要求 family から代替 family への明示対応（一段のみ）。
    pub font_mapping: std::collections::HashMap<String, String>,
}
impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            width: 1600,
            strict: false,
            punctuation_compression: false,
            font_file: Vec::new(),
            no_system_fonts: false,
            font_mapping: Default::default(),
        }
    }
}

/// 出力した manifest.json と同じ内容。警告は呼び出し元が処理する。
#[derive(Clone, Debug, Serialize)]
pub struct ConversionReport {
    pub schema_version: u32,
    pub version: &'static str,
    pub status: &'static str,
    pub width: u32,
    pub height: u32,
    pub punctuation_compression: bool,
    pub system_fonts: bool,
    pub fonts: Vec<FontReport>,
    pub slides: Vec<SlideReport>,
}
#[derive(Clone, Debug, Serialize)]
pub struct FontReport {
    pub family: String,
    pub post_script_name: String,
    pub face_index: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct SlideReport {
    pub slide: usize,
    pub image: String,
    pub warnings: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// strict モードで停止した場合の診断。image は予定ファイル名で、未出力。
/// 返された anyhow::Error の downcast_ref::<StrictModeError>() で取得できる。
#[derive(Debug)]
pub struct StrictModeError {
    pub slides: Vec<SlideReport>,
}
impl std::fmt::Display for StrictModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "未対応・近似描画を検出したため、strict モードで停止しました"
        )
    }
}
impl std::error::Error for StrictModeError {}

/// PPTX を指定したディレクトリへ同期的に変換する。
///
/// 既存の出力先は上書きしない。警告・診断は戻り値に含め、標準出力・
/// 標準エラーには書き込まない。エラーは anyhow::Error で返す。
/// 途中の I/O エラーでは画像が残ることがあるが、完了時だけ manifest を書く。
/// 再帰的な解析のため、内部で 8 MiB のスタックを持つスレッドを使用する。
/// 描画は近似を含み、警告がなくても PowerPoint 互換性を保証しない。
pub fn convert(
    input: impl AsRef<Path>,
    out: impl AsRef<Path>,
    settings: &ConvertOptions,
) -> Result<ConversionReport> {
    let input = input.as_ref().to_path_buf();
    let out = out.as_ref().to_path_buf();
    let settings = settings.clone();
    std::thread::Builder::new()
        .name("pptx-render".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || convert_inner(input, out, settings))
        .context("変換用スレッドを作成できません")?
        .join()
        .map_err(|_| anyhow::anyhow!("変換処理が異常終了しました"))?
}
fn convert_inner(
    input: PathBuf,
    out: PathBuf,
    settings: ConvertOptions,
) -> Result<ConversionReport> {
    if !(64..=4096).contains(&settings.width) {
        bail!("出力画像の横幅は 64〜4096 を指定してください");
    }
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    let mut options = resvg::usvg::Options::default();
    if !settings.no_system_fonts {
        options.fontdb_mut().load_system_fonts();
    }
    for path in &settings.font_file {
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
        &input,
        &pptx::RenderOptions {
            fonts: pptx::FontResolver::new(&options.fontdb, &settings.font_mapping),
            punctuation_compression: settings.punctuation_compression,
        },
    )?;
    let height = (f64::from(settings.width) * deck.height / deck.width).round();
    if !(1.0..=8192.0).contains(&height) {
        bail!("出力画像の高さが上限を超えています");
    }
    let height = height as u32;
    if u64::from(settings.width) * u64::from(height) > 16_777_216 {
        bail!("出力画像の画素数が上限を超えています");
    }
    if options.fontdb.is_empty() && deck.slides.iter().any(|s| s.svg.contains("<text")) {
        bail!("フォントがありません。--font-file で指定してください");
    }
    let has_warnings = deck.slides.iter().any(|s| !s.warnings.is_empty());
    if settings.strict && has_warnings {
        return Err(StrictModeError {
            slides: deck
                .slides
                .iter()
                .enumerate()
                .map(|(index, slide)| SlideReport {
                    slide: index + 1,
                    image: format!("slide-{:03}.png", index + 1),
                    warnings: slide.warnings.clone(),
                    diagnostics: slide.diagnostics.clone(),
                })
                .collect(),
        }
        .into());
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
        let mut pixmap = resvg::tiny_skia::Pixmap::new(settings.width, height)
            .context("描画領域を確保できません")?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                settings.width as f32 / deck.width as f32,
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
    let manifest = ConversionReport {
        schema_version: 2,
        version: env!("CARGO_PKG_VERSION"),
        status: if has_warnings {
            "preview_with_warnings"
        } else {
            "rendered"
        },
        width: settings.width,
        height,
        punctuation_compression: settings.punctuation_compression,
        system_fonts: !settings.no_system_fonts,
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
    Ok(manifest)
}
