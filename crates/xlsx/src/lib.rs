pub mod cell_paint;
pub mod cell_text;
pub mod color;
pub mod diagnostic;
pub mod drawing;
mod font_catalog;
mod font_names;
pub mod font_sources;
pub mod fonts;
pub mod format;
pub mod grid;
pub mod image_geometry;
pub mod layout;
pub mod output;
pub mod package;
pub mod page_borders;
pub mod page_scene;
pub mod pdf;
pub mod placement;
pub mod print;
pub mod range;
pub mod render;
pub mod report;
pub mod rich_text;
pub mod styles;
pub mod system_fonts;
mod text_direction;
pub mod text_layout;
pub mod text_overflow;
pub mod theme;
pub mod warnings;
pub mod workbook;
pub mod workbook_layout;
mod xml;
mod xstring;

use diagnostic::{Failure, Result};
use package::{Limits, Package};
use std::path::Path;

#[derive(Default)]
pub struct ConversionOptions {
    pub font_sources: font_sources::Sources,
    pub font_search_limits: font_sources::SearchLimits,
    pub font_limits: fonts::FontLimits,
    /// 不足書体だけに適用する明示的な代用。書体の曖昧さ・埋め込み制限は回避しない。
    pub fallback_family: Option<String>,
    pub system_fonts: bool,
}

impl ConversionOptions {
    fn validate(&self) -> Result<()> {
        if self.fallback_family.as_ref().is_some_and(|name| {
            name.trim().is_empty() || name.len() > 1024 || name.chars().any(char::is_control)
        }) {
            return Err(Failure::input("invalid_fallback_font"));
        }
        Ok(())
    }
}

pub struct PreparedWorkbook {
    pub workbook: workbook::Workbook,
    pub fonts: fonts::FontBook,
}

impl PreparedWorkbook {
    /// 準備済み入力と変換設定をセル描画へ接続する。
    /// ロケール・文字体系は呼び出し側が指定し、OS 設定から推測しない。
    /// 自動色は未確定のまま返すため、印刷条件を確定した呼び出し側で設定する。
    pub fn text_context<'a>(
        &'a self,
        options: &'a ConversionOptions,
        locale: format::date::Locale,
        script: theme::TextScript<'a>,
        warning_limit: usize,
    ) -> Result<cell_text::Context<'a>> {
        options.validate()?;
        let styles = self
            .workbook
            .styles
            .as_ref()
            .ok_or_else(|| Failure::unsupported("cell_styles_unresolved"))?;
        Ok(cell_text::Context {
            styles,
            fonts: &self.fonts,
            colors: color::ColorContext {
                theme: self.workbook.theme.as_ref(),
                indexed: &styles.indexed_colors,
                automatic: None,
                foreground: None,
                background: None,
            },
            format: format::FormatContext {
                date_system: if self.workbook.date_1904 {
                    format::date::DateSystem::Excel1904
                } else {
                    format::date::DateSystem::Excel1900
                },
                locale,
            },
            script,
            normal_script: script,
            font_limits: &options.font_limits,
            warning_limit,
            fallback_family: options.fallback_family.as_deref(),
        })
    }
}

/// 入力検査と、明示されたフォントの読み込みまでを実施する。
/// この結果だけでは全セルを描画できることを保証しない。
pub fn prepare(
    input: &Path,
    limits: &Limits,
    options: &ConversionOptions,
) -> Result<PreparedWorkbook> {
    options.validate()?;
    let package = Package::open(input, limits)?;
    let workbook = workbook::inspect(&package, limits)?;
    let mut fonts = fonts::FontBook::default();
    options.font_sources.load(
        &mut fonts,
        &options.font_search_limits,
        &options.font_limits,
    )?;
    if options.system_fonts {
        let mut families = std::collections::BTreeSet::new();
        if let Some(styles) = &workbook.styles {
            for font in styles.fonts.iter().filter_map(|font| font.as_ref().ok()) {
                if let Some(family) = &font.family {
                    families.insert(family.to_ascii_lowercase());
                }
            }
        }
        for run in workbook
            .sheets
            .iter()
            .flat_map(|sheet| &sheet.cells)
            .flat_map(|cell| match &cell.value {
                workbook::CellValue::RichText(runs) => runs.as_ref(),
                _ => &[],
            })
        {
            if let Some(family) = run.font.as_ref().and_then(|font| font.family.as_ref()) {
                families.insert(family.to_ascii_lowercase());
            }
        }
        if let Some(theme) = &workbook.theme {
            for group in [&theme.major, &theme.minor] {
                for family in [&group.latin, &group.east_asian, &group.complex]
                    .into_iter()
                    .chain(group.scripts.values())
                {
                    if !family.is_empty() {
                        families.insert(family.to_ascii_lowercase());
                    }
                }
            }
        }
        if let Some(family) = &options.fallback_family {
            families.insert(family.to_ascii_lowercase());
        }
        let sources = font_sources::Sources {
            files: Vec::new(),
            directories: system_fonts::directories()?,
        };
        sources.load_matching(
            &mut fonts,
            &options.font_search_limits,
            &options.font_limits,
            &families,
            64 * 1024 * 1024,
        )?;
    }
    Ok(PreparedWorkbook { workbook, fonts })
}

/// 描画実装の追加中も、未実装の変換を成功として返さない。
pub fn convert(input: &Path, output: &Path, limits: &Limits) -> Result<report::Report> {
    convert_with_options(input, output, limits, &ConversionOptions::default())
}

pub fn convert_with_options(
    input: &Path,
    output: &Path,
    limits: &Limits,
    options: &ConversionOptions,
) -> Result<report::Report> {
    output::check_available(output)?;
    let _prepared = prepare(input, limits, options)?;
    Err(Failure::unsupported("rendering_not_implemented"))
}

/// 入力の準備から組版・排他的保存までを実行し、確定後にだけ成功情報を返す。
/// 配置規則は呼び出し側が検証して指定する。CLI の既定規則を代用しない。
/// 文字のはみ出しは既存の占有判定へ接続し、frame の clip を置き換える。
pub fn convert_with_policy(
    input: &Path,
    output: &Path,
    limits: &Limits,
    options: &ConversionOptions,
    environment: &render::Environment<'_>,
    render_limits: &render::Limits,
    frame: impl FnMut(
        &workbook::Sheet,
        &layout::Page,
        &workbook::Cell,
        &placement::RangePlacement,
    ) -> Result<cell_text::Frame>,
) -> Result<report::Report> {
    output::check_available(output)?;
    let prepared = prepare(input, limits, options)?;
    let rendered =
        prepared.render_with_cell_overflow(options, environment, render_limits, frame)?;
    output::publish(output, &rendered.bytes)?;
    Ok(rendered.report)
}
