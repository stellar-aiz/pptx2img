//! Per-picture failures in embedded images must not abort the deck.
mod support;
use std::fs;
use support::*;

/// A minimal EMF header record (EMR_HEADER with the " EMF" signature), written for this test.
fn emf() -> Vec<u8> {
    let mut data = vec![0u8; 88];
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..8].copy_from_slice(&88u32.to_le_bytes());
    data[40..44].copy_from_slice(b" EMF");
    data
}
fn picture(id: u32, x: i32) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="pic{id}"/></p:nvPicPr><p:blipFill><a:blip r:embed="img"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
        transform(x, 10, 30, 30)
    )
}
fn image_rels(target: &str) -> Vec<u8> {
    format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="img" Type="{R}/image" Target="{target}"/></Relationships>"#
    )
    .into_bytes()
}
fn manifest(out: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap()
}
/// The single omitted diagnostic on a slide, as (message, part, object_id).
fn omitted(manifest: &serde_json::Value, slide: usize) -> (&str, &str, &str) {
    let records: Vec<_> = manifest["slides"][slide]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "omitted")
        .collect();
    assert_eq!(records.len(), 1, "{manifest}");
    let field = |key: &str| records[0][key].as_str().unwrap();
    (field("message"), field("part"), field("object_id"))
}

#[test]
fn undecodable_slide_pictures_are_omitted_and_the_rest_renders() {
    for (name, data, message) in [
        (
            "a.emf",
            emf(),
            "Only PNG/JPEG images are supported: ppt/media/a.emf",
        ),
        (
            "a.svg",
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#.to_vec(),
            "Only PNG/JPEG images are supported: ppt/media/a.svg",
        ),
        (
            "a.png",
            b"\x89PNG\r\n\x1a\ntruncated".to_vec(),
            "Cannot read image dimensions: ppt/media/a.png",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.pptx");
        let out = dir.path().join("out");
        let shapes = picture(5, 10) + &rect(60, 60, 20, 20, "FF0000");
        let target = format!("../media/{name}");
        let part = format!("ppt/media/{name}");
        write_deck(
            &input,
            page("sld", "", "", &shapes),
            vec![
                ("ppt/slides/_rels/slide1.xml.rels", image_rels(&target)),
                (part.as_str(), data),
            ],
        );
        check(run(&input, &out, false));
        let img = image(&out);
        assert_eq!(rgb(&img, 70, 70), (255, 0, 0), "{name}");
        assert_eq!(rgb(&img, 25, 25), (255, 255, 255), "{name}");
        assert_eq!(
            omitted(&manifest(&out), 0),
            (message, "ppt/slides/slide1.xml", "5")
        );
    }
}

#[test]
fn undecodable_layout_picture_is_omitted_on_every_slide() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let slide = page("sld", "", "", &rect(60, 60, 20, 20, "00FF00"));
    let slide_rels = rel("slideLayout", "../layouts/layout.xml").into_bytes();
    let presentation = format!(
        r#"<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="r1"/><p:sldId id="257" r:id="r2"/></p:sldIdLst><p:sldSz cx="952500" cy="952500"/></p:presentation>"#
    );
    let presentation_rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="r2" Type="{R}/slide" Target="slides/slide2.xml"/></Relationships>"#
    );
    write_deck(
        &input,
        slide.clone(),
        vec![
            ("ppt/presentation.xml", presentation.into_bytes()),
            (
                "ppt/_rels/presentation.xml.rels",
                presentation_rels.into_bytes(),
            ),
            ("ppt/slides/slide2.xml", slide.into_bytes()),
            ("ppt/slides/_rels/slide1.xml.rels", slide_rels.clone()),
            ("ppt/slides/_rels/slide2.xml.rels", slide_rels),
            (
                "ppt/layouts/layout.xml",
                page("sldLayout", "", "", &picture(9, 10)).into_bytes(),
            ),
            (
                "ppt/layouts/_rels/layout.xml.rels",
                image_rels("../media/logo.emf"),
            ),
            ("ppt/media/logo.emf", emf()),
        ],
    );
    check(run(&input, &out, false));
    let report = manifest(&out);
    for (index, file) in ["slide-001.png", "slide-002.png"].iter().enumerate() {
        let img = resvg::tiny_skia::Pixmap::load_png(out.join(file)).unwrap();
        assert_eq!(rgb(&img, 70, 70), (0, 255, 0), "{file}");
        assert_eq!(
            omitted(&report, index),
            (
                "Only PNG/JPEG images are supported: ppt/media/logo.emf",
                "ppt/layouts/layout.xml",
                "9"
            )
        );
    }
}
