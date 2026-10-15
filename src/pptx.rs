//! OOXML のうち明示座標の図形・文字・画像を描画する。未対応は警告として残す。
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use roxmltree::{Document, Node};
use std::{collections::HashMap, fs::File, io::Read, path::Path};

mod text;
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
}
struct Part {
    target: String,
    kind: String,
    external: bool,
}
type Parts = HashMap<String, Vec<u8>>;
type Relationships = HashMap<String, Part>;

fn warn(warnings: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !warnings.contains(&value) {
        warnings.push(value);
    }
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

pub fn read(path: &Path, fonts: &resvg::usvg::fontdb::Database) -> Result<Deck> {
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
        slides.push(render_slide(&parts, &part.target, width, height, fonts)?);
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

fn color(node: Node<'_, '_>, default: &str, theme: &Theme, warnings: &mut Vec<String>) -> String {
    for c in node.children().filter(|n| n.is_element()) {
        let resolved = match c.tag_name().name() {
            "srgbClr" => c
                .attribute("val")
                .filter(|v| theme::valid_rgb(v))
                .map(|s| format!("#{s}")),
            "sysClr" => c
                .attribute("lastClr")
                .filter(|v| theme::valid_rgb(v))
                .map(|s| format!("#{s}")),
            "schemeClr" => c
                .attribute("val")
                .and_then(|v| theme.color(v))
                .map(str::to_owned),
            _ => None,
        };
        if let Some(rgb) = resolved {
            if c.children().any(|n| n.is_element()) {
                warn(warnings, "色変換（透明度・明度など）は未対応です");
            }
            return rgb;
        }
    }
    warn(warnings, "色を解決できないため代替色で描画しました");
    default.into()
}
fn fill(node: Node<'_, '_>, default: &str, theme: &Theme, warnings: &mut Vec<String>) -> String {
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
    fonts: &resvg::usvg::fontdb::Database,
) -> Result<Slide> {
    let doc = xml(parts, name)?;
    let rels = relationships(parts, name)?;
    let mut warnings = Vec::new();
    let theme = Theme::load(parts, name, &mut warnings)?;
    if rels.values().any(|r| r.kind.ends_with("/slideLayout")) {
        warn(
            &mut warnings,
            "レイアウト・マスターの図形・座標・文字スタイルの完全な継承は未対応です",
        );
    }
    let common = child(doc.root_element(), P, "cSld").context("スライド内容がありません")?;
    let background = child(common, P, "bg")
        .and_then(|n| child(n, P, "bgPr"))
        .map(|n| fill(n, "#ffffff", &theme, &mut warnings))
        .unwrap_or_else(|| "#ffffff".into());
    if child(common, P, "bg")
        .and_then(|n| child(n, P, "bgRef"))
        .is_some()
    {
        warn(&mut warnings, "テーマ背景は未対応です");
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"{background}\"/>"
    );
    let tree = child(common, P, "spTree").context("図形ツリーがありません")?;
    for node in tree.children().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "nvGrpSpPr" | "grpSpPr" => {}
            "sp" | "pic" | "cxnSp" => {
                render_shape(node, parts, &rels, &theme, fonts, &mut svg, &mut warnings)?
            }
            other => warn(
                &mut warnings,
                format!("未対応の要素を省略しました: {other}"),
            ),
        }
    }
    for n in doc.root_element().children().filter(|n| n.is_element()) {
        if !matches!(n.tag_name().name(), "cSld" | "clrMapOvr") {
            warn(
                &mut warnings,
                format!("未対応のスライド属性: {}", n.tag_name().name()),
            );
        }
    }
    svg.push_str("</svg>");
    Ok(Slide { svg, warnings })
}
fn render_shape(
    node: Node<'_, '_>,
    parts: &Parts,
    rels: &Relationships,
    theme: &Theme,
    fonts: &resvg::usvg::fontdb::Database,
    svg: &mut String,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let Some(props) = child(node, P, "spPr") else {
        warn(warnings, "図形属性がない要素を省略しました");
        return Ok(());
    };
    let Some(transform) = child(props, A, "xfrm") else {
        warn(warnings, "継承座標の要素を省略しました");
        return Ok(());
    };
    let (Some(offset), Some(extent)) = (child(transform, A, "off"), child(transform, A, "ext"))
    else {
        bail!("図形の座標・寸法がありません");
    };
    let x = number(offset, "x", 0.0) / EMU;
    let y = number(offset, "y", 0.0) / EMU;
    let w = number(extent, "cx", 0.0) / EMU;
    let h = number(extent, "cy", 0.0) / EMU;
    if w < 0.0 || h < 0.0 {
        bail!("図形寸法が負です");
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
    for attr in props.children().filter(|n| n.is_element()) {
        if !matches!(
            attr.tag_name().name(),
            "xfrm" | "prstGeom" | "solidFill" | "noFill" | "ln"
        ) {
            warn(
                warnings,
                format!("未対応の図形属性: {}", attr.tag_name().name()),
            );
        }
    }
    if child(node, P, "style").is_some() {
        warn(warnings, "テーマ由来の図形スタイルは未対応です");
    }
    if node.descendants().any(|n| n.has_tag_name((P, "ph"))) {
        warn(warnings, "プレースホルダーの継承は未対応です");
    }
    if node.has_tag_name((P, "pic")) {
        render_image(node, parts, rels, w, h, svg, warnings)?;
    } else {
        let fill = fill(props, "none", theme, warnings);
        let line = child(props, A, "ln");
        let stroke = line
            .map(|n| fill_line(n, theme, warnings))
            .unwrap_or_else(|| "none".into());
        let line_width = line.map(|n| number(n, "w", 9525.0) / EMU).unwrap_or(1.0);
        let style = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{line_width}\"");
        if let Some(geometry) = child(props, A, "prstGeom") {
            if geometry.descendants().any(|n| n.has_tag_name((A, "gd"))) {
                warn(warnings, "図形の調整値は未対応です");
            }
            match geometry.attribute("prst").unwrap_or("") {
                "rect" => svg.push_str(&format!("<rect width=\"{w}\" height=\"{h}\" {style}/>")),
                "ellipse" => svg.push_str(&format!(
                    "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {style}/>",
                    w / 2.0,
                    h / 2.0,
                    w / 2.0,
                    h / 2.0
                )),
                "line" => svg.push_str(&format!("<line x2=\"{w}\" y2=\"{h}\" {style}/>")),
                other => warn(warnings, format!("未対応のプリセット図形: {other}")),
            }
        }
        if let Some(body) = child(node, P, "txBody") {
            text::render(body, w, h, theme, fonts, svg, warnings);
        }
    }
    svg.push_str("</g>");
    Ok(())
}
fn fill_line(node: Node<'_, '_>, theme: &Theme, warnings: &mut Vec<String>) -> String {
    for n in node.children().filter(|n| n.is_element()) {
        if !matches!(n.tag_name().name(), "solidFill" | "noFill") {
            warn(warnings, format!("未対応の線属性: {}", n.tag_name().name()));
        }
    }
    fill(node, "#000000", theme, warnings)
}
fn render_image(
    node: Node<'_, '_>,
    parts: &Parts,
    rels: &Relationships,
    w: f64,
    h: f64,
    svg: &mut String,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let Some(blip) = node.descendants().find(|n| n.has_tag_name((A, "blip"))) else {
        warn(warnings, "画像参照がありません");
        return Ok(());
    };
    let Some(id) = blip.attribute((R, "embed")) else {
        warn(warnings, "外部リンク画像は読み込みません");
        return Ok(());
    };
    let part = rels.get(id).context("画像の関連がありません")?;
    if part.external {
        warn(warnings, "外部リンク画像は読み込みません");
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
        warn(warnings, "PNG/JPEG 以外の画像は未対応です");
        return Ok(());
    };
    if node
        .descendants()
        .any(|n| n.has_tag_name((A, "srcRect")) || n.has_tag_name((A, "tile")))
        || blip.children().any(|n| n.is_element())
    {
        warn(warnings, "画像のトリミング・タイル・効果は未対応です");
    }
    svg.push_str(&format!("<image width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"none\" href=\"data:{mime};base64,{}\"/>", STANDARD.encode(data)));
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
