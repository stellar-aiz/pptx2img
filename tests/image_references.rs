//! A picture whose reference cannot be resolved is omitted on its own; the deck still renders.
mod support;
use resvg::tiny_skia::{Color, Pixmap};
use std::fs;
use support::*;

const OLE_OBJECT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject";
const HD_PHOTO: &str = "http://schemas.microsoft.com/office/2007/relationships/hdphoto";
const IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

fn blue_png() -> Vec<u8> {
    let mut png = Pixmap::new(2, 2).unwrap();
    png.fill(Color::from_rgba8(0, 0, 255, 255));
    png.encode_png().unwrap()
}
/// A 30x30 px picture at (10, 10) with `blip` as its `a:blip` element, plus a red marker.
fn shapes(blip: &str) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="5" name="pic"/></p:nvPicPr><p:blipFill>{blip}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}</p:spPr></p:pic>{}"#,
        transform(10, 10, 30, 30),
        rect(60, 60, 20, 20, "FF0000")
    )
}
/// Render a deck whose slide has one relationship `img` of `kind` to `target`.
fn render(
    blip: &str,
    kind: &str,
    target: &str,
    media: Option<(&str, Vec<u8>)>,
) -> (Pixmap, serde_json::Value) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="img" Type="{kind}" Target="{target}"/></Relationships>"#
    );
    let mut extra = vec![("ppt/slides/_rels/slide1.xml.rels", rels.into_bytes())];
    extra.extend(media);
    write_deck(&input, page("sld", "", "", &shapes(blip)), extra);
    check(run(&input, &out, false));
    let img = image(&out);
    assert_eq!(
        rgb(&img, 70, 70),
        (255, 0, 0),
        "the rest of the slide renders"
    );
    let manifest = serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    (img, manifest)
}
/// Every `omitted` diagnostic on the slide, as (message, object_id).
fn omitted(manifest: &serde_json::Value) -> Vec<(String, String)> {
    manifest["slides"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "omitted")
        .map(|d| {
            (
                d["message"].as_str().unwrap().into(),
                d["object_id"].as_str().unwrap().into(),
            )
        })
        .collect()
}
fn omission(message: &str) -> Vec<(String, String)> {
    vec![(message.into(), "5".into())]
}

#[test]
fn blip_through_a_skipped_relationship_type_is_loaded_and_judged_per_picture() {
    // An HD Photo part is loaded for the picture and omitted as an unsupported format.
    let (img, report) = render(
        r#"<a:blip r:embed="img"/>"#,
        HD_PHOTO,
        "../media/image1.wdp",
        Some(("ppt/media/image1.wdp", b"II\xbc\x01wdp".to_vec())),
    );
    assert_eq!(rgb(&img, 25, 25), (255, 255, 255));
    assert_eq!(
        omitted(&report),
        omission("Only PNG/JPEG images are supported: ppt/media/image1.wdp")
    );
    // PNG data behind a non-image relationship type renders, as it did before #48.
    let (img, report) = render(
        r#"<a:blip r:embed="img"/>"#,
        OLE_OBJECT,
        "../media/image1.png",
        Some(("ppt/media/image1.png", blue_png())),
    );
    assert_eq!(rgb(&img, 25, 25), (0, 0, 255));
    assert_eq!(omitted(&report), vec![]);
}

#[test]
fn dangling_embed_is_omitted_with_the_relationship_id() {
    let (img, report) = render(
        r#"<a:blip r:embed="missing"/>"#,
        IMAGE,
        "../media/image1.png",
        Some(("ppt/media/image1.png", blue_png())),
    );
    assert_eq!(rgb(&img, 25, 25), (255, 255, 255));
    assert_eq!(
        omitted(&report),
        omission("Missing image relationship: missing")
    );
}

#[test]
fn relationship_to_an_absent_part_is_omitted_with_the_part_name() {
    let (img, report) = render(
        r#"<a:blip r:embed="img"/>"#,
        IMAGE,
        "../media/absent.png",
        None,
    );
    assert_eq!(rgb(&img, 25, 25), (255, 255, 255));
    assert_eq!(
        omitted(&report),
        omission("Missing image part: ppt/media/absent.png")
    );
}

#[test]
fn crop_without_visible_area_is_omitted() {
    let blip = r#"<a:blip r:embed="img"/><a:srcRect l="60000" r="50000"/>"#;
    let (img, report) = render(
        blip,
        IMAGE,
        "../media/image1.png",
        Some(("ppt/media/image1.png", blue_png())),
    );
    assert_eq!(rgb(&img, 25, 25), (255, 255, 255));
    assert_eq!(
        omitted(&report),
        omission("Image crop leaves no visible area: ppt/media/image1.png")
    );
}
