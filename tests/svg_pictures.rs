//! SVG pictures (`asvg:svgBlip`) render, with or without a PNG fallback, and missing or
//! external references get accurate diagnostics.
mod support;
use resvg::tiny_skia::{Color, Pixmap};
use std::fs;
use support::*;

const ASVG: &str = "http://schemas.microsoft.com/office/drawing/2016/SVG/main";
const SVG_EXT: &str = "{96DAC541-7B7A-43D3-8B79-37D633B846F1}";

/// Left half green, right half blue; stretched to the frame like any picture.
const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="5" height="10" fill="#00FF00"/><rect x="5" width="5" height="10" fill="#0000FF"/></svg>"##;

/// A PNG fallback relationship and an SVG relationship.
const BOTH: [(&str, &str, &str); 2] = [
    ("png", "../media/icon.png", ""),
    ("svg", "../media/icon.svg", ""),
];

fn yellow_png() -> Vec<u8> {
    let mut png = Pixmap::new(2, 2).unwrap();
    png.fill(Color::from_rgba8(255, 255, 0, 255));
    png.encode_png().unwrap()
}
/// `a:blip` with optional `r:embed` and an `svgBlip` extension with `svg` attributes.
fn blip(embed: Option<&str>, svg: Option<&str>) -> String {
    let embed = embed.map_or(String::new(), |id| format!(r#" r:embed="{id}""#));
    let ext = svg.map_or(String::new(), |attrs| {
        format!(
            r#"<a:extLst><a:ext uri="{SVG_EXT}"><asvg:svgBlip xmlns:asvg="{ASVG}" {attrs}/></a:ext></a:extLst>"#
        )
    });
    format!("<a:blip{embed}>{ext}</a:blip>")
}
/// Render a 60x30 px picture at (10, 10) plus a red marker; relationships are
/// (id, type suffix, target, extra attributes).
fn render(
    blip: &str,
    rels: &[(&str, &str, &str)],
    media: Vec<(&str, Vec<u8>)>,
) -> (Pixmap, Vec<(String, String)>) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let shapes = format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="5" name="icon"/></p:nvPicPr><p:blipFill>{blip}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}</p:spPr></p:pic>{}"#,
        transform(10, 10, 60, 30),
        rect(60, 60, 20, 20, "FF0000")
    );
    let rels: String = rels
        .iter()
        .map(|(id, target, extra)| {
            format!(r#"<Relationship Id="{id}" Type="{R}/image" Target="{target}"{extra}/>"#)
        })
        .collect();
    let rels = format!(r#"<Relationships xmlns="{PR}">{rels}</Relationships>"#);
    let mut extra = vec![("ppt/slides/_rels/slide1.xml.rels", rels.into_bytes())];
    extra.extend(media);
    write_deck(&input, page("sld", "", "", &shapes), extra);
    check(run(&input, &out, false));
    let img = image(&out);
    assert_eq!(
        rgb(&img, 70, 70),
        (255, 0, 0),
        "the rest of the slide renders"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let diagnostics = manifest["slides"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap().into(),
                d["message"].as_str().unwrap().into(),
            )
        })
        .collect();
    (img, diagnostics)
}
fn svg_part(data: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![("ppt/media/icon.svg", data.as_bytes().to_vec())]
}
fn omitted(message: &str) -> Vec<(String, String)> {
    vec![("omitted".into(), message.into())]
}

#[test]
fn svg_only_picture_renders_stretched_to_its_frame() {
    let (img, diagnostics) = render(
        &blip(None, Some(r#"r:embed="svg""#)),
        &[("svg", "../media/icon.svg", "")],
        svg_part(ICON),
    );
    assert_eq!(rgb(&img, 15, 25), (0, 255, 0));
    assert_eq!(
        rgb(&img, 65, 25),
        (0, 0, 255),
        "stretched to the 60 px frame"
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn png_fallback_is_drawn_once_without_the_extension_warning() {
    let mut media = svg_part(ICON);
    media.push(("ppt/media/icon.png", yellow_png()));
    let (img, diagnostics) = render(&blip(Some("png"), Some(r#"r:embed="svg""#)), &BOTH, media);
    assert_eq!(rgb(&img, 15, 25), (255, 255, 0));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn rejected_svg_falls_back_to_png_or_is_omitted_once() {
    let linked = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="https://example.com/a.png" width="10" height="10"/></svg>"#;
    let mut media = svg_part(linked);
    media.push(("ppt/media/icon.png", yellow_png()));
    // The PNG fallback comes first, so the rejected SVG is never consulted.
    let (img, diagnostics) = render(&blip(Some("png"), Some(r#"r:embed="svg""#)), &BOTH, media);
    assert_eq!(rgb(&img, 15, 25), (255, 255, 0));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // A broken PNG fallback lets the SVG decide; both reasons go into one omission.
    let mut media = svg_part(linked);
    media.push(("ppt/media/icon.png", b"not an image".to_vec()));
    let (img, diagnostics) = render(&blip(Some("png"), Some(r#"r:embed="svg""#)), &BOTH, media);
    assert_eq!(rgb(&img, 15, 25), (255, 255, 255));
    assert_eq!(
        diagnostics,
        omitted(
            "Only PNG/JPEG images are supported: ppt/media/icon.png; SVG picture references external or embedded resources: ppt/media/icon.svg"
        )
    );
}

#[test]
fn broken_png_fallback_draws_the_svg() {
    let mut media = svg_part(ICON);
    media.push(("ppt/media/icon.png", b"not an image".to_vec()));
    let (img, diagnostics) = render(&blip(Some("png"), Some(r#"r:embed="svg""#)), &BOTH, media);
    assert_eq!(rgb(&img, 15, 25), (0, 255, 0));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn svg_text_is_reported() {
    let text = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#00FF00"/><text y="8">A</text></svg>"##;
    let (img, diagnostics) = render(
        &blip(None, Some(r#"r:embed="svg""#)),
        &[("svg", "../media/icon.svg", "")],
        svg_part(text),
    );
    assert_eq!(rgb(&img, 15, 25), (0, 255, 0));
    assert_eq!(
        diagnostics,
        vec![(
            "unsupported".into(),
            "Text inside SVG pictures may not render: ppt/media/icon.svg".into()
        )]
    );
}

#[test]
fn missing_and_external_references_are_named_accurately() {
    // No embed of either kind.
    let (_, diagnostics) = render(&blip(None, None), &[], vec![]);
    assert_eq!(diagnostics, omitted("Missing image reference"));
    let (_, diagnostics) = render(&blip(None, Some("")), &[], vec![]);
    assert_eq!(diagnostics, omitted("Missing image reference"));
    // A linked picture.
    let (_, diagnostics) = render(r#"<a:blip r:link="png"/>"#, &[], vec![]);
    assert_eq!(
        diagnostics,
        omitted("Externally linked images are not loaded")
    );
    // An external relationship target.
    let (_, diagnostics) = render(
        &blip(None, Some(r#"r:embed="svg""#)),
        &[(
            "svg",
            "https://example.com/icon.svg",
            r#" TargetMode="External""#,
        )],
        vec![],
    );
    assert_eq!(
        diagnostics,
        omitted("Externally linked images are not loaded")
    );
    let (_, diagnostics) = render(
        &blip(Some("png"), None),
        &[(
            "png",
            "https://example.com/icon.png",
            r#" TargetMode="External""#,
        )],
        vec![],
    );
    assert_eq!(
        diagnostics,
        omitted("Externally linked images are not loaded")
    );
}
