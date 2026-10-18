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
