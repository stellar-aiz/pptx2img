mod support;
use support::*;
#[test]
fn theme_fill_matches_direct_color_and_explicit_values_win() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("colors.pptx");
    let out = d.path().join("out");
    let shape = |x, explicit: &str| {
        format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/>{explicit}</p:spPr><p:style><a:fillRef idx="1"><a:schemeClr val="accent1"><a:lumMod val="50000"/><a:lumOff val="50000"/></a:schemeClr></a:fillRef></p:style></p:sp>"#,
            transform(x, 10, 20, 30)
        )
    };
    let shapes = shape(0, "")
        + &shape(
            25,
            r#"<a:solidFill><a:srgbClr val="8080FF"/></a:solidFill>"#,
        )
        + &shape(50, "<a:noFill/>")
        + &shape(
            75,
            r#"<a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>"#,
        );
    let theme = format!(
        r#"<a:theme xmlns:a="{A}"><a:themeElements><a:clrScheme><a:accent1><a:srgbClr val="0000FF"/></a:accent1></a:clrScheme><a:fmtScheme><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
    );
    write_deck(
        &p,
        page("sld", "", "", &shapes),
        vec![
            (
                "ppt/slides/_rels/slide1.xml.rels",
                rel("theme", "../theme/theme.xml").into_bytes(),
            ),
            ("ppt/theme/theme.xml", theme.into_bytes()),
        ],
    );
    check(run(&p, &out, true));
    let pixels = image(&out);
    assert_eq!(rgb(&pixels, 10, 20), (128, 128, 255));
    assert_eq!(rgb(&pixels, 10, 20), rgb(&pixels, 35, 20));
    assert_eq!(rgb(&pixels, 60, 20), (255, 255, 255));
    assert_eq!(rgb(&pixels, 85, 20), (255, 0, 0));
}

#[test]
fn presentation_text_defaults_and_theme_fill_survive_together() {
    let d = tempfile::tempdir().unwrap();
    for explicit in [false, true] {
        let input = d.path().join(format!("{explicit}.pptx"));
        let out = d.path().join(format!("{explicit}"));
        let defaults = r#"<a:defRPr sz="1200"><a:latin typeface="Rhodopsin Metrics Test"/><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:defRPr>"#;
        let local = if explicit { defaults } else { "" };
        let shape = format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="既定本文"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"/></p:spPr><p:style><a:fillRef idx="1"><a:schemeClr val="accent2"/></a:fillRef></p:style><p:txBody><a:bodyPr lIns="0" rIns="0" tIns="0" bIns="0"/><a:p><a:pPr>{local}</a:pPr><a:r><a:t>AAAA</a:t></a:r></a:p></p:txBody></p:sp>"#,
            transform(10, 10, 80, 40)
        );
        let presentation = format!(
            r#"<p:presentation xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="r1"/></p:sldIdLst><p:sldSz cx="952500" cy="952500"/><p:defaultTextStyle><a:lvl1pPr>{defaults}</a:lvl1pPr></p:defaultTextStyle></p:presentation>"#
        );
        let theme = format!(
            r#"<a:theme xmlns:a="{A}"><a:themeElements><a:clrScheme><a:accent1><a:srgbClr val="FF0000"/></a:accent1><a:accent2><a:srgbClr val="00FF00"/></a:accent2></a:clrScheme><a:fmtScheme><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
        );
        write_deck(
            &input,
            page("sld", "", "", &shape),
            vec![
                ("ppt/presentation.xml", presentation.into_bytes()),
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    rel("theme", "../theme/theme.xml").into_bytes(),
                ),
                ("ppt/theme/theme.xml", theme.into_bytes()),
            ],
        );
        check(run(&input, &out, false));
        let image = image(&out);
        assert_eq!(rgb(&image, 80, 40), (0, 255, 0));
        assert!(
            image
                .pixels()
                .iter()
                .any(|p| p.red() > 200 && p.green() < 20 && p.blue() < 20)
        );
    }
    assert_eq!(
        image(&d.path().join("false")).data(),
        image(&d.path().join("true")).data()
    );
}

