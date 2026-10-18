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
        let defaults = r#"<a:defRPr sz="1200"><a:latin typeface="Pptx2img Metrics Test"/><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:defRPr>"#;
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
