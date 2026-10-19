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

/// Two slides that share one layout whose only shape is a picture of `media`.
fn write_layout_deck(input: &std::path::Path, media: &str, data: Vec<u8>) {
    let slide = page("sld", "", "", &rect(60, 60, 20, 20, "00FF00"));
    let slide_rels = rel("slideLayout", "../layouts/layout.xml").into_bytes();
    let presentation = format!(
        r#"<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="r1"/><p:sldId id="257" r:id="r2"/></p:sldIdLst><p:sldSz cx="952500" cy="952500"/></p:presentation>"#
    );
    let presentation_rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="r2" Type="{R}/slide" Target="slides/slide2.xml"/></Relationships>"#
    );
    let part = format!("ppt/media/{media}");
    write_deck(
        input,
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
                image_rels(&format!("../media/{media}")),
            ),
            (part.as_str(), data),
        ],
    );
}

#[test]
fn undecodable_layout_picture_is_omitted_on_every_slide() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    write_layout_deck(&input, "logo.emf", emf());
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

/// Encode an RGBA PNG whose columns take `color(x)`; written for this test.
fn png_columns(width: u32, height: u32, color: impl Fn(u32) -> [u8; 4]) -> Vec<u8> {
    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().unwrap();
        let row: Vec<u8> = (0..width).flat_map(&color).collect();
        let mut stream = writer.stream_writer().unwrap();
        for _ in 0..height {
            std::io::Write::write_all(&mut stream, &row).unwrap();
        }
        stream.finish().unwrap();
    }
    data
}
fn red_blue(width: u32) -> impl Fn(u32) -> [u8; 4] {
    move |x| {
        if x < width / 2 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        }
    }
}
/// Set the IHDR interlace flag and recompute its CRC (bytes 12..29 are the CRC input).
fn mark_interlaced(mut data: Vec<u8>) -> Vec<u8> {
    data[28] = 1;
    let mut crc = !0u32;
    for byte in &data[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    data[29..33].copy_from_slice(&(!crc).to_be_bytes());
    data
}
const FULL_PICTURE: &str = r#"<p:pic><p:nvPicPr><p:cNvPr id="5" name="big"/></p:nvPicPr><p:blipFill><a:blip r:embed="img"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="762000"/></a:xfrm></p:spPr></p:pic>"#;
/// Render `shapes` (referencing `name` as r:embed="img") plus a green marker; return output and manifest.
fn render_deck(
    dir: &std::path::Path,
    name: &str,
    shapes: &str,
    data: Vec<u8>,
) -> (resvg::tiny_skia::Pixmap, serde_json::Value) {
    let input = dir.join(format!("{name}.pptx"));
    let out = dir.join(format!("{name}-out"));
    let shapes = format!("{shapes}{}", rect(90, 90, 10, 10, "00FF00"));
    let part = format!("ppt/media/{name}");
    write_deck(
        &input,
        page("sld", "", "", &shapes),
        vec![
            (
                "ppt/slides/_rels/slide1.xml.rels",
                image_rels(&format!("../media/{name}")),
            ),
            (part.as_str(), data),
        ],
    );
    check(run(&input, &out, false));
    let img = image(&out);
    assert_eq!(rgb(&img, 95, 95), (0, 255, 0), "{name}");
    (img, manifest(&out))
}
fn messages<'a>(manifest: &'a serde_json::Value, code: &str) -> Vec<&'a str> {
    manifest["slides"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .map(|d| d["message"].as_str().unwrap())
        .collect()
}

