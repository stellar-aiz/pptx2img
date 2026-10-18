//! フォントの公称 advance と行メトリクスによる横書き配置。
//! 句読点のぶら下がりと固定行送りを解釈する。複雑な shaping などは未対応。
use super::fonts::Resolver;
use super::*;
#[cfg(test)]
use resvg::usvg::fontdb::Database;
use skrifa::raw::TableProvider;
use skrifa::{
    FontRef, MetadataProvider,
    instance::{LocationRef, Size},
};

#[derive(Clone, Debug, PartialEq)]
struct Style {
    face: String,
    size: f64,
    color: String,
    bold: bool,
    italic: bool,
    spacing: f64,
}
#[derive(Clone)]
struct Glyph {
    ch: char,
    advance: f64,
    offset: f64,
    compressed: bool,
    ascent: f64,
    descent: f64,
    leading: f64,
    style: Style,
}
struct Line {
    glyphs: Vec<Glyph>,
    x: f64,
    baseline: f64,
}

fn attr<'a>(nodes: &[Node<'a, '_>], key: &str) -> Option<&'a str> {
    nodes.iter().find_map(|n| n.attribute(key))
}
fn prop_number(nodes: &[Node<'_, '_>], key: &str, default: f64) -> f64 {
    attr(nodes, key)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
fn face(nodes: &[Node<'_, '_>], kind: &str) -> Option<String> {
    nodes.iter().find_map(|n| {
        child(*n, A, kind)
            .and_then(|n| n.attribute("typeface"))
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    })
}
fn style(
    nodes: &[Node<'_, '_>],
    east_asian: bool,
    theme: &Theme,
    warnings: &mut Warnings,
) -> Style {
    let latin = face(nodes, "latin").unwrap_or_else(|| "sans-serif".into());
    let requested = if east_asian {
        face(nodes, "ea").unwrap_or(latin)
    } else {
        latin
    };
    Style {
        face: theme.font(&requested, warnings),
        size: prop_number(nodes, "sz", 1800.0).clamp(100.0, 40000.0) / 75.0,
        color: nodes
            .iter()
            .find_map(|n| child(*n, A, "solidFill"))
            .map(|n| color(n, "#000000", theme, warnings))
            .unwrap_or_else(|| "#000000".into()),
        bold: matches!(attr(nodes, "b"), Some("1" | "true")),
        italic: matches!(attr(nodes, "i"), Some("1" | "true")),
        spacing: prop_number(nodes, "spc", 0.0).clamp(-400000.0, 400000.0) / 75.0,
    }
}
fn is_east_asian(ch: char) -> bool {
    matches!(ch as u32, 0x3000..=0x9fff | 0xf900..=0xfaff | 0xff00..=0xffef | 0x20000..=0x323af)
}
// Impress の 600 DPI 参照デバイスに合わせた計測用サイズ。
// 字形の描画サイズは変更しない。根拠と比較範囲は README のフォント幅計測を参照。
fn measurement_size(css_px: f64) -> f64 {
    let hundredth_mm = (css_px * 2540.0 / 96.0).round();
    (hundredth_mm * 600.0 / 2540.0).round().max(1.0) * 96.0 / 600.0
}

fn glyphs(
    value: &str,
    mut style: Style,
    resolver: &Resolver<'_>,
    warnings: &mut Warnings,
) -> Vec<Glyph> {
    let fonts = resolver.database;
    let requested = style.face.clone();
    let id = fonts
        .faces()
        .find(|f| format!("pptx2img-face-{}", f.id) == style.face)
        .map(|f| f.id)
        .or_else(|| resolver.select(&style.face, value, style.bold, style.italic, warnings));
    if let Some(id) = id {
        style.face = format!("pptx2img-face-{id}");
        if let Some(Some(result)) = fonts.with_face_data(id, |data, index| {
            let font = FontRef::from_index(data, index).ok()?;
            let size = Size::new(style.size as f32);
            let metrics = font.metrics(size, LocationRef::default());
            // Office 系の横書きでは Windows のセルメトリクスを基準にする。
            // OS/2 がない書体は標準の typographic/hhea メトリクスへ戻す。
            let (ascent, descent, leading) = font
                .os2()
                .ok()
                .filter(|t| t.us_win_ascent() > 0)
                .map(|t| {
                    let scale = style.size / f64::from(metrics.units_per_em);
                    (
                        f64::from(t.us_win_ascent()) * scale,
                        f64::from(t.us_win_descent()) * scale,
                        0.0,
                    )
                })
                .unwrap_or((
                    f64::from(metrics.ascent),
                    f64::from(-metrics.descent),
                    f64::from(metrics.leading.max(0.0)),
                ));
            let gm = font.glyph_metrics(Size::unscaled(), LocationRef::default());
            if metrics.units_per_em == 0 {
                return None;
            }
            // サイズ指定の glyph_metrics は固定小数点の丸めを含む。
            // フォント単位で取得した幅を、計測用サイズへ倍精度で換算する。
            let advance_scale = measurement_size(style.size) / f64::from(metrics.units_per_em);
            let map = font.charmap();
            Some(
                value
                    .chars()
                    .map(|ch| {
                        let advance = map.map(ch).and_then(|id| gm.advance_width(id));
                        if advance.is_none() && ch != '\n' {
                            let fallback = resolver.fallback(
                                &requested,
                                ch,
                                style.bold,
                                style.italic,
                                warnings,
                            );
                            if let Some(fallback) = fallback {
                                let mut replacement = style.clone();
                                replacement.face = format!("pptx2img-face-{fallback}");
                                return glyphs(&ch.to_string(), replacement, resolver, warnings)
                                    .remove(0);
                            }
                            warnings.push(
                                Code::MissingGlyph,
                                format!("フォントに文字がなく幅を近似します: U+{:04X}", ch as u32),
                            );
                        }
                        Glyph {
                            ch,
                            offset: 0.0,
                            compressed: false,
                            advance: advance
                                .map(|value| f64::from(value) * advance_scale)
                                .unwrap_or(style.size),
                            ascent,
                            descent,
                            leading,
                            style: style.clone(),
                        }
                    })
                    .collect(),
            )
        }) {
            return result;
        }
    }
    warnings.push(
        Code::Approximation,
        "フォントのメトリクスを取得できないため文字幅を近似します",
    );
    value
        .chars()
        .map(|ch| Glyph {
            ch,
            offset: 0.0,
            compressed: false,
            advance: style.size,
            ascent: style.size,
            descent: style.size * 0.2,
            leading: 0.0,
            style: style.clone(),
        })
        .collect()
}
fn break_allowed(left: char, right: char) -> bool {
    let prohibited_start = "、。，．・：；？！ー）］｝〉》」』】〕〗〙〛’”ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ々ゝゞヽヾ";
    let prohibited_end = "（［｛〈《「『【〔〖〘〚‘“";
    !prohibited_start.contains(right)
        && !prohibited_end.contains(left)
        && !(left.is_ascii_alphanumeric() && right.is_ascii_alphanumeric())
        && !matches!(right as u32, 0x0300..=0x036f | 0x3099..=0x309a | 0xfe00..=0xfe0f)
}
#[cfg(test)]
fn paragraph_hanging(nodes: [Option<Node<'_, '_>>; 3], inherited: bool) -> bool {
    nodes
        .into_iter()
        .flatten()
        .find_map(|n| theme::bool_attr(n, "hangingPunct"))
        .unwrap_or(inherited)
}
fn can_hang(ch: char) -> bool {
    "、。，．".contains(ch)
}
// 字間は同じ文字書式が続く文字間にだけ加える。行末には加えない。
// LibreOffice の描画でも書式境界では字間がリセットされる。
fn step(glyph: &Glyph, next: Option<&Glyph>) -> f64 {
    let spacing = next
        .filter(|next| glyph.style == next.style)
        .map(|_| glyph.style.spacing)
        .unwrap_or(0.0);
    (glyph.advance + spacing).max(0.0)
}
fn text_width(glyphs: &[Glyph]) -> f64 {
    glyphs
        .iter()
        .enumerate()
        .map(|(i, g)| step(g, glyphs.get(i + 1)))
        .sum()
}
fn layout_width(glyphs: &[Glyph], available: f64, hanging: bool) -> f64 {
    let width = text_width(glyphs);
    if hanging
        && width > available
        && let Some(last) = glyphs.last()
        && can_hang(last.ch)
        && text_width(&glyphs[..glyphs.len() - 1]) <= available
    {
        text_width(&glyphs[..glyphs.len() - 1])
    } else {
        width
    }
}
fn baseline_offset(
    paragraph: Option<Node<'_, '_>>,
    ascent: f64,
    natural: f64,
    advance: f64,
) -> f64 {
    let fixed = paragraph
        .and_then(|n| child(n, A, "lnSpc"))
        .and_then(|n| child(n, A, "spcPts"))
        .is_some();
    // 固定行送りはベースライン下の領域を保ち、上側の空きを増減する。
    // 百分率は次行までの距離を変えるが、初行の ascent は変えない。
    if fixed {
        ascent + advance - natural
    } else {
        ascent
    }
}
// LibreOffice 比較で確認したレイアウト単位（1/100 mm）。
// 枠と書式区間は四捨五入。収まり判定の累積幅と文字ごとの圧縮余白は切り捨てる。
const LAYOUT_UNITS_PER_PX: f64 = 2540.0 / 96.0;
fn layout_units(width: f64) -> f64 {
    (width * LAYOUT_UNITS_PER_PX).round()
}
fn fits_line(width: f64, compressed_units: f64, available: f64) -> bool {
    // 境界での収まり判定は累積幅の整数部を使う。文字ごとの切り捨てはしない。
    (width * LAYOUT_UNITS_PER_PX).floor() - compressed_units <= layout_units(available)
}
fn wrap(glyphs: &[Glyph], available: f64, enabled: bool, hanging: bool) -> Vec<Vec<Glyph>> {
    let mut result = Vec::new();
    for hard_line in glyphs.split(|g| g.ch == '\n') {
        if !enabled || hard_line.is_empty() {
            result.push(hard_line.to_vec());
            continue;
        }
        let explicit_spacing = hard_line.iter().any(|g| g.style.spacing != 0.0);
        let mut remaining = hard_line;
        while !remaining.is_empty() {
            let mut sum = 0.0;
            let mut segment_width = 0.0;
            let mut completed_units = 0.0;
            let mut end = 0;
            while end < remaining.len() {
                let candidate = if end == 0 {
                    remaining[end].advance
                } else {
                    sum - remaining[end - 1].advance
                        + step(&remaining[end - 1], Some(&remaining[end]))
                        + remaining[end].advance
                };
                // 明示字間を含む行は、同じ書式の区間ごとに幅を丸める。
                if end > 0 && remaining[end - 1].style != remaining[end].style {
                    completed_units += layout_units(segment_width);
                    segment_width = remaining[end].advance;
                } else {
                    segment_width += candidate - sum;
                }
                let fits = if explicit_spacing {
                    completed_units + layout_units(segment_width) <= layout_units(available)
                } else {
                    fits_line(candidate, 0.0, available)
                };
                if end > 0 && !fits {
                    break;
                }
                sum = candidate;
                end += 1;
            }
            // hangingPunct は句読点を 1 文字だけ枠外に置ける。禁則による追い出しより先に扱う。
            if hanging && end < remaining.len() && can_hang(remaining[end].ch) {
                end += 1;
            }
            if end < remaining.len() {
                let fit = (1..=end)
                    .rev()
                    .find(|&i| break_allowed(remaining[i - 1].ch, remaining[i].ch));
                end = fit.unwrap_or_else(|| {
                    (end..remaining.len())
                        .find(|&i| break_allowed(remaining[i - 1].ch, remaining[i].ch))
                        .unwrap_or(remaining.len())
                });
            }
            result.push(remaining[..end].to_vec());
            remaining = &remaining[end..];
        }
    }
    result
}
// 全角約物の半角分の余白を上限に、行に必要な分だけ縮める。
// 中点・感嘆符・疑問符・仮名はこのモードでは圧縮しない。
fn opening(ch: char) -> bool {
    "「『（［｛〈《【〔".contains(ch)
}
fn capacity(g: &Glyph) -> f64 {
    if (opening(g.ch) || "、。，．」』）］｝〉》】〕：；".contains(g.ch))
        && (g.advance - g.style.size).abs() < g.style.size * 0.05
    {
        g.advance / 2.0
    } else {
        0.0
    }
}
fn compress_line(glyphs: &[Glyph], ratio: f64) -> Vec<Glyph> {
    let leading = glyphs
        .first()
        .filter(|g| opening(g.ch))
        .map(|g| capacity(g) * ratio)
        .unwrap_or(0.0);
    glyphs
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let mut result = g.clone();
            let reduction = capacity(g) * ratio;
            result.advance -= reduction;
            result.compressed = ratio > 0.0;
            // 行頭の開き括弧は字形を左へ突き出さず、後続の位置も同じ量だけ戻す。
            result.offset = if i == 0 {
                0.0
            } else {
                leading - if opening(g.ch) { reduction } else { 0.0 }
            };
            result
        })
        .collect()
}
fn compress_wrap(glyphs: &[Glyph], available: f64, hanging: bool) -> Vec<Vec<Glyph>> {
    let mut result = Vec::new();
    for hard_line in glyphs.split(|g| g.ch == '\n') {
        if hard_line.is_empty() {
            result.push(vec![]);
            continue;
        }
        let mut remaining = hard_line;
        while !remaining.is_empty() {
            let mut natural = 0.0;
            let mut compressed_units = 0.0;
            let mut end = 0;
            while end < remaining.len() {
                let next = natural + remaining[end].advance;
                let next_compressed =
                    compressed_units + (capacity(&remaining[end]) * LAYOUT_UNITS_PER_PX).floor();
                if end > 0 && !fits_line(next, next_compressed, available) {
                    break;
                }
                natural = next;
                compressed_units = next_compressed;
                end += 1;
            }
            if hanging && end < remaining.len() && can_hang(remaining[end].ch) {
                end += 1;
            }
            let candidate = end;
            if end < remaining.len() {
                end = (1..=end)
                    .rev()
                    .find(|&i| break_allowed(remaining[i - 1].ch, remaining[i].ch))
                    .unwrap_or_else(|| {
                        (end..remaining.len())
                            .find(|&i| break_allowed(remaining[i - 1].ch, remaining[i].ch))
                            .unwrap_or(remaining.len())
                    });
            }
            let line = &remaining[..end];
            let total: f64 = line.iter().map(capacity).sum();
            let width = layout_width(line, available, hanging);
            // 折り返した行が自然幅へ戻る整数境界では、圧縮済みの字形位置が残る。
            // 境界の 1 単位（1/100 mm）はこの保持判定にだけ適用する。
            let ratio = if total == 0.0 {
                0.0
            } else if end < candidate
                || (end < remaining.len() && layout_units(width) <= layout_units(available) + 1.0)
            {
                1.0
            } else {
                ((width - available) / total).clamp(0.0, 1.0)
            };
            result.push(compress_line(line, ratio));
            remaining = &remaining[end..];
        }
    }
    result
}
fn spacing(node: Option<Node<'_, '_>>, tag: &str, natural: f64, default: f64) -> f64 {
    let Some(value) = node.and_then(|n| child(n, A, tag)) else {
        return default;
    };
    if let Some(points) = child(value, A, "spcPts") {
        return number(points, "val", 0.0) / 75.0;
    }
    child(value, A, "spcPct")
        .map(|n| natural * number(n, "val", 100000.0) / 100000.0)
        .unwrap_or(default)
}
pub(super) struct TextBody<'a> {
    pub body: Node<'a, 'a>,
    pub inherited: Vec<Node<'a, 'a>>,
    pub master_style: Option<Node<'a, 'a>>,
    pub cell_props: Option<Node<'a, 'a>>,
}
impl<'a> From<Node<'a, 'a>> for TextBody<'a> {
    fn from(body: Node<'a, 'a>) -> Self {
        Self {
            body,
            inherited: vec![],
            master_style: None,
            cell_props: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TextExtent {
    pub height: f64,
    pub offset_y: f64,
}
#[derive(Clone, Copy, Debug)]
struct AutoFit {
    scale: f64,
    reduction: f64,
    expand: bool,
}
fn auto_fit(nodes: &[Node<'_, '_>], warnings: &mut Warnings) -> AutoFit {
    let mode = nodes.iter().find_map(|n| {
        n.children().find(|c| {
            c.tag_name().namespace() == Some(A)
                && matches!(
                    c.tag_name().name(),
                    "noAutofit" | "normAutofit" | "spAutoFit"
                )
        })
    });
    let mut result = AutoFit {
        scale: 1.0,
        reduction: 0.0,
        expand: false,
    };
    if let Some(mode) = mode {
        match mode.tag_name().name() {
            "normAutofit" => {
                let percentage =
                    |key: &str, default: f64, maximum: f64, warnings: &mut Warnings| {
                        let Some(raw) = mode.attribute(key) else {
                            return default;
                        };
                        let value = if let Some(percent) = raw.strip_suffix('%') {
                            percent.parse::<f64>().ok().map(|n| n / 100.0)
                        } else {
                            raw.parse::<f64>().ok().map(|n| n / 100000.0)
                        };
                        match value.filter(|n| {
                            n.is_finite()
                                && *n >= 0.0
                                && *n <= maximum
                                && (key != "fontScale" || *n > 0.0)
                        }) {
                            Some(value) => value,
                            None => {
                                warn(
                                    warnings,
                                    format!("不正な自動調整値 {key}={raw} を既定値で近似します"),
                                );
                                default
                            }
                        }
                    };
                result.scale = percentage("fontScale", 1.0, 1.0, warnings);
                result.reduction = percentage("lnSpcReduction", 0.0, 1.0, warnings);
            }
            "spAutoFit" => result.expand = true,
            _ => {}
        }
    }
    result
}

pub(super) fn render<'a>(
    source: impl Into<TextBody<'a>>,
    w: f64,
    h: f64,
    theme: &Theme,
    options: &RenderOptions<'_>,
    svg: &mut String,
    warnings: &mut Warnings,
) -> TextExtent {
    let source = source.into();
    let body = source.body;
    let bodies: Vec<_> = std::iter::once(body)
        .chain(source.inherited.iter().copied())
        .collect();
    warnings.push(
        Code::Approximation,
        "文字組みは近似です。複雑な shaping・完全な Office 互換は未対応です",
    );
    if options.punctuation_compression {
        warn(
            warnings,
            "自動句読点圧縮は実験的な描画方針です。PPTX 内の指定ではありません",
        );
    }
    let resolver = &options.fonts;
    let body_props: Vec<_> = bodies
        .iter()
        .filter_map(|n| child(*n, A, "bodyPr"))
        .collect();
    if attr(&body_props, "vert").is_some_and(|s| s != "horz") {
        warn(warnings, "縦書きは未対応です");
    }
    let autofit = auto_fit(&body_props, warnings);
    let text_style = |nodes: &[Node<'_, '_>], east: bool, warnings: &mut Warnings| {
        let mut style = style(nodes, east, theme, warnings);
        style.size *= autofit.scale;
        style
    };
    let margin = |key, cell_key, default| {
        source
            .cell_props
            .and_then(|n| n.attribute(cell_key))
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite())
            .unwrap_or_else(|| prop_number(&body_props, key, default))
            / EMU
    };
    let left = margin("lIns", "marL", 91440.0);
    let right = margin("rIns", "marR", 91440.0);
    let top = margin("tIns", "marT", 45720.0);
    let bottom = margin("bIns", "marB", 45720.0);
    let wrap_enabled = attr(&body_props, "wrap") != Some("none");
    let available = (w - left - right).max(1.0);
    let placeholder_type = bodies
        .iter()
        .filter_map(|n| n.parent())
        .filter_map(layout::placeholder)
        .find_map(|n| n.attribute("type"));
    let style_kind = match placeholder_type {
        Some("title" | "ctrTitle") => "titleStyle",
        Some("body" | "obj" | "subTitle") => "bodyStyle",
        _ => "otherStyle",
    };
    // 通常のテキストボックスはプレゼン既定を使用し、プレースホルダーの
    // マスター書式を流用しない。図形内文字は従来のotherStyleを維持する。
    let textbox = body
        .parent()
        .and_then(|n| child(n, P, "nvSpPr"))
        .and_then(|n| child(n, P, "cNvSpPr"))
        .and_then(|n| theme::bool_attr(n, "txBox"))
        == Some(true);
    let presentation = theme
        .presentation
        .as_deref()
        .and_then(|s| Document::parse(s).ok());
    let presentation_style = presentation
        .as_ref()
        .and_then(|doc| child(doc.root_element(), P, "defaultTextStyle"));
    let mut lines = Vec::new();
    let mut cursor = 0.0;
    for paragraph in body.children().filter(|n| n.has_tag_name((A, "p"))) {
        let pp = child(paragraph, A, "pPr");
        let level = pp
            .map(|n| number(n, "lvl", 0.0) as usize)
            .unwrap_or(0)
            .min(8);
        let level_tag = format!("lvl{}pPr", level + 1);
        let mut paragraph_props: Vec<_> = pp.into_iter().collect();
        for (i, body) in bodies.iter().enumerate() {
            if i > 0
                && let Some(inherited) = child(*body, A, "p").and_then(|p| child(p, A, "pPr"))
            {
                paragraph_props.push(inherited);
            }
            if let Some(list) = child(*body, A, "lstStyle") {
                paragraph_props.extend(child(list, A, &level_tag));
                paragraph_props.extend(child(list, A, "defPPr"));
            }
        }
        if !textbox && let Some(style) = source.master_style.and_then(|n| child(n, P, style_kind)) {
            paragraph_props.extend(child(style, A, &level_tag));
            paragraph_props.extend(child(style, A, "defPPr"));
        }
        if let Some(style) = presentation_style {
            paragraph_props.extend(child(style, A, &level_tag));
            paragraph_props.extend(child(style, A, "defPPr"));
        }
        let hanging = paragraph_props
            .iter()
            .find_map(|n| theme::bool_attr(*n, "hangingPunct"))
            .unwrap_or_else(|| theme.hanging_punctuation(style_kind, level));
        let defaults: Vec<_> = paragraph_props
            .iter()
            .filter_map(|n| child(*n, A, "defRPr"))
            .collect();
        let spacing_node = |tag| {
            paragraph_props
                .iter()
                .copied()
                .find(|n| child(*n, A, tag).is_some())
        };
        let mut all = Vec::new();
        for run in paragraph.children().filter(|n| n.is_element()) {
            if run.has_tag_name((A, "br")) {
                all.extend(glyphs(
                    "\n",
                    text_style(
                        &child(run, A, "rPr")
                            .into_iter()
                            .chain(defaults.iter().copied())
                            .collect::<Vec<_>>(),
                        false,
                        warnings,
                    ),
                    resolver,
                    warnings,
                ));
            } else if run.has_tag_name((A, "r")) || run.has_tag_name((A, "fld")) {
                let nodes: Vec<_> = child(run, A, "rPr")
                    .into_iter()
                    .chain(defaults.iter().copied())
                    .collect();
                let value = child(run, A, "t").and_then(|n| n.text()).unwrap_or("");
                // 欧文と東アジア文字では別のフォント指定を解決する。
                let mut segment = String::new();
                let mut last = None;
                for ch in value.chars() {
                    let east = is_east_asian(ch);
                    if last.is_some_and(|v| v != east) {
                        all.extend(glyphs(
                            &segment,
                            text_style(&nodes, last.unwrap(), warnings),
                            resolver,
                            warnings,
                        ));
                        segment.clear();
                    }
                    segment.push(ch);
                    last = Some(east);
                }
                if !segment.is_empty() {
                    all.extend(glyphs(
                        &segment,
                        text_style(&nodes, last.unwrap_or(false), warnings),
                        resolver,
                        warnings,
                    ));
                }
            }
        }
        if paragraph_props.iter().any(|n| {
            n.children()
                .any(|n| matches!(n.tag_name().name(), "buChar" | "buAutoNum"))
                || n.attribute("marL").is_some()
                || n.attribute("indent").is_some()
        }) {
            warn(warnings, "箇条書き・段落インデントは未対応です");
        }
        if all.windows(2).any(|pair| {
            pair[0].style == pair[1].style && pair[0].advance + pair[0].style.spacing < 0.0
        }) {
            warn(
                warnings,
                "文字幅を超える負の字間は未対応のため送り幅を 0 に制限します",
            );
        }
        let end_defaults: Vec<_> = child(paragraph, A, "endParaRPr")
            .into_iter()
            .chain(defaults.iter().copied())
            .collect();
        let empty_style = text_style(&end_defaults, false, warnings);
        let has_empty_line = all.is_empty()
            || all.first().is_some_and(|g| g.ch == '\n')
            || all.last().is_some_and(|g| g.ch == '\n')
            || all.windows(2).any(|p| p[0].ch == '\n' && p[1].ch == '\n');
        let empty = if has_empty_line {
            glyphs(" ", empty_style, resolver, warnings)
        } else {
            Vec::new()
        };
        let empty_ascent = empty.first().map(|g| g.ascent).unwrap_or(0.0);
        let empty_height = empty
            .first()
            .map(|g| g.ascent + g.descent + g.leading)
            .unwrap_or(0.0);
        let natural = all
            .iter()
            .map(|g| g.ascent + g.descent + g.leading)
            .fold(0.0, f64::max)
            .max(if all.is_empty() { empty_height } else { 0.0 });
        cursor += spacing(spacing_node("spcBef"), "spcBef", natural, 0.0);
        let compressed = options.punctuation_compression && wrap_enabled;
        if compressed && all.iter().any(|g| g.style.spacing != 0.0) {
            warn(
                warnings,
                "自動句読点圧縮と明示字間の併用は未対応のため明示字間を優先します",
            );
        }
        let wrapped = if compressed && all.iter().all(|g| g.style.spacing == 0.0) {
            compress_wrap(&all, available, hanging)
        } else {
            wrap(&all, available, wrap_enabled, hanging)
        };
        for glyph_line in wrapped {
            let width = layout_width(&glyph_line, available, hanging);
            if width > available + 0.01 {
                warnings.push(Code::TextOverflow, "文字がテキスト枠の横幅を超えています");
            }
            let ascent = if glyph_line.is_empty() {
                empty_ascent
            } else {
                glyph_line.iter().map(|g| g.ascent).fold(0.0, f64::max)
            };
            let descent = glyph_line.iter().map(|g| g.descent).fold(0.0, f64::max);
            let leading = glyph_line.iter().map(|g| g.leading).fold(0.0, f64::max);
            let line_height = if glyph_line.is_empty() {
                empty_height
            } else {
                ascent + descent + leading
            };
            let fixed = spacing_node("lnSpc")
                .and_then(|n| child(n, A, "lnSpc"))
                .and_then(|n| child(n, A, "spcPts"))
                .is_some();
            let advance = (spacing(spacing_node("lnSpc"), "lnSpc", line_height, line_height)
                - if fixed {
                    0.0
                } else {
                    line_height * autofit.reduction
                })
            .max(0.0);
            let x = match attr(&paragraph_props, "algn") {
                Some("ctr") => left + (available - width) / 2.0,
                Some("r") => left + available - width,
                _ => left,
            };
            lines.push(Line {
                glyphs: glyph_line,
                x,
                baseline: cursor
                    + baseline_offset(spacing_node("lnSpc"), ascent, line_height, advance),
            });
            cursor += advance;
        }
        cursor += spacing(spacing_node("spcAft"), "spcAft", natural, 0.0);
    }
    let content_height = lines.iter().fold(cursor, |bottom, line| {
        bottom.max(line.baseline + line.glyphs.iter().map(|g| g.descent).fold(0.0, f64::max))
    });
    let anchor = source
        .cell_props
        .and_then(|n| n.attribute("anchor"))
        .or_else(|| attr(&body_props, "anchor"));
    let can_expand = autofit.expand
        && source.cell_props.is_none()
        && attr(&body_props, "vert").is_none_or(|s| s == "horz");
    if autofit.expand && !can_expand {
        warn(warnings, "表セル・縦書きの自動枠拡張は未対応です");
    }
    if autofit.expand && !wrap_enabled {
        warn(
            warnings,
            "折り返しなしの横方向自動枠拡張は未対応です。高さだけ拡張します",
        );
    }
    let expanded_height = if can_expand {
        h.max(top + content_height + bottom)
    } else {
        h
    };
    let extent = TextExtent {
        height: expanded_height,
        offset_y: -(expanded_height - h)
            * match anchor {
                Some("ctr") => 0.5,
                Some("b") => 1.0,
                _ => 0.0,
            },
    };
    let h = expanded_height;
    if content_height > h - top - bottom + 0.01 {
        warnings.push(Code::TextOverflow, "文字がテキスト枠の高さを超えています");
    }
    let vertical = match anchor {
        Some("ctr") => top + (h - top - bottom - content_height) / 2.0,
        Some("b") => h - bottom - content_height,
        Some("t") | None => top,
        _ => {
            warn(warnings, "均等割付の縦配置は未対応です");
            top
        }
    };
    for line in lines {
        let mut x = line.x;
        let y = line.baseline + vertical;
        svg.push_str(&format!("<text y=\"{y}\" xml:space=\"preserve\">"));

        let mut start = 0;
        while start < line.glyphs.len() {
            let style = &line.glyphs[start].style;
            let mut end = start + 1;
            while end < line.glyphs.len() && line.glyphs[end].style == *style {
                end += 1;
            }
            let positions = {
                let mut position = x;
                line.glyphs[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, g)| {
                        let result = (position + g.offset).to_string();
                        position += step(
                            g,
                            line.glyphs
                                .get(start + i + 1)
                                .filter(|_| start + i + 1 < end),
                        );
                        result
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let value: String = line.glyphs[start..end].iter().map(|g| g.ch).collect();
            svg.push_str(&format!("<tspan x=\"{positions}\" font-family=\"{}\" font-size=\"{}\" fill=\"{}\" font-weight=\"{}\" font-style=\"{}\">{}</tspan>", escape(&style.face), style.size, style.color, if style.bold {"bold"} else {"normal"}, if style.italic {"italic"} else {"normal"}, escape(&value)));
            x += text_width(&line.glyphs[start..end]);
            start = end;
        }
        svg.push_str("</text>");
    }
    extent
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(value: &str) -> Vec<Glyph> {
        value
            .chars()
            .map(|ch| Glyph {
                ch,
                offset: 0.0,
                compressed: false,
                advance: 10.0,
                ascent: 8.0,
                descent: 2.0,
                leading: 0.0,
                style: Style {
                    face: "sans-serif".into(),
                    size: 10.0,
                    color: "#000000".into(),
                    bold: false,
                    italic: false,
                    spacing: 0.0,
                },
            })
            .collect()
    }
    fn strings(lines: Vec<Vec<Glyph>>) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.iter().map(|g| g.ch).collect())
            .collect()
    }
    #[test]
    fn twelve_point_boundary_uses_cumulative_width_before_kinsoku() {
        for text in ["あいうえお", "あいうえおか", "あ、いう、", "あ、いう、え"]
        {
            let mut glyphs = sample(text);
            for glyph in &mut glyphs {
                glyph.advance = 16.0;
                glyph.style.size = 16.0;
            }
            let below = strings(wrap(&glyphs, 59.95 * 4.0 / 3.0, true, false));
            assert!(below.len() > 1);
            for width in [59.98, 59.99, 60.0] {
                let lines = strings(wrap(&glyphs, width * 4.0 / 3.0, true, false));
                assert_eq!(lines[0], text.chars().take(5).collect::<String>());
                assert_eq!(lines.concat(), text);
            }
        }
        // 各文字を切り捨ててから合計すると狭すぎる枠にも収まってしまう。
        let mut glyphs = sample("あいうえお");
        for glyph in &mut glyphs {
            glyph.advance = 16.0;
            glyph.style.size = 16.0;
        }
        assert_eq!(
            strings(wrap(&glyphs, 59.95 * 4.0 / 3.0, true, false)),
            ["あいうえ", "お"]
        );
    }
    #[test]
    fn rounded_line_width_changes_only_at_layout_unit_boundary() {
        let mut glyphs = sample("あいうえおか");
        for glyph in &mut glyphs {
            glyph.advance = 32.0;
            glyph.style.size = 32.0;
        }
        assert_eq!(
            strings(wrap(&glyphs, 143.98 * 4.0 / 3.0, true, false)),
            ["あいうえお", "か"]
        );
        assert_eq!(
            strings(wrap(&glyphs, 143.99 * 4.0 / 3.0, true, false)),
            ["あいうえおか"]
        );
    }
    #[test]
    fn explicit_spacing_rounds_style_segments_before_adding_widths() {
        let mut glyphs = sample("あいう");
        for (index, glyph) in glyphs.iter_mut().enumerate() {
            glyph.advance = 32.0;
            glyph.style.size = 32.0;
            glyph.style.spacing = index as f64;
        }
        // 区間はすべて別書式なので字間は加わらないが、各区間の丸めは蓄積する。
        assert_eq!(strings(wrap(&glyphs, 96.0, true, false)), ["あい", "う"]);
        assert_eq!(strings(wrap(&glyphs, 96.04, true, false)), ["あいう"]);
    }
    #[test]
    fn font_units_preserve_proportions_at_fractional_sizes() {
        let mut fonts = Database::new();
        fonts.load_font_data(include_bytes!("../../tests/fixtures/metrics.ttf").to_vec());
        for (size_pt, full) in [
            (12.0, 16.0),
            (18.0, 24.0),
            (20.0, 26.72),
            (21.5, 28.64),
            (24.0, 32.0),
            (32.0, 42.72),
        ] {
            let mut style = sample("あ")[0].style.clone();
            style.face = "Pptx2img Metrics Test".into();
            style.size = size_pt * 4.0 / 3.0;
            let mut warnings = Warnings::default();
            let measured = glyphs(
                "あA \u{301}",
                style.clone(),
                &Resolver::new(&fonts, &HashMap::new()),
                &mut warnings,
            );
            assert!(warnings.is_empty(), "{warnings:?}");
            for (glyph, expected) in measured.iter().zip([full, full / 2.0, full / 4.0, 0.0]) {
                assert!((glyph.advance - expected).abs() < 1e-10);
                assert_eq!(glyph.style.size, style.size);
            }
            let missing = glyphs(
                "未",
                style.clone(),
                &Resolver::new(&fonts, &HashMap::new()),
                &mut warnings,
            );
            assert_eq!(missing[0].advance, style.size);
            assert!(warnings.iter().any(|w| w.contains("U+672A")));
        }
    }
    #[test]
    fn compression_capacity_is_truncated_per_character() {
        for (size_pt, below, fits) in [(24.0, 120.0, 120.01), (18.0, 90.01, 90.02)] {
            let mut glyphs = sample("あ、いう、え");
            for glyph in &mut glyphs {
                glyph.advance = size_pt * 4.0 / 3.0;
                glyph.style.size = glyph.advance;
            }
            assert_eq!(
                strings(compress_wrap(&glyphs, below * 4.0 / 3.0, false)).len(),
                2
            );
            assert_eq!(
                strings(compress_wrap(&glyphs, fits * 4.0 / 3.0, false)),
                ["あ、いう、え"]
            );
        }
    }
    #[test]
    fn wrapped_line_keeps_maximum_compression_at_expansion_boundary() {
        let mut glyphs = sample("あ、いう、え");
        for glyph in &mut glyphs {
            glyph.advance = 32.0;
            glyph.style.size = 32.0;
        }
        let before = compress_wrap(&glyphs, 119.9 * 4.0 / 3.0, false);
        assert_eq!(strings(before.clone()), ["あ、いう、", "え"]);
        assert!(before[0][1].advance > 31.9);
        for width in [119.95, 119.98, 120.0] {
            let lines = compress_wrap(&glyphs, width * 4.0 / 3.0, false);
            assert_eq!(strings(lines.clone()), ["あ、いう、", "え"]);
            assert_eq!(lines[0][1].advance, 16.0);
            assert_eq!(lines[0][4].advance, 16.0);
            assert_eq!(lines[1][0].advance, 32.0);
        }
        assert_eq!(compress_wrap(&glyphs, 120.01 * 4.0 / 3.0, false).len(), 1);
        // 残りの文字がない行では、枠に必要な分だけ縮める。
        let last = compress_wrap(&glyphs[..5], 119.95 * 4.0 / 3.0, false);
        assert!(last[0][1].advance > 31.9);
    }
    #[test]
    fn automatic_compression_uses_only_the_required_punctuation_space() {
        let line = sample("あ、いう、え");
        let result = compress_wrap(&line, 55.0, false);
        assert_eq!(strings(result.clone()), ["あ、いう、え"]);
        assert_eq!(text_width(&result[0]), 55.0);
        assert_eq!(result[0][1].advance, 7.5);
        assert_eq!(result[0][4].advance, 7.5);
        assert_eq!(result[0][0].advance, 10.0);
        assert_eq!(
            strings(wrap(&line, 55.0, true, false)),
            ["あ、いう、", "え"]
        );
    }
    #[test]
    fn spacious_lines_and_uncompressible_characters_are_unchanged() {
        let line = sample("あ、いう。え");
        let result = compress_wrap(&line, 80.0, false);
        assert_eq!(text_width(&result[0]), 60.0);
        assert!(result[0].iter().all(|g| !g.compressed && g.offset == 0.0));
        let line = sample("あいうえお・？！");
        assert!(line.iter().all(|g| capacity(g) == 0.0));
        assert_eq!(
            strings(compress_wrap(&line, 35.0, false)),
            strings(wrap(&line, 35.0, true, false))
        );
    }
    #[test]
    fn compression_has_a_half_width_limit_and_keeps_explicit_breaks() {
        let line = sample("あ、いう、え");
        let result = compress_wrap(&line, 45.0, false);
        assert!(
            result
                .iter()
                .all(|line| line.iter().all(|g| g.advance >= 5.0))
        );
        assert_eq!(
            strings(compress_wrap(&sample("あ、\n\nい。"), 30.0, false)),
            ["あ、", "", "い。"]
        );
        let mut narrow = sample("、");
        narrow[0].advance = 5.0;
        assert_eq!(capacity(&narrow[0]), 0.0);
    }
    #[test]
    fn opening_bracket_removes_leading_space_without_scaling_the_glyph() {
        let line = compress_line(&sample("あ「い」"), 0.5);
        assert_eq!(line[1].offset, -2.5);
        assert_eq!(line[1].advance, 7.5);
        assert_eq!(line[1].style.size, 10.0);
        let leading = compress_line(&sample("「あ」"), 0.5);
        assert_eq!(leading[0].offset, 0.0);
    }
    #[test]
    fn compressed_positions_reach_svg_and_explicit_spacing_takes_priority() {
        for (spc, wrap_mode, expected, warn_spacing) in [
            ("0", "square", "0 10 17.5 27.5 37.5 45", false),
            ("-75", "square", "0 9 18 27 36 45", true),
            ("0", "none", "0 10 20 30 40 50", false),
        ] {
            let source = format!(
                r#"<a:txBody xmlns:a="{A}"><a:bodyPr lIns="0" rIns="0" wrap="{wrap_mode}"/><a:p><a:r><a:rPr sz="750" spc="{spc}"/><a:t>あ、いう、え</a:t></a:r></a:p></a:txBody>"#
            );
            let doc = Document::parse(&source).unwrap();
            let mut svg = String::new();
            let mut warnings = Warnings::default();
            render(
                doc.root_element(),
                55.0,
                100.0,
                &Theme::default(),
                &RenderOptions {
                    fonts: Resolver::new(&Database::new(), &HashMap::new()),
                    punctuation_compression: true,
                },
                &mut svg,
                &mut warnings,
            );
            assert!(svg.contains(&format!("<tspan x=\"{expected}\"")), "{svg}");
            assert_eq!(
                warnings.iter().any(|s| s.contains("明示字間を優先")),
                warn_spacing
            );
            assert!(warnings.iter().any(|s| s.contains("実験的な描画方針")));
        }
    }
    #[test]
    fn signed_spacing_changes_width_and_wrap_without_trailing_space() {
        let mut glyphs = sample("あいうえ");
        for g in &mut glyphs {
            g.style.spacing = -5.0;
        }
        assert_eq!(text_width(&glyphs), 25.0);
        assert_eq!(strings(wrap(&glyphs, 20.0, true, false)), ["あいう", "え"]);
        for g in &mut glyphs {
            g.style.spacing = 2.0;
        }
        assert_eq!(text_width(&glyphs), 46.0);
        assert_eq!(strings(wrap(&glyphs, 22.0, true, false)), ["あい", "うえ"]);
        assert_eq!(text_width(&glyphs[..1]), 10.0);
    }
    #[test]
    fn spacing_stops_at_style_boundary_and_explicit_break() {
        let mut glyphs = sample("あ、、い");
        glyphs[1].style.spacing = -5.0;
        glyphs[2].style.spacing = -5.0;
        assert_eq!(text_width(&glyphs), 35.0);
        glyphs[2].style.color = "#ff0000".into();
        assert_eq!(text_width(&glyphs), 40.0);
        let mut glyphs = sample("あい\nうえ");
        for g in &mut glyphs {
            g.style.spacing = -5.0;
        }
        let lines = wrap(&glyphs, 100.0, true, false);
        assert_eq!(strings(lines.clone()), ["あい", "うえ"]);
        assert_eq!(text_width(&lines[0]), 15.0);
        assert_eq!(text_width(&lines[1]), 15.0);
    }
    #[test]
    fn hanging_uses_compressed_width() {
        let mut glyphs = sample("あいう、え");
        for g in &mut glyphs {
            g.style.spacing = -5.0;
        }
        let lines = wrap(&glyphs, 20.0, true, true);
        assert_eq!(strings(lines.clone()), ["あいう、", "え"]);
        assert_eq!(layout_width(&lines[0], 20.0, true), 20.0);
    }
    #[test]
    fn spacing_is_inherited_and_zero_cancels_it() {
        let source = format!(
            r#"<a:p xmlns:a="{A}"><a:defRPr spc="-600"/><a:rPr spc="0"/><a:rPr spc="300"/></a:p>"#
        );
        let doc = Document::parse(&source).unwrap();
        let nodes: Vec<_> = doc
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .collect();
        let resolve = |nodes: &[Node<'_, '_>]| {
            style(nodes, true, &Theme::default(), &mut Warnings::default()).spacing
        };
        assert_eq!(resolve(&[nodes[0]]), -8.0);
        assert_eq!(resolve(&[nodes[1], nodes[0]]), 0.0);
        assert_eq!(resolve(&[nodes[2], nodes[0]]), 4.0);
        assert_eq!(resolve(&[]), 0.0);
    }
    #[test]
    fn svg_positions_use_spacing_and_alignment() {
        // フォント未登録時の決定的な幅 10px を用い、SVG 出力まで検証する。
        for (align, expected) in [
            ("l", "0 5 10 15"),
            ("ctr", "12.5 17.5 22.5 27.5"),
            ("r", "25 30 35 40"),
        ] {
            let source = format!(
                r#"<a:txBody xmlns:a="{A}"><a:bodyPr lIns="0" rIns="0"/><a:p><a:pPr algn="{align}"/><a:r><a:rPr sz="750" spc="-375"/><a:t>あ、、い</a:t></a:r></a:p></a:txBody>"#
            );
            let doc = Document::parse(&source).unwrap();
            let mut svg = String::new();
            render(
                doc.root_element(),
                50.0,
                100.0,
                &Theme::default(),
                &RenderOptions {
                    fonts: Resolver::new(&Database::new(), &HashMap::new()),
                    punctuation_compression: false,
                },
                &mut svg,
                &mut Warnings::default(),
            );
            assert!(svg.contains(&format!("<tspan x=\"{expected}\"")), "{svg}");
        }
    }
    #[test]
    fn excessive_negative_spacing_warns_instead_of_reversing_text() {
        let source = format!(
            r#"<a:txBody xmlns:a="{A}"><a:p><a:r><a:rPr sz="750" spc="-1500"/><a:t>あい</a:t></a:r></a:p></a:txBody>"#
        );
        let doc = Document::parse(&source).unwrap();
        let mut warnings = Warnings::default();
        render(
            doc.root_element(),
            50.0,
            100.0,
            &Theme::default(),
            &RenderOptions {
                fonts: Resolver::new(&Database::new(), &HashMap::new()),
                punctuation_compression: false,
            },
            &mut String::new(),
            &mut warnings,
        );
        assert!(
            warnings
                .iter()
                .any(|s| s.contains("文字幅を超える負の字間"))
        );
    }
    #[test]
    fn closing_punctuation_does_not_begin_line() {
        assert_eq!(
            strings(wrap(&sample("あいう、え"), 30.0, true, false)),
            ["あい", "う、え"]
        );
    }
    #[test]
    fn opening_punctuation_does_not_end_line() {
        assert_eq!(
            strings(wrap(&sample("あい「うえ」"), 30.0, true, false)),
            ["あい", "「う", "え」"]
        );
    }
    #[test]
    fn explicit_break_and_empty_line_are_preserved() {
        assert_eq!(
            strings(wrap(&sample("あ\n\nい"), 100.0, true, false)),
            ["あ", "", "い"]
        );
    }
    #[test]
    fn no_wrap_keeps_long_line() {
        assert_eq!(
            strings(wrap(&sample("あいうえ"), 10.0, false, false)),
            ["あいうえ"]
        );
    }
    #[test]
    fn enabled_hanging_keeps_comma_with_previous_line() {
        let lines = wrap(&sample("あいう、え"), 30.0, true, true);
        assert_eq!(strings(lines.clone()), ["あいう、", "え"]);
        assert_eq!(layout_width(&lines[0], 30.0, true), 30.0);
        assert_eq!(layout_width(&lines[0], 30.0, false), 40.0);
    }
    #[test]
    fn hanging_does_not_allow_arbitrary_letters_to_overflow() {
        assert_eq!(
            strings(wrap(&sample("あいうえお"), 30.0, true, true)),
            ["あいう", "えお"]
        );
        assert_eq!(
            strings(wrap(&sample("あいう、、え"), 30.0, true, true)),
            ["あい", "う、、", "え"]
        );
    }
    #[test]
    fn explicit_false_overrides_inherited_hanging() {
        let source = format!(
            r#"<a:p xmlns:a="{A}"><a:pPr hangingPunct="0"/><a:lvl1pPr hangingPunct="true"/><a:defPPr hangingPunct="0"/></a:p>"#
        );
        let doc = Document::parse(&source).unwrap();
        let root = doc.root_element();
        let pp = child(root, A, "pPr");
        let level = child(root, A, "lvl1pPr");
        let default = child(root, A, "defPPr");
        assert!(!paragraph_hanging([pp, level, default], true));
        assert!(paragraph_hanging([None, level, default], false));
        assert!(!paragraph_hanging([None, None, default], true));
        assert!(paragraph_hanging([None, None, None], true));
    }
    #[test]
    fn fixed_spacing_changes_first_baseline_but_percentage_does_not() {
        let source = format!(
            r#"<a:p xmlns:a="{A}"><a:pPr><a:lnSpc><a:spcPts val="1800"/></a:lnSpc></a:pPr><a:pPr><a:lnSpc><a:spcPct val="200000"/></a:lnSpc></a:pPr></a:p>"#
        );
        let doc = Document::parse(&source).unwrap();
        let props: Vec<_> = doc
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .collect();
        // ascent 20 / descent 5 のセルを 24px の固定行送りに置く。下端 5px を保持。
        let fixed = spacing(Some(props[0]), "lnSpc", 25.0, 25.0);
        assert_eq!(fixed, 24.0);
        assert_eq!(baseline_offset(Some(props[0]), 20.0, 25.0, fixed), 19.0);
        let percentage = spacing(Some(props[1]), "lnSpc", 25.0, 25.0);
        assert_eq!(percentage, 50.0);
        assert_eq!(
            baseline_offset(Some(props[1]), 20.0, 25.0, percentage),
            20.0
        );
        assert_eq!(baseline_offset(None, 20.0, 25.0, 25.0), 20.0);
    }
    #[test]
    fn inherited_run_properties_are_merged_per_attribute() {
        let source = format!(
            r#"<a:p xmlns:a="{A}"><a:pPr><a:defRPr sz="1200" b="1"><a:latin typeface="Example"/></a:defRPr></a:pPr><a:r><a:rPr b="0"/></a:r></a:p>"#
        );
        let doc = Document::parse(&source).unwrap();
        let defaults = doc
            .descendants()
            .find(|n| n.has_tag_name((A, "defRPr")))
            .unwrap();
        let run = doc
            .descendants()
            .find(|n| n.has_tag_name((A, "rPr")))
            .unwrap();
        let s = style(
            &[run, defaults],
            false,
            &Theme::default(),
            &mut Warnings::default(),
        );
        assert_eq!(s.size, 16.0);
        assert!(!s.bold);
        assert_eq!(s.face, "Example");
    }
    fn inherited_fixture(paragraphs: &str, size: &str) -> (String, Warnings) {
        let source = format!(
            r#"<p:sp xmlns:p="{P}" xmlns:a="{A}"><p:nvSpPr><p:cNvSpPr txBox="1"/></p:nvSpPr><p:txBody><a:bodyPr lIns="0" rIns="0" tIns="0" bIns="0"/><a:lstStyle/>{paragraphs}</p:txBody><p:txStyles><p:otherStyle><a:lvl1pPr><a:defRPr sz="2400"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sp>"#
        );
        let doc = Document::parse(&source).unwrap();
        let mut theme = Theme::default();
        theme.presentation = Some(format!(
            r#"<p:presentation xmlns:p="{P}" xmlns:a="{A}"><p:defaultTextStyle><a:lvl1pPr><a:defRPr sz="{size}"><a:latin typeface="Pptx2img Metrics Test"/></a:defRPr></a:lvl1pPr></p:defaultTextStyle></p:presentation>"#
        ));
        let mut fonts = Database::new();
        fonts.load_font_data(include_bytes!("../../tests/fixtures/metrics.ttf").to_vec());
        let mut svg = String::new();
        let mut warnings = Warnings::default();
        render(
            TextBody {
                body: child(doc.root_element(), P, "txBody").unwrap(),
                inherited: vec![],
                master_style: child(doc.root_element(), P, "txStyles"),
                cell_props: None,
            },
            100.0,
            90.0,
            &theme,
            &RenderOptions {
                fonts: Resolver::new(&fonts, &HashMap::new()),
                punctuation_compression: false,
            },
            &mut svg,
            &mut warnings,
        );
        (svg, warnings)
    }
    #[test]
    fn textbox_uses_presentation_defaults_and_matches_explicit_runs() {
        let inherited =
            inherited_fixture(r#"<a:p><a:r><a:t>AAAA AAAA AAAA</a:t></a:r></a:p>"#, "900").0;
        let explicit = inherited_fixture(
            r#"<a:p><a:r><a:rPr sz="900"/><a:t>AAAA AAAA AAAA</a:t></a:r></a:p>"#,
            "1800",
        )
        .0;
        assert_eq!(inherited, explicit);
        assert!(inherited.contains("font-size=\"12\""));
    }
    #[test]
    fn paragraph_positions_include_spacing_breaks_and_empty_end_style() {
        for (line_spacing, expected) in [
            (
                r#"<a:spcPct val="150000"/>"#,
                vec![13.6, 31.6, 58.666666666666664],
            ),
            (
                r#"<a:spcPts val="1500"/>"#,
                vec![21.6, 41.6, 62.666666666666664],
            ),
        ] {
            let paragraph = format!(
                r#"<a:p><a:pPr><a:lnSpc>{line_spacing}</a:lnSpc><a:spcBef><a:spcPts val="300"/></a:spcBef><a:spcAft><a:spcPts val="200"/></a:spcAft></a:pPr><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r></a:p><a:p><a:endParaRPr sz="1500"/></a:p>"#
            );
            let (svg, _) = inherited_fixture(&paragraph, "900");
            let root = format!("<svg>{svg}</svg>");
            let doc = Document::parse(&root).unwrap();
            let positions: Vec<f64> = doc
                .descendants()
                .filter(|n| n.has_tag_name("text"))
                .map(|n| n.attribute("y").unwrap().parse().unwrap())
                .collect();
            for (actual, expected) in positions.iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-5, "{positions:?}");
            }
            assert_eq!(positions.len(), 3);
        }
    }
    #[test]
    fn overflowing_text_keeps_diagnostic_without_clipping() {
        let (svg, warnings) = inherited_fixture(
            &format!("<a:p><a:r><a:t>{}</a:t></a:r></a:p>", "A ".repeat(100)),
            "1800",
        );
        assert!(
            warnings
                .records
                .iter()
                .any(|d| d.code == Code::TextOverflow)
        );
        assert_eq!(svg.matches("A ").count(), 100);
    }
    fn autofit_fixture(
        mode: &str,
        inherited_mode: &str,
        anchor: &str,
        paragraphs: &str,
    ) -> (String, TextExtent, Warnings) {
        let xml = format!(
            r#"<root xmlns:a="{A}" xmlns:p="{P}"><p:txBody><a:bodyPr lIns="0" rIns="0" tIns="0" bIns="0" anchor="{anchor}">{mode}</a:bodyPr><a:lstStyle><a:lvl1pPr><a:defRPr sz="1500"><a:latin typeface="Pptx2img Metrics Test"/></a:defRPr></a:lvl1pPr></a:lstStyle>{paragraphs}</p:txBody><p:txBody><a:bodyPr>{inherited_mode}</a:bodyPr></p:txBody></root>"#
        );
        let doc = Document::parse(&xml).unwrap();
        let bodies: Vec<_> = doc
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .collect();
        let mut fonts = Database::new();
        fonts.load_font_data(include_bytes!("../../tests/fixtures/metrics.ttf").to_vec());
        let mut svg = String::new();
        let mut warnings = Warnings::default();
        let extent = render(
            TextBody {
                body: bodies[0],
                inherited: vec![bodies[1]],
                master_style: None,
                cell_props: None,
            },
            200.0,
            20.0,
            &Theme::default(),
            &RenderOptions {
                fonts: Resolver::new(&fonts, &HashMap::new()),
                punctuation_compression: false,
            },
            &mut svg,
            &mut warnings,
        );
        (svg, extent, warnings)
    }
    #[test]
    fn normal_autofit_scales_measurement_and_svg_together() {
        for (scale, expected_size) in [(100000, 20.0), (55000, 11.0), (92500, 18.5)] {
            let (svg, _, _) = autofit_fixture(
                &format!(r#"<a:normAutofit fontScale="{scale}"/>"#),
                "",
                "t",
                "<a:p><a:r><a:t>AA</a:t></a:r></a:p>",
            );
            let root = format!("<svg>{svg}</svg>");
            let doc = Document::parse(&root).unwrap();
            let span = doc.descendants().find(|n| n.has_tag_name("tspan")).unwrap();
            assert_eq!(
                span.attribute("font-size").unwrap().parse::<f64>().unwrap(),
                expected_size
            );
            let positions: Vec<f64> = span
                .attribute("x")
                .unwrap()
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            assert!((positions[1] - measurement_size(expected_size) / 2.0).abs() < 1e-6);
        }
    }
    #[test]
    fn line_spacing_reduction_subtracts_percentage_points_not_fixed_spacing() {
        for (spacing_xml, expected) in [
            (r#"<a:spcPct val="120000"/>"#, 20.0),
            (r#"<a:spcPts val="2400"/>"#, 32.0),
        ] {
            let p = format!(
                "<a:p><a:pPr><a:lnSpc>{spacing_xml}</a:lnSpc></a:pPr><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r></a:p>"
            );
            let (svg, _, _) =
                autofit_fixture(r#"<a:normAutofit lnSpcReduction="20000"/>"#, "", "t", &p);
            let root = format!("<svg>{svg}</svg>");
            let doc = Document::parse(&root).unwrap();
            let y: Vec<f64> = doc
                .descendants()
                .filter(|n| n.has_tag_name("text"))
                .map(|n| n.attribute("y").unwrap().parse().unwrap())
                .collect();
            assert_eq!(y[1] - y[0], expected);
        }
    }
    #[test]
    fn local_no_autofit_cancels_parent_and_shape_fit_preserves_anchor() {
        let paragraph = "<a:p><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r></a:p>";
        let (svg, extent, _) = autofit_fixture(
            "<a:noAutofit/>",
            r#"<a:normAutofit fontScale="55000"/>"#,
            "t",
            paragraph,
        );
        assert!(svg.contains("font-size=\"20\""));
        assert_eq!(extent.height, 20.0);
        for (anchor, offset) in [("t", 0.0), ("ctr", -20.0), ("b", -40.0)] {
            let (_, extent, warnings) = autofit_fixture("<a:spAutoFit/>", "", anchor, paragraph);
            assert_eq!(extent.height, 60.0);
            assert_eq!(extent.offset_y, offset);
            assert!(
                !warnings
                    .records
                    .iter()
                    .any(|d| d.code == Code::TextOverflow)
            );
        }
    }
}
