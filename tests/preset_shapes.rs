mod support;
use support::*;
fn shape(kind: &str, guides: &str, attrs: &str) -> String {
    format!(
        r#"<p:sp><p:spPr>{}<a:prstGeom prst="{kind}"><a:avLst>{guides}</a:avLst></a:prstGeom><a:solidFill><a:srgbClr val="245A51"/></a:solidFill><a:ln w="19050"><a:solidFill><a:srgbClr val="C37B49"/></a:solidFill></a:ln></p:spPr></p:sp>"#,
        transform(20, 20, 60, 40).replace("<a:xfrm>", &format!("<a:xfrm {attrs}>"))
    )
}
fn deck(shapes: &str) -> (pptx2img::ConversionReport, resvg::tiny_skia::Pixmap) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("preset.pptx");
    let out = dir.path().join("out");
    write_deck(&input, page("sld", "", "", shapes), vec![]);
    let result = pptx2img::convert(
        &input,
        &out,
        &pptx2img::ConvertOptions {
            font_fallback: true,
            width: 100,
            no_system_fonts: true,
            font_file: vec![
                concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/metrics.ttf").into(),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    (result, image(&out))
}
#[test]
fn supported_presets_keep_fill_outline_and_transform() {
    for kind in [
        "homePlate",
        "upArrow",
        "rightArrow",
        "rightBrace",
        "wedgeRoundRectCallout",
    ] {
        let (result, pixels) = deck(&shape(kind, "", "rot=\"1200000\" flipH=\"1\" flipV=\"1\""));
        assert!(
            pixels
                .pixels()
                .iter()
                .any(|p| (p.red(), p.green(), p.blue()) == (36, 90, 81)),
            "{kind}"
        );
        assert!(
            pixels
                .pixels()
                .iter()
                .any(|p| (p.red(), p.green(), p.blue()) == (195, 123, 73)),
            "{kind}"
        );
        assert!(
            !result.slides[0]
                .diagnostics
                .iter()
                .any(|d| d.code == pptx2img::DiagnosticCode::Omitted),
            "{kind}"
        );
    }
}
#[test]
fn adjustment_changes_home_plate_outline() {
    let (_, default) = deck(&shape("homePlate", "", ""));
    let (report, adjusted) = deck(&shape(
        "homePlate",
        r#"<a:gd name="adj" fmla="val 100000"/>"#,
        "",
    ));
    assert_ne!(default.data(), adjusted.data());
    assert_eq!(rgb(&default, 55, 24), (36, 90, 81));
    assert_eq!(rgb(&adjusted, 55, 24), (255, 255, 255));
    assert!(
        !report.slides[0]
            .warnings
            .iter()
            .any(|w| w.contains("adjustment"))
    );
}
#[test]
fn unknown_preset_and_adjustment_remain_diagnosed() {
    let (unknown, _) = deck(&shape("star5", "", ""));
    assert!(
        unknown.slides[0]
            .diagnostics
            .iter()
            .any(|d| d.code == pptx2img::DiagnosticCode::Omitted)
    );
    let (invalid, _) = deck(&shape(
        "homePlate",
        r#"<a:gd name="adj" fmla="*/ w 1 2"/>"#,
        "",
    ));
    assert!(
        invalid.slides[0]
            .warnings
            .iter()
            .any(|w| w.contains("adjustment formulas"))
    );
}
