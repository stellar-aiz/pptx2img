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
    let (unknown, _) = deck(&shape("heart", "", ""));
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

const GUIDE_PRESETS: [&str; 15] = [
    "chevron",
    "triangle",
    "downArrow",
    "leftArrow",
    "leftRightArrow",
    "upDownArrow",
    "diamond",
    "trapezoid",
    "pie",
    "star5",
    "cloud",
    "round2SameRect",
    "wedgeEllipseCallout",
    "flowChartAlternateProcess",
    "flowChartMultidocument",
];
fn count(pixels: &resvg::tiny_skia::Pixmap, color: (u8, u8, u8)) -> usize {
    pixels
        .pixels()
        .iter()
        .filter(|p| (p.red(), p.green(), p.blue()) == color)
        .count()
}
#[test]
fn guide_presets_render_fill_and_outline_without_omission() {
    for kind in GUIDE_PRESETS {
        for attrs in ["", r#"rot="1200000" flipH="1""#] {
            let (result, pixels) = deck(&shape(kind, "", attrs));
            assert!(count(&pixels, (36, 90, 81)) > 50, "{kind} {attrs}");
            assert!(count(&pixels, (195, 123, 73)) > 20, "{kind} {attrs}");
            let diagnostics = &result.slides[0].diagnostics;
            assert!(
                !diagnostics
                    .iter()
                    .any(|d| d.message.contains("Unsupported preset shape")),
                "{kind}: {diagnostics:?}"
            );
            assert!(result.slides[0].warnings.is_empty(), "{kind}");
        }
    }
}
#[test]
fn pie_from_zero_to_half_turn_is_the_lower_half_disc() {
    let (_, pixels) = deck(&shape(
        "pie",
        r#"<a:gd name="adj1" fmla="val 0"/><a:gd name="adj2" fmla="val 10800000"/>"#,
        "",
    ));
    // Box 20..80 x 20..60: the lower half is filled, the upper half is background.
    assert_eq!(rgb(&pixels, 50, 50), (36, 90, 81));
    assert_eq!(rgb(&pixels, 30, 45), (36, 90, 81));
    assert_eq!(rgb(&pixels, 50, 30), (255, 255, 255));
    assert_eq!(rgb(&pixels, 30, 35), (255, 255, 255));
}
#[test]
fn connector_presets_draw_open_lines_even_when_flat() {
    for kind in [
        "bentConnector3",
        "bentConnector4",
        "curvedConnector2",
        "curvedConnector3",
    ] {
        let (result, pixels) = deck(&shape(kind, "", ""));
        // Open paths are stroked, never filled with the shape fill.
        assert_eq!(count(&pixels, (36, 90, 81)), 0, "{kind}");
        assert!(count(&pixels, (195, 123, 73)) > 20, "{kind}");
        assert!(result.slides[0].warnings.is_empty(), "{kind}");
        let flat = shape(kind, "", "").replace(&format!(r#"cy="{}""#, 40 * 9525), r#"cy="0""#);
        let (_, pixels) = deck(&flat);
        assert!(count(&pixels, (195, 123, 73)) > 20, "{kind} flat");
    }
}

const NEXT_TIER_PRESETS: [&str; 31] = [
    "rtTriangle",
    "foldedCorner",
    "borderCallout1",
    "flowChartDocument",
    "flowChartMagneticDisk",
    "blockArc",
    "flowChartTerminator",
    "wedgeRectCallout",
    "donut",
    "cloudCallout",
    "arc",
    "borderCallout2",
    "can",
    "flowChartExtract",
    "snip1Rect",
    "mathPlus",
    "leftBracket",
    "upArrowCallout",
    "downArrowCallout",
    "rightBracket",
    "leftBrace",
    "bentUpArrow",
    "irregularSeal2",
    "plus",
    "curvedDownArrow",
    "star7",
    "parallelogram",
    "rightArrowCallout",
    "flowChartConnector",
    "mathMultiply",
    "circularArrow",
];
#[test]
fn next_tier_presets_render_without_omission() {
    for kind in NEXT_TIER_PRESETS {
        for attrs in ["", r#"rot="1200000" flipH="1""#] {
            let (result, pixels) = deck(&shape(kind, "", attrs));
            assert!(count(&pixels, (36, 90, 81)) > 20, "{kind} {attrs}");
            assert!(count(&pixels, (195, 123, 73)) > 10, "{kind} {attrs}");
            let slide = &result.slides[0];
            assert!(
                !slide
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("Unsupported preset shape")),
                "{kind}: {:?}",
                slide.diagnostics
            );
            assert!(
                !slide.warnings.iter().any(|w| w.contains("non-finite")),
                "{kind}: {:?}",
                slide.warnings
            );
        }
    }
}
#[test]
fn next_tier_adjustments_move_the_outline() {
    let tail = r#"<a:gd name="adj1" fmla="val 0"/><a:gd name="adj2" fmla="val 0"/>"#;
    let (_, default) = deck(&shape("wedgeRectCallout", "", ""));
    let (_, moved) = deck(&shape("wedgeRectCallout", tail, ""));
    assert_ne!(default.data(), moved.data());
    // A tail pointing to the shape centre leaves a plain rectangle.
    assert_eq!(rgb(&moved, 38, 62), (255, 255, 255));
    assert_eq!(rgb(&default, 38, 62), (36, 90, 81));
    // The donut hole is background; a thicker ring (adj) shrinks it.
    let (_, ring) = deck(&shape("donut", "", ""));
    assert_eq!(rgb(&ring, 50, 32), (255, 255, 255));
    let (_, thick) = deck(&shape(
        "donut",
        r#"<a:gd name="adj" fmla="val 40000"/>"#,
        "",
    ));
    assert_eq!(rgb(&thick, 50, 32), (36, 90, 81));
    assert_eq!(rgb(&thick, 50, 40), (255, 255, 255));
}
