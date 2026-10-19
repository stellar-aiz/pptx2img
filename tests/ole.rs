mod support;
use resvg::tiny_skia::{Color, Pixmap};
use std::fs;
use support::*;
fn render(mode: &str) -> (Pixmap, serde_json::Value) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("ole.pptx");
    let out = d.path().join("out");
    let mut png = Pixmap::new(2, 2).unwrap();
    png.fill(Color::from_rgba8(255, 0, 0, 128));
    let data = match mode {
        "jpeg" => include_bytes!("fixtures/ole-preview.jpg").to_vec(),
        "unsupported" => b"not an image".to_vec(),
        _ => png.encode_png().unwrap(),
    };
    let pic = if mode == "missing" {
        String::new()
    } else {
        // Apply the outer frame transform once even if inner coordinates differ.
        format!(
            r#"<p:pic><p:blipFill><a:blip r:embed="preview"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
            transform(70, 70, 10, 10)
        )
    };
    let mut shapes = format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="7" name="静的OLE"/></p:nvGraphicFramePr><p:xfrm><a:off x="190500" y="285750"/><a:ext cx="381000" cy="285750"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/presentationml/2006/ole"><p:oleObj progId="Own.Document" r:id="binary"><p:embed/>{pic}</p:oleObj></a:graphicData></a:graphic></p:graphicFrame>"#
    );
    if mode == "fallback" || mode == "hidden" {
        shapes = shapes.replace("<p:oleObj progId", "<mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"><mc:Choice Requires=\"v\" xmlns:v=\"urn:schemas-microsoft-com:vml\"><p:oleObj progId=\"Choice.Document\"/></mc:Choice><mc:Fallback><p:oleObj progId")
            .replace("</p:oleObj>", "</p:oleObj></mc:Fallback></mc:AlternateContent>");
    }
    if mode == "no-embed" {
        shapes = shapes.replace(r#"<a:blip r:embed="preview"/>"#, "<a:blip/>");
    }
    if mode == "hidden" {
        shapes = shapes.replace(
            "<p:pic>",
            "<p:pic><p:nvPicPr><p:cNvPr id=\"8\" name=\"非表示\" hidden=\"1\"/></p:nvPicPr>",
        );
    }
    let (target, external) = if mode == "external" {
        (
            "https://invalid.invalid/not-fetched.png",
            " TargetMode=\"External\"",
        )
    } else {
        ("../media/preview", "")
    };
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="preview" Type="{R}/image" Target="{target}"{external}/><Relationship Id="binary" Type="{R}/oleObject" Target="../embeddings/invalid.bin"/></Relationships>"#
    );
    let mut extra = vec![
        ("ppt/slides/_rels/slide1.xml.rels", rels.into_bytes()),
        (
            "ppt/embeddings/invalid.bin",
            "OLE本体は不正でも読み取らない".as_bytes().to_vec(),
        ),
    ];
    if mode != "missing-part" {
        extra.push(("ppt/media/preview", data));
    }
    write_deck(&path, page("sld", "", "", &shapes), extra);
    check(run(&path, &out, false));
    (
        image(&out),
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap(),
    )
}
#[test]
fn internal_preview_uses_frame_coordinates_once() {
    let (png, report) = render("png");
    assert_eq!(rgb(&png, 30, 40), (255, 127, 127));
    assert_eq!(rgb(&png, 10, 40), (255, 255, 255));
    assert_eq!(rgb(&png, 75, 75), (255, 255, 255));
    assert!(
        report["slides"][0]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("Rendered embedded"))
    );
    let (fallback, _) = render("fallback");
    assert_eq!(fallback.data(), png.data());
    let (jpeg, _) = render("jpeg");
    let (r, g, b) = rgb(&jpeg, 30, 40);
    assert!(r > 240 && g < 10 && b < 10);
}
#[test]
fn unavailable_previews_have_concrete_diagnostics() {
    for (mode, reason) in [
        ("missing", "Missing preview image"),
        ("unsupported", "only PNG/JPEG"),
        ("external", "External preview"),
        ("no-embed", "Missing preview image reference"),
        ("missing-part", "Missing preview image part"),
        ("hidden", "is hidden"),
    ] {
        let (img, report) = render(mode);
        assert_eq!(rgb(&img, 30, 40), (255, 255, 255));
        let records: Vec<_> = report["slides"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["message"].as_str().unwrap().contains("OLE ("))
            .collect();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["object_id"], "7");
        assert_eq!(records[0]["code"], "omitted");
        assert!(
            records[0]["message"]
                .as_str()
                .unwrap()
                .contains("Own.Document")
        );
        assert!(
            !records[0]["message"]
                .as_str()
                .unwrap()
                .contains("Choice.Document")
        );
        assert!(
            report["slides"][0]["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| {
                    let s = w.as_str().unwrap();
                    s.contains("Own.Document") && s.contains(reason)
                }),
            "{mode}: {report}"
        );
    }
}