#[test]
fn alpha_blends_fills_and_lines_over_the_background() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("alpha.pptx");
    let out = d.path().join("out");
    let translucent = |x, fill: &str, line: &str| {
        format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/>{fill}{line}</p:spPr></p:sp>"#,
            transform(x, 10, 30, 80)
        )
    };
    let shapes = rect(0, 0, 100, 100, "000000")
        + &translucent(
            5,
            r#"<a:solidFill><a:srgbClr val="FFFFFF"><a:alpha val="50000"/></a:srgbClr></a:solidFill>"#,
            "<a:ln><a:noFill/></a:ln>",
        )
        + &translucent(
            50,
            "<a:noFill/>",
            r#"<a:ln w="190500"><a:solidFill><a:srgbClr val="FFFFFF"><a:alpha val="50000"/></a:srgbClr></a:solidFill></a:ln>"#,
        );
    write_deck(&p, page("sld", "", "", &shapes), vec![]);
    check(run(&p, &out, true));
    let pixels = image(&out);
    let (r, g, b) = rgb(&pixels, 20, 50);
    for channel in [r, g, b] {
        assert!(channel.abs_diff(128) <= 2, "fill blend {r},{g},{b}");
    }
    let (r, g, b) = rgb(&pixels, 50, 50);
    for channel in [r, g, b] {
        assert!(channel.abs_diff(128) <= 2, "line blend {r},{g},{b}");
    }
    assert_eq!(rgb(&pixels, 65, 50), (0, 0, 0));
}

