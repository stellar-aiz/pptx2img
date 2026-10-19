mod support;
use support::*;
fn render(kind: &str, extras: &str, attrs: &str) -> (serde_json::Value, resvg::tiny_skia::Pixmap) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("line.pptx");
    let o = d.path().join("out");
    let s = format!(
        r#"<p:cxnSp><p:spPr>{}<a:prstGeom prst="{kind}"/><a:ln w="19050" cap="flat"><a:solidFill><a:srgbClr val="000000"/></a:solidFill>{extras}</a:ln></p:spPr></p:cxnSp>"#,
        transform(20, 20, 60, 60).replace("<a:xfrm>", &format!("<a:xfrm {attrs}>"))
    );
    write_deck(&p, page("sld", "", "", &s), vec![]);
    check(run(&p, &o, false));
    (
        serde_json::from_slice(&std::fs::read(o.join("manifest.json")).unwrap()).unwrap(),
        image(&o),
    )
}
#[test]
fn connectors_keep_endpoints_bends_and_flip() {
    let (r, p) = render("bentConnector2", "", "");
    assert_eq!(rgb(&p, 50, 20), (0, 0, 0));
    assert_eq!(rgb(&p, 79, 50), (0, 0, 0));
    assert_eq!(rgb(&p, 50, 50), (255, 255, 255));
    assert!(
        !r["slides"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "omitted")
    );
    let (_, p) = render("bentConnector2", "", "flipH=\"1\"");
    assert_eq!(rgb(&p, 20, 50), (0, 0, 0));
    let (_, p) = render("straightConnector1", "", "");
    assert_eq!(rgb(&p, 50, 50), (0, 0, 0));
}
#[test]
fn dash_and_end_markers_change_visible_pixels_and_unknowns_warn() {
    let (_, solid) = render("straightConnector1", "", "");
    let (_, dash) = render("straightConnector1", r#"<a:prstDash val="dash"/>"#, "");
    assert_ne!(solid.data(), dash.data());
    for k in ["triangle", "stealth", "arrow", "diamond", "oval"] {
        let (r, p) = render(
            "straightConnector1",
            &format!(r#"<a:headEnd type="{k}" w="lg" len="lg"/><a:tailEnd type="{k}"/>"#),
            "",
        );
        assert_ne!(solid.data(), p.data(), "{k}");
        assert!(
            !r["slides"][0]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "omitted")
        );
    }
    let (r, _) = render(
        "straightConnector1",
        r#"<a:custDash/><a:tailEnd type="unknown"/>"#,
        "",
    );
    assert_eq!(r["slides"][0]["diagnostics"].as_array().unwrap().len(), 2);
}

fn themed_line(
    direct: &str,
    index: &str,
    theme_line: &str,
) -> (resvg::tiny_skia::Pixmap, serde_json::Value) {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("theme-line.pptx");
    let out = d.path().join("out");
    let shape = format!(
        r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="2" name="テーマ線"/></p:nvCxnSpPr><p:spPr>{}<a:prstGeom prst="straightConnector1"/>{direct}</p:spPr><p:style><a:lnRef idx="{index}"><a:schemeClr val="accent1"/></a:lnRef></p:style></p:cxnSp>"#,
        transform(20, 20, 60, 60)
    );
    let theme = format!(
        r#"<a:theme xmlns:a="{A}"><a:themeElements><a:clrScheme><a:accent1><a:srgbClr val="3366CC"/></a:accent1></a:clrScheme><a:fmtScheme><a:lnStyleLst>{theme_line}</a:lnStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
    );
    write_deck(
        &input,
        page("sld", "", "", &shape),
        vec![
            (
                "ppt/slides/_rels/slide1.xml.rels",
                rel("theme", "../theme/theme.xml").into_bytes(),
            ),
            ("ppt/theme/theme.xml", theme.into_bytes()),
        ],
    );
    check(run(&input, &out, false));
    (
        image(&out),
        serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap(),
    )
}
const THEME_LINE: &str = r#"<a:ln w="38100" cap="rnd"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="dash"/><a:round/><a:headEnd type="diamond"/><a:tailEnd type="triangle"/></a:ln>"#;
#[test]
fn theme_width_color_dash_and_ends_match_explicit_line() {
    let (themed, report) = themed_line("", "1", THEME_LINE);
    let explicit = THEME_LINE.replace(
        r#"<a:schemeClr val="phClr"/>"#,
        r#"<a:srgbClr val="3366CC"/>"#,
    );
    let (direct, _) = themed_line(&explicit, "0", THEME_LINE);
    assert_eq!(themed.data(), direct.data());
    assert!(
        themed
            .pixels()
            .iter()
            .any(|p| p.blue() > 150 && p.red() < 100)
    );
    assert!(
        !report["slides"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "unsupported")
    );
}
#[test]
fn direct_components_override_theme_without_losing_unspecified_ends() {
    let local = r#"<a:ln w="19050" cap="flat"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:prstDash val="solid"/><a:headEnd type="none"/><a:tailEnd len="lg"/></a:ln>"#;
    let explicit = r#"<a:ln w="19050" cap="flat"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:prstDash val="solid"/><a:round/><a:headEnd type="none"/><a:tailEnd type="triangle" len="lg"/></a:ln>"#;
    let (actual, _) = themed_line(local, "1", THEME_LINE);
    let (expected, _) = themed_line(explicit, "0", THEME_LINE);
    assert_eq!(actual.data(), expected.data());
    let (blank, _) = themed_line("<a:ln><a:noFill/></a:ln>", "1", THEME_LINE);
    assert!(
        blank
            .pixels()
            .iter()
            .all(|p| p.red() == 255 && p.green() == 255 && p.blue() == 255)
    );
}
#[test]
fn unresolved_reference_and_unsupported_inherited_properties_are_diagnosed() {
    for index in ["2", "not-a-number"] {
        let (image, report) = themed_line(
            r#"<a:ln w="19050"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:ln>"#,
            index,
            THEME_LINE,
        );
        assert!(
            image
                .pixels()
                .iter()
                .any(|p| p.red() > 200 && p.green() < 50)
        );
        let records = report["slides"][0]["diagnostics"].as_array().unwrap();
        assert!(records.iter().any(|d| {
            d["message"]
                .as_str()
                .unwrap()
                .contains("theme line reference")
                && d["object_id"] == "2"
        }));
    }
    let line = THEME_LINE
        .replace("<a:prstDash val=\"dash\"/>", "<a:custDash/>")
        .replace("cap=\"rnd\"", "cap=\"rnd\" cmpd=\"dbl\"");
    let (_, report) = themed_line("", "1", &line);
    let messages = report["slides"][0]["warnings"].as_array().unwrap();
    assert!(
        messages
            .iter()
            .any(|s| s.as_str().unwrap().contains("custDash"))
    );
    assert!(
        messages
            .iter()
            .any(|s| s.as_str().unwrap().contains("Compound lines"))
    );
}

#[test]
fn thin_line_arrow_sizes_keep_visible_direction_without_widening_the_line() {
    for (size, min_span) in [("sm", 3), ("med", 5), ("lg", 9)] {
        let mut previous_span = 0;
        for width in [3175, 6350, 9525, 12700, 19050, 25400, 38100, 50800, 76200] {
            let line = format!(
                r#"<a:ln w="{width}"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:tailEnd type="triangle" w="{size}" len="{size}"/></a:ln>"#
            );
            let without = line.replace(
                &format!(r#"<a:tailEnd type="triangle" w="{size}" len="{size}"/>"#),
                "",
            );
            let (actual, report) = themed_line(&line, "0", THEME_LINE);
            let (bare, _) = themed_line(&without, "0", THEME_LINE);
            let changed: Vec<_> = (60..86)
                .flat_map(|y| (60..86).map(move |x| (x, y)))
                .filter(|&(x, y)| actual.pixel(x, y) != bare.pixel(x, y))
                .collect();
            assert!(!changed.is_empty(), "{size} {width}");
            let span = changed
                .iter()
                .map(|(x, y)| *x as i32 - *y as i32)
                .max()
                .unwrap()
                - changed
                    .iter()
                    .map(|(x, y)| *x as i32 - *y as i32)
                    .min()
                    .unwrap();
            assert!(span >= min_span, "{size} {width}: {span}");
            assert!(
                span >= previous_span,
                "{size} {width}: {span} < {previous_span}"
            );
            previous_span = span;
            // Shaft thickness and color away from the head stay unchanged.
            for y in 30..50 {
                for x in 30..50 {
                    assert_eq!(actual.pixel(x, y), bare.pixel(x, y));
                }
            }
            assert!(
                report["slides"][0]["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "approximation")
            );
        }
    }
}

#[test]
fn bent_connector_rotation_after_flip_keeps_saved_bend_and_end_direction() {
    // ECMA preset: (l,t)->(r,t)->(r,b). Flip horizontally, then rotate 270 degrees.
    let (_, pixels) = render("bentConnector2", "", r#"rot="16200000" flipH="1""#);
    assert_eq!(rgb(&pixels, 20, 50), (0, 0, 0));
    assert_eq!(rgb(&pixels, 50, 79), (0, 0, 0));
    assert_eq!(rgb(&pixels, 50, 20), (255, 255, 255));
    assert_eq!(rgb(&pixels, 79, 50), (255, 255, 255));
    let (_, arrow) = render(
        "bentConnector2",
        r#"<a:tailEnd type="triangle"/>"#,
        r#"rot="16200000" flipH="1""#,
    );
    assert_eq!(rgb(&pixels, 75, 77), (255, 255, 255));
    assert!(rgb(&arrow, 75, 77).0 < 255);
}

#[test]
fn attached_connector_keeps_saved_pixels_and_reports_targeted_routing_limit() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("attached.pptx");
    let mut outputs = Vec::new();
    for attached in [false, true] {
        let links = if attached {
            r#"<a:stCxn id="10" idx="2"/><a:endCxn id="11" idx="1"/>"#
        } else {
            ""
        };
        let shape = format!(
            r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="7" name="接続線"/><p:cNvCxnSpPr>{links}</p:cNvCxnSpPr></p:nvCxnSpPr><p:spPr>{}<a:prstGeom prst="bentConnector2"/><a:ln w="19050"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln></p:spPr></p:cxnSp>"#,
            transform(20, 20, 60, 60)
        );
        write_deck(&input, page("sld", "", "", &shape), vec![]);
        let out = d.path().join(if attached { "attached" } else { "free" });
        check(run(&input, &out, false));
        outputs.push(image(&out));
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap();
        let records: Vec<_> = manifest["slides"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| {
                d["message"]
                    .as_str()
                    .unwrap()
                    .contains("Connector rerouting")
            })
            .collect();
        assert_eq!(records.len(), usize::from(attached));
        if attached {
            assert_eq!(records[0]["code"], "unsupported");
            assert_eq!(records[0]["object_id"], "7");
        }
    }
    assert_eq!(outputs[0].data(), outputs[1].data());
}