#[test]
fn fallback_ole_styled_table_and_picture_keep_separate_pixels_and_targets() {
    let d = tempfile::tempdir().unwrap();
    for hidden in [false, true] {
        let marker = if hidden {
            r#"<p:nvPicPr><p:cNvPr id="8" name="非表示" hidden="1"/></p:nvPicPr>"#
        } else {
            ""
        };
        let preview = format!(
            r#"<p:pic>{marker}<p:blipFill><a:blip r:embed="preview"/></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
            transform(0, 0, 20, 20)
        );
        let ole = format!(
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="7" name="OLE"/></p:nvGraphicFramePr><p:xfrm><a:off x="0" y="0"/><a:ext cx="190500" cy="190500"/></p:xfrm><a:graphic><a:graphicData><mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Fallback><p:oleObj progId="Own.Document">{preview}</p:oleObj></mc:Fallback></mc:AlternateContent></a:graphicData></a:graphic></p:graphicFrame>"#
        );
        let table = r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="9" name="表"/></p:nvGraphicFramePr><p:xfrm><a:off x="285750" y="190500"/><a:ext cx="571500" cy="571500"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblPr><a:tableStyleId>join</a:tableStyleId></a:tblPr><a:tblGrid><a:gridCol w="571500"/></a:tblGrid><a:tr h="571500"><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:rPr sz="1200"><a:latin typeface="要求不在"/></a:rPr><a:t>A</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#;
        let pic = format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="10" name="通常画像"/></p:nvPicPr><p:blipFill><a:blip r:embed="preview"/></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
            transform(0, 70, 20, 20)
        );
        let styles = format!(
            r#"<a:tblStyleLst xmlns:a="{A}"><a:tblStyle styleId="join"><a:wholeTbl><a:tcStyle><a:fill><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></a:fill></a:tcStyle></a:wholeTbl></a:tblStyle></a:tblStyleLst>"#
        );
        let rels = format!(
            r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="r2" Type="{R}/tableStyles" Target="tableStyles.xml"/></Relationships>"#
        );
        let input = d.path().join(format!("{hidden}.pptx"));
        let out = d.path().join(format!("{hidden}"));
        write_deck(
            &input,
            page("sld", "", "", &(ole + table + &pic)),
            vec![
                ("ppt/_rels/presentation.xml.rels", rels.into_bytes()),
                ("ppt/tableStyles.xml", styles.into_bytes()),
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    rel("image", "../media/preview.jpg")
                        .replace("r1", "preview")
                        .into_bytes(),
                ),
                (
                    "ppt/media/preview.jpg",
                    include_bytes!("fixtures/ole-preview.jpg").to_vec(),
                ),
            ],
        );
        check(run(&input, &out, false));
        let img = image(&out);
        assert_eq!(rgb(&img, 80, 70), (0, 255, 0));
        let (r, g, b) = rgb(&img, 10, 80);
        assert!(r > 240 && g < 10 && b < 10);
        assert_eq!(rgb(&img, 10, 10) == (255, 255, 255), hidden);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
        let records = manifest["slides"][0]["diagnostics"].as_array().unwrap();
        assert_eq!(
            records
                .iter()
                .filter(|d| d["message"].as_str().unwrap().contains("OLE ("))
                .count(),
            1
        );
        let font = records
            .iter()
            .find(|d| d["code"] == "font_substitution")
            .unwrap();
        assert_eq!(font["object_id"], "9");
        assert!(
            font["location"]
                .as_str()
                .unwrap()
                .ends_with("/row/1/cell/1")
        );
    }
    let visible = image(&d.path().join("false"));
    let hidden = image(&d.path().join("true"));
    for y in 20..100 {
        for x in 0..100 {
            assert_eq!(visible.pixel(x, y), hidden.pixel(x, y));
        }
    }
}