#[test]
fn tint_and_shade_fill_shapes_and_table_style_bands() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("tint.pptx");
    let out = d.path().join("out");
    let shaded = |x, transform_xml: &str| {
        format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/><a:solidFill><a:schemeClr val="accent1">{transform_xml}</a:schemeClr></a:solidFill></p:spPr></p:sp>"#,
            transform(x, 0, 20, 40)
        )
    };
    let rows = (0..2)
        .map(|_| r#"<a:tr h="190500"><a:tc><a:txBody><a:bodyPr/><a:p><a:endParaRPr/></a:p></a:txBody><a:tcPr/></a:tc></a:tr>"#)
        .collect::<String>();
    let shapes = shaded(0, r#"<a:tint val="20000"/>"#)
        + &shaded(30, r#"<a:shade val="50000"/>"#)
        + &format!(
            r#"<p:graphicFrame><p:xfrm><a:off x="0" y="476250"/><a:ext cx="952500" cy="381000"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblPr firstRow="1" bandRow="1"><a:tableStyleId>{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}</a:tableStyleId></a:tblPr><a:tblGrid><a:gridCol w="952500"/></a:tblGrid>{rows}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#
        );
    let style = format!(
        r#"<a:tblStyleLst xmlns:a="{A}"><a:tblStyle styleId="{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}" styleName="Medium Style 2 - Accent 1"><a:wholeTbl><a:tcStyle><a:fill><a:solidFill><a:schemeClr val="accent1"><a:tint val="20000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:wholeTbl><a:firstRow><a:tcStyle><a:fill><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:fill></a:tcStyle></a:firstRow></a:tblStyle></a:tblStyleLst>"#
    );
    let theme = format!(
        r#"<a:theme xmlns:a="{A}"><a:themeElements><a:clrScheme><a:accent1><a:srgbClr val="4472C4"/></a:accent1></a:clrScheme></a:themeElements></a:theme>"#
    );
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="r2" Type="{R}/tableStyles" Target="tableStyles.xml"/></Relationships>"#
    );
    write_deck(
        &p,
        page("sld", "", "", &shapes),
        vec![
            ("ppt/_rels/presentation.xml.rels", rels.into_bytes()),
            ("ppt/tableStyles.xml", style.into_bytes()),
            (
                "ppt/slides/_rels/slide1.xml.rels",
                rel("theme", "../theme/theme.xml").into_bytes(),
            ),
            ("ppt/theme/theme.xml", theme.into_bytes()),
        ],
    );
    let output = run(&p, &out, false);
    check(output);
    let pixels = image(&out);
    assert_eq!(rgb(&pixels, 10, 20), (0xE9, 0xEB, 0xF5));
    assert_eq!(rgb(&pixels, 40, 20), (0x2F, 0x52, 0x8F));
    // Header row keeps the accent; the body row uses the 20% tint, not the saturated accent.
    // Rows grow to fit their text, so locate the body row from the rendered geometry.
    assert_eq!(rgb(&pixels, 50, 60), (0x44, 0x72, 0xC4));
    let header_end = (60..pixels.height())
        .find(|&y| rgb(&pixels, 50, y) != (0x44, 0x72, 0xC4))
        .expect("the body row is below the header row");
    assert_eq!(rgb(&pixels, 50, header_end + 2), (0xE9, 0xEB, 0xF5));
    let manifest = std::fs::read_to_string(out.join("manifest.json")).unwrap();
    assert!(
        !manifest.contains("Unsupported color transform"),
        "{manifest}"
    );
}

const THEME_WITH_BACKGROUNDS: &str = r#"<a:themeElements><a:clrScheme><a:accent1><a:srgbClr val="00AA00"/></a:accent1><a:accent2><a:srgbClr val="0000FF"/></a:accent2></a:clrScheme><a:fmtScheme><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:lumMod val="50000"/></a:schemeClr></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements>"#;
/// Render `shapes` on a slide whose layout and master come from `layout_bg` and `master_bg`;
/// the master links a theme with `bgFillStyleLst`. Extra parts (media) may be added.
fn render_with_master(
    slide_bg: &str,
    layout_bg: &str,
    master_bg: &str,
    shapes: &str,
    extra: Vec<(&str, Vec<u8>)>,
    strict: bool,
) -> (resvg::tiny_skia::Pixmap, serde_json::Value) {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("fills.pptx");
    let out = d.path().join("out");
    let theme = format!(r#"<a:theme xmlns:a="{A}">{THEME_WITH_BACKGROUNDS}</a:theme>"#);
    let mut parts = vec![
        (
            "ppt/slides/_rels/slide1.xml.rels",
            rel("slideLayout", "../layouts/layout.xml").into_bytes(),
        ),
        (
            "ppt/layouts/layout.xml",
            page("sldLayout", "", layout_bg, "").into_bytes(),
        ),
        (
            "ppt/layouts/_rels/layout.xml.rels",
            rel("slideMaster", "../masters/master.xml").into_bytes(),
        ),
        (
            "ppt/masters/master.xml",
            page("sldMaster", "", master_bg, "").into_bytes(),
        ),
        (
            "ppt/masters/_rels/master.xml.rels",
            rel("theme", "../theme/theme.xml").into_bytes(),
        ),
        ("ppt/theme/theme.xml", theme.into_bytes()),
    ];
    parts.extend(extra);
    write_deck(&input, page("sld", "", slide_bg, shapes), parts);
    check(run(&input, &out, strict));
    let manifest =
        serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap();
    (image(&out), manifest)
}
fn messages(manifest: &serde_json::Value) -> Vec<String> {
    manifest["slides"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["message"].as_str().unwrap().to_string())
        .collect()
}
#[test]
fn theme_background_reference_paints_the_referenced_color() {
    let master_bg = r#"<p:bg><p:bgRef idx="1001"><a:schemeClr val="accent1"/></p:bgRef></p:bg>"#;
    let (pixels, manifest) = render_with_master("", "", master_bg, "", vec![], true);
    assert_eq!(rgb(&pixels, 50, 50), (0, 170, 0));
    assert!(messages(&manifest).is_empty());
    // A layout reference overrides the master; idx 1002 applies the style's own transform.
    let layout_bg = r#"<p:bg><p:bgRef idx="1002"><a:schemeClr val="accent2"/></p:bgRef></p:bg>"#;
    let (pixels, _) = render_with_master("", layout_bg, master_bg, "", vec![], true);
    let (r, g, b) = rgb(&pixels, 50, 50);
    assert!(r == 0 && g == 0 && b > 0 && b < 255, "{:?}", (r, g, b));
}
#[test]
fn pattern_fills_draw_both_colors_and_name_unknown_presets() {
    let shape = |x: i32, preset: &str| {
        format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/><a:pattFill prst="{preset}"><a:fgClr><a:srgbClr val="FF0000"/></a:fgClr><a:bgClr><a:srgbClr val="FFFFFF"/></a:bgClr></a:pattFill></p:spPr></p:sp>"#,
            transform(x, 10, 30, 80)
        )
    };
    let shapes = shape(0, "dkUpDiag") + &shape(34, "trellis") + &shape(68, "zigZag");
    let (pixels, manifest) = render_with_master("", "", "", &shapes, vec![], false);
    for left in [2, 36, 70] {
        let colors: std::collections::BTreeSet<_> = (left..left + 26)
            .flat_map(|x| (12..88).map(move |y| (x, y)))
            .map(|(x, y)| rgb(&pixels, x, y))
            .collect();
        assert!(
            colors.contains(&(255, 0, 0)) && colors.contains(&(255, 255, 255)),
            "shape at {left} is not a two-color pattern"
        );
    }
    let diagnostics = manifest["slides"][0]["diagnostics"].as_array().unwrap();
    let unsupported: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"] == "unsupported")
        .map(|d| d["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        unsupported,
        ["Unsupported pattern fill preset: zigZag; drawn as pct50"]
    );
    // Every pattern, drawn or not, is flagged as approximate.
    let approximate = diagnostics
        .iter()
        .filter(|d| d["code"] == "approximation")
        .count();
    assert_eq!(approximate, 3);
}
#[test]
fn group_fill_reaches_children_including_nested_groups() {
    let child = |x: i32, y: i32| {
        format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/><a:grpFill/></p:spPr></p:sp>"#,
            transform(x, y, 20, 20)
        )
    };
    let group = |props: &str, children: &str| {
        format!(
            r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="9" name="group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/><a:chOff x="0" y="0"/><a:chExt cx="952500" cy="952500"/></a:xfrm>{props}</p:grpSpPr>{children}</p:grpSp>"#
        )
    };
    let blue = r#"<a:solidFill><a:srgbClr val="0000FF"/></a:solidFill>"#;
    let nested = group("<a:grpFill/>", &child(50, 10)) + &group("<a:noFill/>", &child(50, 50));
    let shapes = group(blue, &(child(10, 10) + &nested)) + &child(10, 50);
    let (pixels, manifest) = render_with_master("", "", "", &shapes, vec![], true);
    assert_eq!(rgb(&pixels, 20, 20), (0, 0, 255));
    assert_eq!(rgb(&pixels, 60, 20), (0, 0, 255));
    // A group without a fill, and a shape outside any group, leave grpFill children unfilled.
    assert_eq!(rgb(&pixels, 60, 60), (255, 255, 255));
    assert_eq!(rgb(&pixels, 20, 60), (255, 255, 255));
    assert!(messages(&manifest).is_empty());
}
fn red_png() -> Vec<u8> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(4, 4).unwrap();
    pixmap.fill(resvg::tiny_skia::Color::from_rgba8(255, 0, 0, 255));
    pixmap.encode_png().unwrap()
}
#[test]
fn picture_fill_is_stretched_and_clipped_to_the_shape_outline() {
    let shape = format!(
        r#"<p:sp><p:spPr>{}<a:prstGeom prst="ellipse"/><a:blipFill><a:blip r:embed="r2"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></p:spPr></p:sp>"#,
        transform(10, 10, 80, 80)
    );
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slideLayout" Target="../layouts/layout.xml"/><Relationship Id="r2" Type="{R}/image" Target="../media/red.png"/></Relationships>"#
    );
    let (pixels, manifest) = render_with_master(
        "",
        "",
        "",
        &shape,
        vec![
            ("ppt/slides/_rels/slide1.xml.rels", rels.into_bytes()),
            ("ppt/media/red.png", red_png()),
        ],
        true,
    );
    assert_eq!(rgb(&pixels, 50, 50), (255, 0, 0));
    assert_eq!(rgb(&pixels, 12, 12), (255, 255, 255));
    assert!(messages(&manifest).is_empty());
}
#[test]
fn picture_background_comes_from_the_part_that_declares_it() {
    let layout_bg = r#"<p:bg><p:bgPr><a:blipFill><a:blip r:embed="r2"/><a:stretch><a:fillRect/></a:stretch></a:blipFill><a:effectLst/></p:bgPr></p:bg>"#;
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slideMaster" Target="../masters/master.xml"/><Relationship Id="r2" Type="{R}/image" Target="../media/red.png"/></Relationships>"#
    );
    let (pixels, manifest) = render_with_master(
        "",
        layout_bg,
        "",
        "",
        vec![
            ("ppt/layouts/_rels/layout.xml.rels", rels.into_bytes()),
            ("ppt/media/red.png", red_png()),
        ],
        true,
    );
    assert_eq!(rgb(&pixels, 50, 50), (255, 0, 0));
    assert!(messages(&manifest).is_empty());
}
