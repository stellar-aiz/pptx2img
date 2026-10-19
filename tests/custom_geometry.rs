//! Custom geometry (`a:custGeom`) freeforms drawn from independently written paths.
mod support;
use support::*;

/// Render one 80x80 px freeform at (10, 10) with a red fill and a 4 px blue outline.
fn render(paths: &str) -> (resvg::tiny_skia::Pixmap, String) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("custom.pptx");
    let out = d.path().join("out");
    let shape = format!(
        r#"<p:sp><p:spPr>{}<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/><a:pathLst>{paths}</a:pathLst></a:custGeom><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:ln w="38100"><a:solidFill><a:srgbClr val="0000FF"/></a:solidFill></a:ln></p:spPr></p:sp>"#,
        transform(10, 10, 80, 80)
    );
    write_deck(&p, page("sld", "", "", &shape), vec![]);
    let output = run(&p, &out, false);
    check(output);
    (
        image(&out),
        std::fs::read_to_string(out.join("manifest.json")).unwrap(),
    )
}
fn red(pixel: (u8, u8, u8)) -> bool {
    pixel.0 > 200 && pixel.1 < 60 && pixel.2 < 60
}
fn blue(pixel: (u8, u8, u8)) -> bool {
    pixel.2 > 200 && pixel.0 < 100 && pixel.1 < 100
}
const WHITE: (u8, u8, u8) = (255, 255, 255);

#[test]
fn closed_freeform_with_a_bezier_is_filled_and_outlined() {
    // A triangle whose base is a cubic Bezier bowing up to y=65 in the 80x80 path space.
    let (pixels, manifest) = render(
        r#"<a:path w="80" h="80"><a:moveTo><a:pt x="0" y="80"/></a:moveTo><a:lnTo><a:pt x="40" y="0"/></a:lnTo><a:lnTo><a:pt x="80" y="80"/></a:lnTo><a:cubicBezTo><a:pt x="60" y="60"/><a:pt x="20" y="60"/><a:pt x="0" y="80"/></a:cubicBezTo><a:close/></a:path>"#,
    );
    assert!(red(rgb(&pixels, 50, 50)), "{:?}", rgb(&pixels, 50, 50));
    // Below the curve but inside the straight-line triangle: not filled.
    assert_eq!(rgb(&pixels, 50, 85), WHITE);
    assert!(blue(rgb(&pixels, 30, 50)), "{:?}", rgb(&pixels, 30, 50));
    assert_eq!(rgb(&pixels, 15, 15), WHITE);
    assert!(!manifest.contains("custGeom"), "{manifest}");
    assert!(!manifest.contains("Unsupported"), "{manifest}");
}

#[test]
fn arcs_follow_the_ellipse_and_open_unfilled_paths_stay_empty() {
    // Path 1: quarter disk around (0, 80) drawn with arcTo from the top (270 degrees)
    // clockwise to the right. Path 2: an open "V" with fill="none".
    let (pixels, manifest) = render(
        r#"<a:path w="80" h="80"><a:moveTo><a:pt x="0" y="40"/></a:moveTo><a:arcTo wR="40" hR="40" stAng="16200000" swAng="5400000"/><a:lnTo><a:pt x="0" y="80"/></a:lnTo><a:close/></a:path><a:path w="80" h="80" fill="none"><a:moveTo><a:pt x="50" y="10"/></a:moveTo><a:lnTo><a:pt x="70" y="30"/></a:lnTo><a:lnTo><a:pt x="50" y="50"/></a:lnTo></a:path>"#,
    );
    assert!(red(rgb(&pixels, 22, 78)), "{:?}", rgb(&pixels, 22, 78));
    // Outside the chord from (0, 40) to (40, 80) but inside the arc.
    assert!(red(rgb(&pixels, 35, 65)), "{:?}", rgb(&pixels, 35, 65));
    assert_eq!(rgb(&pixels, 50, 50), WHITE);
    // The open path is stroked along its segments and not filled inside.
    assert!(blue(rgb(&pixels, 70, 30)), "{:?}", rgb(&pixels, 70, 30));
    assert_eq!(rgb(&pixels, 66, 40), WHITE);
    assert!(!manifest.contains("custGeom"), "{manifest}");
}
