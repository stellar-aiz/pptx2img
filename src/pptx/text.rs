//! フォントの公称 advance と行メトリクスによる横書き配置。
//! 句読点のぶら下がりと固定行送りを解釈する。複雑な shaping などは未対応。
use super::*;
use resvg::usvg::fontdb::{Database, Family, Query, Style as FontStyle, Weight};
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
    warnings: &mut Vec<String>,
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
fn glyphs(
    value: &str,
    mut style: Style,
    fonts: &Database,
    warnings: &mut Vec<String>,
) -> Vec<Glyph> {
    let families = [if style.face == "sans-serif" {
        Family::SansSerif
    } else {
        Family::Name(&style.face)
    }];
    let query = Query {
        families: &families,
        weight: if style.bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        },
        style: if style.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..Default::default()
    };
    let id = fonts.query(&query).or_else(|| {
        warn(
            warnings,
            format!("指定フォントがないため代替します: {}", style.face),
        );
        fonts.query(&Query {
            families: &[Family::SansSerif],
            ..query
        })
    });
    if let Some(id) = id {
        if let Some(info) = fonts.face(id) {
            style.face = info.families[0].0.clone();
        }
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
            let gm = font.glyph_metrics(size, LocationRef::default());
            let map = font.charmap();
            Some(
                value
                    .chars()
                    .map(|ch| {
                        let advance = map.map(ch).and_then(|id| gm.advance_width(id));
                        if advance.is_none() && ch != '\n' {
                            warn(
                                warnings,
                                format!("フォントに文字がなく幅を近似します: U+{:04X}", ch as u32),
                            );
                        }
                        Glyph {
                            ch,
                            offset: 0.0,
                            compressed: false,
                            advance: f64::from(advance.unwrap_or(style.size as f32)),
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
    warn(
        warnings,
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
fn wrap(glyphs: &[Glyph], available: f64, enabled: bool, hanging: bool) -> Vec<Vec<Glyph>> {
    let mut result = Vec::new();
    for hard_line in glyphs.split(|g| g.ch == '\n') {
        if !enabled || hard_line.is_empty() {
            result.push(hard_line.to_vec());
            continue;
        }
        let mut remaining = hard_line;
        while !remaining.is_empty() {
            let mut sum = 0.0;
            let mut end = 0;
            while end < remaining.len() {
                let candidate = if end == 0 {
                    remaining[end].advance
                } else {
                    sum - remaining[end - 1].advance
                        + step(&remaining[end - 1], Some(&remaining[end]))
                        + remaining[end].advance
                };
                if end > 0 && candidate > available {
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
            let mut minimum = 0.0;
            let mut end = 0;
            while end < remaining.len() {
                let next = minimum + remaining[end].advance - capacity(&remaining[end]);
                if end > 0 && next > available {
                    break;
                }
                minimum = next;
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
            let ratio = if total == 0.0 {
                0.0
            } else if end < candidate {
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
pub(super) fn render(
    body: Node<'_, '_>,
    w: f64,
    h: f64,
    theme: &Theme,
    options: &RenderOptions<'_>,
    svg: &mut String,
    warnings: &mut Vec<String>,
) {
    warn(
        warnings,
        "文字組みは近似です。複雑な shaping・自動縮小・マスター文字スタイル・完全な Office 互換は未対応です",
    );
    if options.punctuation_compression {
        warn(
            warnings,
            "自動句読点圧縮は実験的な描画方針です。PPTX 内の指定ではありません",
        );
    }
    let bp = child(body, A, "bodyPr");
    if bp.is_some_and(|n| n.attribute("vert").is_some_and(|s| s != "horz")) {
        warn(warnings, "縦書きは未対応です");
    }
    let left = bp.map(|n| number(n, "lIns", 91440.0) / EMU).unwrap_or(9.6);
    let right = bp.map(|n| number(n, "rIns", 91440.0) / EMU).unwrap_or(9.6);
    let top = bp.map(|n| number(n, "tIns", 45720.0) / EMU).unwrap_or(4.8);
    let bottom = bp.map(|n| number(n, "bIns", 45720.0) / EMU).unwrap_or(4.8);
    let wrap_enabled = bp.and_then(|n| n.attribute("wrap")) != Some("none");
    let available = (w - left - right).max(1.0);
    let placeholder = body
        .parent()
        .and_then(|n| n.descendants().find(|n| n.has_tag_name((P, "ph"))));
    let style_kind = match placeholder.and_then(|n| n.attribute("type")) {
        Some("title" | "ctrTitle") => "titleStyle",
        Some("body" | "obj" | "subTitle") => "bodyStyle",
        _ => "otherStyle",
    };
    let mut lines = Vec::new();
    let mut cursor = 0.0;
    for paragraph in body.children().filter(|n| n.has_tag_name((A, "p"))) {
        let pp = child(paragraph, A, "pPr");
        let level = pp
            .map(|n| number(n, "lvl", 0.0) as usize)
            .unwrap_or(0)
            .min(8);
        let level_tag = format!("lvl{}pPr", level + 1);
        let list = child(body, A, "lstStyle").and_then(|n| child(n, A, &level_tag));
        let list_default = child(body, A, "lstStyle").and_then(|n| child(n, A, "defPPr"));
        let hanging = paragraph_hanging(
            [pp, list, list_default],
            theme.hanging_punctuation(style_kind, level),
        );
        let defaults: Vec<_> = [pp, list, list_default]
            .into_iter()
            .flatten()
            .filter_map(|n| child(n, A, "defRPr"))
            .collect();
        let mut all = Vec::new();
        for run in paragraph.children().filter(|n| n.is_element()) {
            if run.has_tag_name((A, "br")) {
                all.extend(glyphs(
                    "\n",
                    style(&defaults, false, theme, warnings),
                    options.fonts,
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
                            style(&nodes, last.unwrap(), theme, warnings),
                            options.fonts,
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
                        style(&nodes, last.unwrap_or(false), theme, warnings),
                        options.fonts,
                        warnings,
                    ));
                }
            }
        }
        if pp.is_some_and(|n| {
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
        let empty_size = style(&defaults, false, theme, warnings).size;
        let natural = all
            .iter()
            .map(|g| g.ascent + g.descent + g.leading)
            .fold(0.0, f64::max)
            .max(empty_size);
        cursor += spacing(pp, "spcBef", natural, 0.0);
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
                warn(warnings, "文字がテキスト枠の横幅を超えています");
            }
            let ascent = glyph_line.iter().map(|g| g.ascent).fold(0.0, f64::max);
            let descent = glyph_line.iter().map(|g| g.descent).fold(0.0, f64::max);
            let leading = glyph_line.iter().map(|g| g.leading).fold(0.0, f64::max);
            let line_height = if glyph_line.is_empty() {
                empty_size
            } else {
                ascent + descent + leading
            };
            let advance = spacing(pp, "lnSpc", line_height, line_height);
            let x = match pp
                .and_then(|n| n.attribute("algn"))
                .or_else(|| list.and_then(|n| n.attribute("algn")))
            {
                Some("ctr") => left + (available - width) / 2.0,
                Some("r") => left + available - width,
                _ => left,
            };
            lines.push(Line {
                glyphs: glyph_line,
                x,
                baseline: cursor + baseline_offset(pp, ascent, line_height, advance),
            });
            cursor += advance;
        }
        cursor += spacing(pp, "spcAft", natural, 0.0);
    }
    if cursor > h - top - bottom + 0.01 {
        warn(warnings, "文字がテキスト枠の高さを超えています");
    }
    let vertical = match bp.and_then(|n| n.attribute("anchor")) {
        Some("ctr") => top + (h - top - bottom - cursor) / 2.0,
        Some("b") => h - bottom - cursor,
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
        let positioned = line.glyphs.iter().any(|g| g.offset != 0.0 || g.compressed);
        let mut start = 0;
        while start < line.glyphs.len() {
            let style = &line.glyphs[start].style;
            let mut end = start + 1;
            while end < line.glyphs.len() && line.glyphs[end].style == *style {
                end += 1;
            }
            let positions = if style.spacing != 0.0 || positioned {
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
            } else {
                x.to_string()
            };
            let value: String = line.glyphs[start..end].iter().map(|g| g.ch).collect();
            svg.push_str(&format!("<tspan x=\"{positions}\" font-family=\"{}\" font-size=\"{}\" fill=\"{}\" font-weight=\"{}\" font-style=\"{}\">{}</tspan>", escape(&style.face), style.size, style.color, if style.bold {"bold"} else {"normal"}, if style.italic {"italic"} else {"normal"}, escape(&value)));
            x += text_width(&line.glyphs[start..end]);
            start = end;
        }
        svg.push_str("</text>");
    }
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
            ("0", "none", "0", false),
        ] {
            let source = format!(
                r#"<a:txBody xmlns:a="{A}"><a:bodyPr lIns="0" rIns="0" wrap="{wrap_mode}"/><a:p><a:r><a:rPr sz="750" spc="{spc}"/><a:t>あ、いう、え</a:t></a:r></a:p></a:txBody>"#
            );
            let doc = Document::parse(&source).unwrap();
            let mut svg = String::new();
            let mut warnings = vec![];
            render(
                doc.root_element(),
                55.0,
                100.0,
                &Theme::default(),
                &RenderOptions {
                    fonts: &Database::new(),
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
        let resolve =
            |nodes: &[Node<'_, '_>]| style(nodes, true, &Theme::default(), &mut vec![]).spacing;
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
                    fonts: &Database::new(),
                    punctuation_compression: false,
                },
                &mut svg,
                &mut vec![],
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
        let mut warnings = vec![];
        render(
            doc.root_element(),
            50.0,
            100.0,
            &Theme::default(),
            &RenderOptions {
                fonts: &Database::new(),
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
        let s = style(&[run, defaults], false, &Theme::default(), &mut vec![]);
        assert_eq!(s.size, 16.0);
        assert!(!s.bold);
        assert_eq!(s.face, "Example");
    }
}
