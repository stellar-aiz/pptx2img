//! OOXML のうち明示座標の図形・文字・画像を描画する。未対応は警告として残す。
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use roxmltree::{Document, Node};
use std::{collections::HashMap, fs::File, io::Read, path::Path};

mod colors;
mod diagnostics;
mod effects;
mod fonts;
mod geometry;
pub(crate) use fonts::Resolver as FontResolver;
mod layout;
mod lines;
mod objects;
mod table_styles;
mod text;
pub use diagnostics::{Code as DiagnosticCode, Diagnostic, FontResolution};
use diagnostics::{Code, Target, Warnings};
mod theme;
use theme::Theme;

const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_R: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const EMU: f64 = 9525.0;
pub struct Deck {
    pub width: f64,
    pub height: f64,
    pub slides: Vec<Slide>,
}
pub struct Slide {
    pub svg: String,
    pub warnings: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}
struct Part {
    target: String,
    kind: String,
    external: bool,
}
type Parts = HashMap<String, Vec<u8>>;
type Relationships = HashMap<String, Part>;

fn warn(warnings: &mut Warnings, value: impl Into<String>) {
    warnings.push(Code::Unsupported, value);
}
fn xml<'a>(parts: &'a Parts, name: &str) -> Result<Document<'a>> {
    let bytes = parts
        .get(name)
        .with_context(|| format!("PPTX 内の部品がありません: {name}"))?;
    Document::parse(std::str::from_utf8(bytes)?)
        .with_context(|| format!("XML を解釈できません: {name}"))
}
fn resolve(base: &str, target: &str) -> Result<String> {
    if target.contains('\\') || target.contains(':') || target.contains('%') {
        bail!("未対応の部品 URI: {target}");
    }
    let mut components: Vec<&str> = if target.starts_with('/') {
        vec![]
    } else {
        base.rsplit_once('/')
            .map(|(dir, _)| dir.split('/').collect())
            .unwrap_or_default()
    };
    for component in target.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    bail!("パッケージ外を指す部品 URI");
                }
            }
            other => components.push(other),
        }
    }
    Ok(components.join("/"))
}
fn relationships(parts: &Parts, base: &str) -> Result<Relationships> {
    let path = match base.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{base}.rels"),
    };
    if !parts.contains_key(&path) {
        return Ok(HashMap::new());
    }
    let doc = xml(parts, &path)?;
    let mut result = HashMap::new();
    for node in doc
        .descendants()
        .filter(|n| n.has_tag_name((PKG_R, "Relationship")))
    {
        let id = node
            .attribute("Id")
            .context("Relationship に Id がありません")?;
        let external = node.attribute("TargetMode") == Some("External");
        let target = node
            .attribute("Target")
            .context("Relationship に Target がありません")?;
        result.insert(
            id.into(),
            Part {
                target: if external {
                    target.into()
                } else {
                    resolve(base, target)?
                },
                kind: node.attribute("Type").unwrap_or("").into(),
                external,
            },
        );
    }
    Ok(result)
}
fn child<'a, 'i>(node: Node<'a, 'i>, ns: &str, tag: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.has_tag_name((ns, tag)))
}
fn number(node: Node<'_, '_>, attr: &str, default: f64) -> f64 {
    node.attribute(attr)
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub struct RenderOptions<'a> {
    pub fonts: FontResolver<'a>,
    pub punctuation_compression: bool,
}