#[test]
fn oversized_png_and_jpeg_are_downscaled_with_an_approximation() {
    let dir = tempfile::tempdir().unwrap();
    for (name, data, from, to) in [
        (
            "big.png",
            png_columns(4400, 4400, red_blue(4400)),
            "4400x4400",
            "2048x2048",
        ),
        (
            "wide.jpg",
            include_bytes!("fixtures/oversized.jpg").to_vec(),
            "8400x64",
            "2048x16",
        ),
    ] {
        let (img, report) = render_deck(dir.path(), name, FULL_PICTURE, data);
        let (r, g, b) = rgb(&img, 25, 40);
        assert!(r > 240 && g < 16 && b < 16, "{name}: left {r},{g},{b}");
        let (r, g, b) = rgb(&img, 75, 40);
        assert!(r < 16 && g < 16 && b > 240, "{name}: right {r},{g},{b}");
        assert_eq!(
            messages(&report, "approximation")
                .into_iter()
                .filter(|m| m.starts_with("Downscaled"))
                .collect::<Vec<_>>(),
            [format!(
                "Downscaled oversized image from {from} to {to} px: ppt/media/{name}"
            )]
        );
        assert!(messages(&report, "omitted").is_empty(), "{report}");
    }
}

#[test]
fn images_above_the_ceiling_or_undecodable_when_oversized_are_omitted() {
    let dir = tempfile::tempdir().unwrap();
    let valid = png_columns(9000, 8, red_blue(9000));
    let mut truncated = valid.clone();
    truncated.truncate(valid.len() - 40);
    for (name, data, expected) in [
        (
            "long.png",
            png_columns(40_000, 1, red_blue(40_000)),
            "Image exceeds the downscale ceiling of 67108864 pixels or 32768 px per side (40000x1 px): ppt/media/long.png",
        ),
        (
            "interlaced.png",
            mark_interlaced(valid),
            "Cannot downscale oversized image (9000x8 px, interlaced PNG is not downscaled): ppt/media/interlaced.png",
        ),
        (
            "broken.png",
            truncated,
            "Cannot downscale oversized image (9000x8 px, Cannot decode PNG): ppt/media/broken.png",
        ),
    ] {
        let (img, report) = render_deck(dir.path(), name, FULL_PICTURE, data);
        assert_eq!(rgb(&img, 50, 40), (255, 255, 255), "{name}");
        assert_eq!(messages(&report, "omitted"), [expected], "{report}");
    }
}

#[test]
fn oversized_ole_preview_has_one_consistent_outcome() {
    let dir = tempfile::tempdir().unwrap();
    for (name, data, drawn) in [
        ("wide.png", png_columns(9000, 8, red_blue(9000)), true),
        ("long.png", png_columns(40_000, 1, red_blue(40_000)), false),
    ] {
        let frame = r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="7" name="ole"/></p:nvGraphicFramePr><p:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="762000"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/presentationml/2006/ole"><p:oleObj progId="Own.Document"><p:embed/><p:pic><p:blipFill><a:blip r:embed="img"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr/></p:pic></p:oleObj></a:graphicData></a:graphic></p:graphicFrame>"#;
        let (img, report) = render_deck(dir.path(), name, frame, data);
        let rendered = messages(&report, "approximation")
            .iter()
            .any(|m| m.starts_with("Rendered embedded PNG/JPEG preview"));
        let omitted = messages(&report, "omitted");
        assert_eq!(rendered, drawn, "{report}");
        assert_eq!(omitted.len(), if drawn { 0 } else { 1 }, "{report}");
        let (r, _, b) = rgb(&img, 75, 40);
        assert_eq!((r < 16 && b > 240), drawn, "{name}");
    }
}

#[test]
fn oversized_layout_picture_is_downscaled_on_every_slide() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    write_layout_deck(&input, "logo.png", png_columns(4400, 4400, red_blue(4400)));
    check(run(&input, &out, false));
    let report = manifest(&out);
    for (index, file) in ["slide-001.png", "slide-002.png"].iter().enumerate() {
        let img = resvg::tiny_skia::Pixmap::load_png(out.join(file)).unwrap();
        assert_eq!(rgb(&img, 15, 25), (255, 0, 0), "{file}: {report}");
        assert_eq!(rgb(&img, 35, 25), (0, 0, 255), "{file}");
        let diagnostics = report["slides"][index]["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics.iter().any(|d| d["code"] == "approximation"
                && d["part"] == "ppt/layouts/layout.xml"
                && d["message"]
                    == "Downscaled oversized image from 4400x4400 to 2048x2048 px: ppt/media/logo.png"),
            "{file}: {report}"
        );
    }
}
