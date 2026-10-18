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
        // 内側座標が異なっても外枠の座標を一度だけ適用する。
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
            .any(|w| w.as_str().unwrap().contains("静的描画"))
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
        ("missing", "プレビュー画像がありません"),
        ("unsupported", "PNG/JPEG 以外"),
        ("external", "外部参照"),
        ("missing-part", "部品がありません"),
        ("hidden", "hidden 指定"),
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