pub fn read(path: &Path, options: &RenderOptions<'_>) -> Result<Deck> {
    let file = File::open(path).context("入力 PPTX を開けません")?;
    if file.metadata()?.len() > 128 * 1024 * 1024 {
        bail!("入力は 128 MiB 以下にしてください");
    }
    let mut archive = zip::ZipArchive::new(file).context("PPTX の ZIP を読めません")?;
    if archive.len() > 10_000 {
        bail!("部品数が上限を超えています");
    }
    let mut parts = HashMap::new();
    let mut total = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let mut data = Vec::new();
        (&mut entry)
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut data)?;
        total += data.len();
        if data.len() > 32 * 1024 * 1024 || total > 256 * 1024 * 1024 {
            bail!("展開後の PPTX が上限を超えています");
        }
        if parts.insert(name, data).is_some() {
            bail!("ZIP 内に同名の部品があります");
        }
    }
    let roots = relationships(&parts, "")?;
    let root = roots
        .values()
        .find(|r| r.kind.ends_with("/officeDocument") && !r.external)
        .context("presentation の関連がありません")?;
    let doc = xml(&parts, &root.target)?;
    if !doc.root_element().has_tag_name((P, "presentation")) {
        bail!("Transitional 形式の PPTX のみ対応しています");
    }
    let size = child(doc.root_element(), P, "sldSz").context("スライド寸法がありません")?;
    let width = number(size, "cx", 0.0) / EMU;
    let height = number(size, "cy", 0.0) / EMU;
    if width <= 0.0 || height <= 0.0 || width > 100_000.0 || height > 100_000.0 {
        bail!("スライド寸法が不正です");
    }
    let rels = relationships(&parts, &root.target)?;
    let mut slides = Vec::new();
    for id in doc.descendants().filter(|n| n.has_tag_name((P, "sldId"))) {
        if slides.len() >= 200 {
            bail!("初期版は 200 スライドまで対応しています");
        }
        let rid = id
            .attribute((R, "id"))
            .context("スライドの関連 ID がありません")?;
        let part = rels.get(rid).context("スライドの関連を解決できません")?;
        if part.external || !part.kind.ends_with("/slide") {
            bail!("スライドの関連が不正です");
        }
        slides.push(render_slide(&parts, &part.target, width, height, options)?);
    }
    if slides.is_empty() {
        bail!("スライドがありません");
    }
    Ok(Deck {
        width,
        height,
        slides,
    })
}

