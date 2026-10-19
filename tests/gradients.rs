mod support;
use support::*;

fn render(fill: &str, background: &str, attrs: &str) -> (resvg::tiny_skia::Pixmap, String) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("gradient.pptx");
    let out = d.path().join("out");
    let shape = if fill.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/>{fill}</p:spPr></p:sp>"#,
            transform(0, 0, 100, 100).replace("<a:xfrm>", &format!("<a:xfrm {attrs}>"))
        )
    };
    write_deck(&p, page("sld", "", background, &shape), vec![]);
    let output = run(&p, &out, false);
    check(output);
    (
        image(&out),
        std::fs::read_to_string(out.join("manifest.json")).unwrap(),
    )
}
fn gradient(inner: &str, stops: &[(u32, &str)]) -> String {
    let stops = stops
        .iter()
        .map(|(pos, color)| format!(r#"<a:gs pos="{pos}"><a:srgbClr val="{color}"/></a:gs>"#))
        .collect::<String>();
    format!(r#"<a:gradFill><a:gsLst>{stops}</a:gsLst>{inner}</a:gradFill>"#)
}
fn near(actual: (u8, u8, u8), expected: (f64, f64, f64)) -> bool {
    [
        (actual.0, expected.0),
        (actual.1, expected.1),
        (actual.2, expected.2),
    ]
    .iter()
    .all(|(a, e)| (*a as f64 - e).abs() <= 4.0)
}
/// Expected color `t` of the way from red to blue.
fn mix(t: f64) -> (f64, f64, f64) {
    (255.0 * (1.0 - t), 0.0, 255.0 * t)
}

#[test]
fn linear_two_stop_gradient_follows_the_angle() {
    let stops = [(0, "FF0000"), (100000, "0000FF")];
    let (pixels, manifest) = render(&gradient(r#"<a:lin ang="0" scaled="0"/>"#, &stops), "", "");
    assert!(
        near(rgb(&pixels, 10, 50), mix(0.105)),
        "{:?}",
        rgb(&pixels, 10, 50)
    );
    assert!(
        near(rgb(&pixels, 90, 50), mix(0.905)),
        "{:?}",
        rgb(&pixels, 90, 50)
    );
    assert_eq!(rgb(&pixels, 10, 5), rgb(&pixels, 10, 95));
    assert!(!manifest.contains("Non-solid fills"), "{manifest}");
    // 90 degrees runs top to bottom; stops are sorted by position.
    let reversed = [(100000, "0000FF"), (0, "FF0000")];
    let (pixels, _) = render(&gradient(r#"<a:lin ang="5400000"/>"#, &reversed), "", "");
    assert!(near(rgb(&pixels, 50, 10), mix(0.105)));
    assert!(near(rgb(&pixels, 50, 90), mix(0.905)));
    assert_eq!(rgb(&pixels, 5, 10), rgb(&pixels, 95, 10));
}

#[test]
fn stops_keep_alpha_and_inner_positions() {
    let fill = r#"<a:gradFill><a:gsLst><a:gs pos="25000"><a:srgbClr val="FF0000"><a:alpha val="50000"/></a:srgbClr></a:gs><a:gs pos="75000"><a:srgbClr val="FF0000"/></a:gs></a:gsLst><a:lin ang="0"/></a:gradFill>"#;
    let (pixels, _) = render(fill, "", "");
    // Before the first stop the first color repeats: 50% red over white.
    assert!(
        near(rgb(&pixels, 10, 50), (255.0, 127.5, 127.5)),
        "{:?}",
        rgb(&pixels, 10, 50)
    );
    assert_eq!(rgb(&pixels, 90, 50), (255, 0, 0));
}

#[test]
fn radial_path_gradient_starts_at_the_focus_and_backgrounds_use_gradients() {
    let center =
        r#"<a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path>"#;
    let (pixels, manifest) = render(
        &gradient(center, &[(0, "FF0000"), (100000, "0000FF")]),
        "",
        "",
    );
    assert!(
        near(rgb(&pixels, 50, 50), mix(0.0)),
        "{:?}",
        rgb(&pixels, 50, 50)
    );
    // Radius reaches the corners: 30px from the center is 30 / 70.7 of the way.
    for (x, y) in [(20, 50), (80, 50), (50, 20), (50, 80)] {
        assert!(
            near(rgb(&pixels, x, y), mix(30.0 / 70.71)),
            "{:?}",
            rgb(&pixels, x, y)
        );
    }
    assert!(!manifest.contains("approximated"), "{manifest}");
    let rect =
        r#"<a:path path="rect"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path>"#;
    let (_, manifest) = render(
        &gradient(rect, &[(0, "FF0000"), (100000, "0000FF")]),
        "",
        "",
    );
    assert!(manifest.contains("approximated by a circular radial gradient"));
    let background = format!(
        "<p:bg><p:bgPr>{}<a:effectLst/></p:bgPr></p:bg>",
        gradient(r#"<a:lin ang="0"/>"#, &[(0, "FF0000"), (100000, "0000FF")])
    );
    let (pixels, manifest) = render("", &background, "");
    assert!(near(rgb(&pixels, 10, 50), mix(0.105)));
    assert!(!manifest.contains("Non-solid fills"), "{manifest}");
}

#[test]
fn rotated_gradients_report_rot_with_shape_and_tiles() {
    let fill = r#"<a:gradFill rotWithShape="0"><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst><a:lin ang="0"/><a:tileRect l="-10000"/></a:gradFill>"#;
    let (_, manifest) = render(fill, "", r#"rot="5400000""#);
    assert!(manifest.contains("rotWithShape"), "{manifest}");
    assert!(manifest.contains("tile rectangles"), "{manifest}");
    let (_, manifest) = render(fill, "", "");
    assert!(!manifest.contains("rotWithShape"), "{manifest}");
    let corner = r#"<a:path path="circle"><a:fillToRect l="100000" b="100000"/></a:path>"#;
    let (pixels, manifest) = render(
        &gradient(corner, &[(0, "FF0000"), (100000, "0000FF")]),
        "",
        "",
    );
    assert!(manifest.contains("approximated by a circular radial gradient"));
    assert!(
        near(rgb(&pixels, 99, 0), mix(0.0)),
        "{:?}",
        rgb(&pixels, 99, 0)
    );
    let wide = r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst><a:lin ang="2700000" scaled="1"/></a:gradFill>"#;
    let (_, manifest) = render(wide, "", "");
    assert!(
        !manifest.contains("Scaled gradient"),
        "square boxes are exact"
    );
}