fn color(node: Node<'_, '_>, default: &str, theme: &Theme, warnings: &mut Warnings) -> String {
    colors::resolve(node, default, theme, None, warnings)
}
fn fill(node: Node<'_, '_>, default: &str, theme: &Theme, warnings: &mut Warnings) -> String {
    if child(node, A, "noFill").is_some() {
        return "none".into();
    }
    if let Some(solid) = child(node, A, "solidFill") {
        return color(solid, default, theme, warnings);
    }
    if node.children().any(|n| {
        matches!(
            n.tag_name().name(),
            "gradFill" | "pattFill" | "blipFill" | "grpFill"
        )
    }) {
        warn(warnings, "単色以外の塗りは未対応です");
    }
    default.into()
}
fn render_slide(
    parts: &Parts,
    name: &str,
    width: f64,
    height: f64,
    options: &RenderOptions<'_>,
) -> Result<Slide> {
    let paths = layout::chain(parts, name)?;
    let docs = paths
        .iter()
        .map(|p| xml(parts, p))
        .collect::<Result<Vec<_>>>()?;
    let roots: Vec<_> = docs.iter().map(Document::root_element).collect();
    let mut warnings = Warnings::default();
    warnings.target = Target {
        part: name.into(),
        location: "slide".into(),
        ..Target::default()
    };
    let theme = Theme::load(parts, name, &mut warnings)?;
    let background_node = roots
        .iter()
        .find_map(|r| child(*r, P, "cSld").and_then(|n| child(n, P, "bg")));
    let background = background_node
        .and_then(|n| child(n, P, "bgPr"))
        .map(|n| fill(n, "#ffffff", &theme, &mut warnings))
        .unwrap_or_else(|| "#ffffff".into());
    if background_node.and_then(|n| child(n, P, "bgRef")).is_some() {
        warn(&mut warnings, "テーマ背景は未対応です");
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"{background}\"/>"
    );
    // 低い層から描き、スライド自身が最後に重なる。ひな形のサンプル文字は描かない。
    for index in (0..roots.len()).rev() {
        if index > 0 && theme::bool_attr(roots[0], "showMasterSp") == Some(false) {
            continue;
        }
        if index == 2 && theme::bool_attr(roots[1], "showMasterSp") == Some(false) {
            continue;
        }
        let tree = layout::tree(roots[index]).context("図形ツリーがありません")?;
        let rels = relationships(parts, &paths[index])?;
        let mut renderer = objects::Renderer {
            parts,
            rels: &rels,
            theme: &theme,
            options,
            svg: &mut svg,
            warnings: &mut warnings,
            part: &paths[index],
            ancestors: &roots[index + 1..],
            master_style: roots
                .iter()
                .find(|n| n.has_tag_name((P, "sldMaster")))
                .and_then(|n| child(*n, P, "txStyles")),
        };
        for (position, node) in tree.children().filter(|n| n.is_element()).enumerate() {
            if index > 0 && layout::placeholder(node).is_some() {
                continue;
            }
            renderer.node(node, &format!("spTree/{position}"), 0)?;
        }
    }
    warnings.target = Target {
        part: name.into(),
        location: "slide".into(),
        ..Target::default()
    };
    for n in roots[0].children().filter(|n| n.is_element()) {
        if !matches!(n.tag_name().name(), "cSld" | "clrMapOvr") {
            warn(
                &mut warnings,
                format!("未対応のスライド属性: {}", n.tag_name().name()),
            );
        }
    }
    svg.push_str("</svg>");
    let (warnings, diagnostics) = warnings.into_parts();
    Ok(Slide {
        svg,
        warnings,
        diagnostics,
    })
}
fn render_shape(
    shape: objects::Shape<'_>,
    parts: &Parts,
    rels: &Relationships,
    theme: &Theme,
    options: &RenderOptions<'_>,
    svg: &mut String,
    warnings: &mut Warnings,
) -> Result<()> {
    let node = shape.chain[0];
    if node.has_tag_name((P, "cxnSp"))
        && node
            .descendants()
            .any(|n| n.has_tag_name((A, "stCxn")) || n.has_tag_name((A, "endCxn")))
    {
        warnings.push(
            Code::Unsupported,
            "接続先の接続点に基づくコネクタ経路の再計算は未対応です。保存された形状と変換で描画します",
        );
    }
    let props_chain: Vec<_> = shape
        .chain
        .iter()
        .filter_map(|n| child(*n, P, "spPr"))
        .collect();
    let Some(_) = props_chain.first() else {
        warnings.push(Code::Omitted, "図形属性がない要素を省略しました");
        return Ok(());
    };
    let transforms: Vec<_> = props_chain
        .iter()
        .filter_map(|n| child(*n, A, "xfrm"))
        .collect();
    let Some(transform) = transforms.first().copied() else {
        warnings.push(Code::Omitted, "継承座標を解決できない要素を省略しました");
        return Ok(());
    };
    let (Some(offset), Some(extent)) = (
        transforms.iter().find_map(|n| child(*n, A, "off")),
        transforms.iter().find_map(|n| child(*n, A, "ext")),
    ) else {
        bail!("図形の座標・寸法がありません");
    };
    let x = number(offset, "x", 0.0) / EMU;
    let mut y = number(offset, "y", 0.0) / EMU;
    let w = number(extent, "cx", 0.0) / EMU;
    let mut h = number(extent, "cy", 0.0) / EMU;
    if w < 0.0 || h < 0.0 {
        bail!("図形寸法が負です");
    }
    let mut text_svg = String::new();
    if let Some(body) = child(node, P, "txBody") {
        // 図形の文字領域で計測する。高さ変更で文字領域の幅も変わるため、
        // 最終寸法で再計測する。保存済みfontScaleは再探索しない。
        let geometry_node = props_chain.iter().find_map(|n| child(*n, A, "prstGeom"));
        for attempt in 0..16 {
            let mut measured_warnings = Warnings::default();
            measured_warnings.target = warnings.target.clone();
            let rect = geometry_node
                .and_then(|n| geometry::preset(n, w, h, "", &mut measured_warnings))
                .map(|g| g.text)
                .unwrap_or((0.0, 0.0, w, h));
            text_svg.clear();
            text_svg.push_str(&format!(
                "<g transform=\"translate({} {})\">",
                rect.0, rect.1
            ));
            let extent = text::render(
                text::TextBody {
                    body,
                    inherited: shape
                        .chain
                        .iter()
                        .skip(1)
                        .filter_map(|n| child(*n, P, "txBody"))
                        .collect(),
                    master_style: shape.master_style,
                    cell_props: None,
                },
                rect.2,
                rect.3,
                theme,
                options,
                &mut text_svg,
                &mut measured_warnings,
            );
            text_svg.push_str("</g>");
            let growth = extent.height - rect.3;
            if growth > 0.01 && (rect.3 > 0.0 || h == 0.0) && attempt < 15 && h < 100_000.0 {
                let ratio = if rect.3 > 0.0 {
                    (h / rect.3).max(1.0)
                } else {
                    1.0
                };
                let new_height = (h + growth * ratio).min(100_000.0);
                y += extent.offset_y / growth * (new_height - h);
                h = new_height;
                continue;
            }
            if growth > 0.01 {
                warn(
                    warnings,
                    "図形文字領域の自動拡張が上限に達したため本文が収まらない可能性があります",
                );
            }
            // 中間寸法の診断を残さず、採用した配置の診断だけを公開する。
            for record in measured_warnings.records {
                warnings.measured(record);
            }
            break;
        }
    }
    let rotation = number(transform, "rot", 0.0) / 60_000.0;
    let fx = if matches!(transform.attribute("flipH"), Some("1" | "true")) {
        -1
    } else {
        1
    };
    let fy = if matches!(transform.attribute("flipV"), Some("1" | "true")) {
        -1
    } else {
        1
    };
    svg.push_str(&format!(
        "<g transform=\"translate({} {}) rotate({rotation}) scale({fx} {fy}) translate({} {})\">",
        x + w / 2.0,
        y + h / 2.0,
        -w / 2.0,
        -h / 2.0
    ));
    for attr in props_chain
        .iter()
        .flat_map(|n| n.children())
        .filter(|n| n.is_element())
    {
        if !matches!(
            attr.tag_name().name(),
            "xfrm" | "prstGeom" | "solidFill" | "noFill" | "ln" | "effectLst"
        ) {
            warn(
                warnings,
                format!("未対応の図形属性: {}", attr.tag_name().name()),
            );
        }
    }
    let style_node = shape.chain.iter().find_map(|n| child(*n, P, "style"));
    if style_node.is_some_and(|n| {
        n.children()
            .any(|c| c.is_element() && !matches!(c.tag_name().name(), "fillRef" | "lnRef"))
    }) {
        warn(warnings, "テーマ由来の効果・文字スタイル参照は未対応です");
    }
    let effect_count = props_chain
        .iter()
        .find_map(|n| child(*n, A, "effectLst"))
        .map(|n| {
            effects::begin(
                n,
                (w, h, rotation, fx as f64, fy as f64),
                theme,
                svg,
                warnings,
            )
        })
        .unwrap_or(0);
    if node.has_tag_name((P, "pic")) {
        render_image(node, parts, rels, w, h, svg, warnings)?;
    } else {
        let explicit_fill = props_chain.iter().copied().find(|n| layout::has_fill(*n));
        let fill = if let Some(explicit) = explicit_fill {
            fill(explicit, "none", theme, warnings)
        } else if let Some(reference) = style_node.and_then(|n| child(n, A, "fillRef")) {
            theme.fill_reference(reference, warnings)
        } else {
            "none".into()
        };
        let direct_lines: Vec<_> = props_chain
            .iter()
            .filter_map(|n| child(*n, A, "ln"))
            .collect();
        let resolved_line = lines::resolve(
            &direct_lines,
            style_node.and_then(|n| child(n, A, "lnRef")),
            theme,
            warnings,
        );
        let line_doc = resolved_line.as_deref().map(Document::parse).transpose()?;
        let line = line_doc
            .as_ref()
            .and_then(|doc| child(doc.root_element(), A, "ln"));

        let stroke = line
            .map(|n| fill_line(n, theme, warnings))
            .unwrap_or_else(|| "none".into());
        let line_width = line.map(|n| number(n, "w", 9525.0) / EMU).unwrap_or(1.0);
        let decorations = line
            .map(|n| lines::decorations(n, &stroke, line_width, svg, warnings))
            .unwrap_or_default();
        let style = format!(
            "{decorations} fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{line_width}\""
        );

        if let Some(geometry) = props_chain.iter().find_map(|n| child(*n, A, "prstGeom")) {
            if let Some(preset) = geometry::preset(geometry, w, h, &style, warnings) {
                svg.push_str(&preset.svg);
            } else {
                if geometry.attribute("prst") != Some("roundRect")
                    && geometry.descendants().any(|n| n.has_tag_name((A, "gd")))
                {
                    warn(warnings, "図形の調整値は未対応です");
                }
                match geometry.attribute("prst").unwrap_or("") {
                    "rect" => {
                        svg.push_str(&format!("<rect width=\"{w}\" height=\"{h}\" {style}/>"))
                    }
                    "roundRect" => {
                        let mut adjustment = 16667.0_f64;
                        for guide in geometry.descendants().filter(|n| n.has_tag_name((A, "gd"))) {
                            let value = guide
                                .attribute("fmla")
                                .and_then(|s| s.strip_prefix("val "))
                                .and_then(|s| s.trim().parse::<f64>().ok())
                                .filter(|v| v.is_finite());
                            if let (Some("adj"), Some(value)) = (guide.attribute("name"), value) {
                                adjustment = value;
                            } else {
                                warn(warnings, "角丸矩形の調整式は未対応です");
                            }
                        }
                        let radius = w.min(h) * adjustment.clamp(0.0, 50000.0) / 100000.0;
                        svg.push_str(&format!("<rect width=\"{w}\" height=\"{h}\" rx=\"{radius}\" ry=\"{radius}\" {style}/>"));
                    }
                    "ellipse" => svg.push_str(&format!(
                        "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {style}/>",
                        w / 2.0,
                        h / 2.0,
                        w / 2.0,
                        h / 2.0
                    )),
                    "bentConnector2" => {
                        svg.push_str(&format!("<path d=\"M0 0 H{w} V{h}\" {style}/>"))
                    }
                    "line" | "straightConnector1" => {
                        svg.push_str(&format!("<line x2=\"{w}\" y2=\"{h}\" {style}/>"))
                    }
                    other => {
                        warnings.push(Code::Omitted, format!("未対応のプリセット図形: {other}"))
                    }
                }
            }
        }
        svg.push_str(&text_svg);
    }
    for _ in 0..effect_count {
        svg.push_str("</g>");
    }
    svg.push_str("</g>");
    Ok(())
}
fn fill_line(node: Node<'_, '_>, theme: &Theme, warnings: &mut Warnings) -> String {
    for n in node.children().filter(|n| n.is_element()) {
        if !matches!(
            n.tag_name().name(),
            "solidFill"
                | "noFill"
                | "prstDash"
                | "headEnd"
                | "tailEnd"
                | "round"
                | "bevel"
                | "miter"
        ) {
            warn(warnings, format!("未対応の線属性: {}", n.tag_name().name()));
        }
    }
    fill(node, "none", theme, warnings)
}
fn render_image(
    node: Node<'_, '_>,
    parts: &Parts,
    rels: &Relationships,
    w: f64,
    h: f64,
    svg: &mut String,
    warnings: &mut Warnings,
) -> Result<()> {
    let Some(blip) = node.descendants().find(|n| n.has_tag_name((A, "blip"))) else {
        warnings.push(Code::Omitted, "画像参照がありません");
        return Ok(());
    };
    let Some(id) = blip.attribute((R, "embed")) else {
        warnings.push(Code::Omitted, "外部リンク画像は読み込みません");
        return Ok(());
    };
    let part = rels.get(id).context("画像の関連がありません")?;
    if part.external {
        warnings.push(Code::Omitted, "外部リンク画像は読み込みません");
        return Ok(());
    }
    let data = parts.get(&part.target).context("画像の部品がありません")?;
    let dimensions = imagesize::blob_size(data).context("画像寸法を取得できません")?;
    if dimensions.width > 8192
        || dimensions.height > 8192
        || dimensions.width.saturating_mul(dimensions.height) > 16_777_216
    {
        bail!("埋め込み画像の画素数が上限を超えています");
    }
    let mime = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else {
        warnings.push(Code::Omitted, "PNG/JPEG 以外の画像は未対応です");
        return Ok(());
    };
    if node.descendants().any(|n| {
        n.has_tag_name((A, "tile")) || (n.has_tag_name((A, "fillRect")) && n.attributes().len() > 0)
    }) || blip.children().any(|n| n.is_element())
    {
        warn(warnings, "画像のタイル・効果・塗り先の矩形指定は未対応です");
    }
    let crop = node.descendants().find(|n| n.has_tag_name((A, "srcRect")));
    let values: Vec<_> = ["l", "t", "r", "b"]
        .iter()
        .map(|key| crop.map(|n| number(n, key, 0.0) / 100000.0).unwrap_or(0.0))
        .collect();
    let (left, top, right, bottom) = (values[0], values[1], values[2], values[3]);
    if left + right >= 1.0 || top + bottom >= 1.0 {
        bail!("画像のトリミング後の寸法が正数ではありません");
    }
    let iw = w / (1.0 - left - right);
    let ih = h / (1.0 - top - bottom);
    // 入れ子の SVG の表示領域で切り抜く。回転・反転は外側の図形座標系で適用する。
    svg.push_str(&format!("<svg width=\"{w}\" height=\"{h}\" overflow=\"hidden\"><image x=\"{}\" y=\"{}\" width=\"{iw}\" height=\"{ih}\" preserveAspectRatio=\"none\" href=\"data:{mime};base64,{}\"/></svg>", -left*iw, -top*ih, STANDARD.encode(data)));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_parts_stay_in_package() {
        assert_eq!(
            resolve("ppt/slides/slide1.xml", "../media/image1.png").unwrap(),
            "ppt/media/image1.png"
        );
        assert!(resolve("ppt/slides/slide1.xml", "../../../outside").is_err());
        assert!(resolve("ppt/slides/slide1.xml", "https://example.com/a").is_err());
    }
    #[test]
    fn xml_text_is_escaped() {
        assert_eq!(escape("<&\""), "&lt;&amp;&quot;");
    }
}
