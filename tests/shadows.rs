mod support;
use support::*;
fn render(
    attrs: &str,
    transform_attrs: &str,
    extra: &str,
) -> (resvg::tiny_skia::Pixmap, serde_json::Value) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("shadow.pptx");
    let out = d.path().join("out");
    let shape = format!(
        r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:effectLst><a:outerShdw {attrs}><a:prstClr val="black"><a:alpha val="50000"/></a:prstClr></a:outerShdw><a:glow/></a:effectLst></p:spPr></p:sp>{extra}"#,
        transform(20, 20, 30, 30).replace("<a:xfrm>", &format!("<a:xfrm {transform_attrs}>"))
    );
    write_deck(&p, page("sld", "", "", &shape), vec![]);
    check(run(&p, &out, false));
    (
        image(&out),
        serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap(),
    )
}
#[test]
fn shadow_direction_opacity_rotation_and_stacking_are_preserved() {
    let (p, r) = render(
        "dist=\"95250\" dir=\"0\" rotWithShape=\"0\"",
        "rot=\"5400000\" flipH=\"1\"",
        "",
    );
    let c = rgb(&p, 55, 30);
    assert!((126..=129).contains(&c.0), "pixel={c:?}");
    assert_eq!(c.0, c.1);
    assert_eq!(rgb(&p, 30, 30), (255, 0, 0));
    assert_eq!(rgb(&p, 30, 55), (255, 255, 255));
    assert!(
        r["slides"][0]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("glow"))
    );
    let (p, _) = render(
        "dist=\"95250\" dir=\"0\" rotWithShape=\"1\"",
        "rot=\"5400000\"",
        "",
    );
    assert!(rgb(&p, 30, 55).0 < 200);
    let (p, _) = render(
        "dist=\"95250\" dir=\"0\"",
        "",
        &rect(50, 20, 20, 20, "0000FF"),
    );
    assert_eq!(rgb(&p, 55, 30), (0, 0, 255));
}
#[test]
fn blur_spreads_shadow_and_keeps_approximation_diagnostic() {
    let (p, r) = render("blurRad=\"76200\" dist=\"95250\" dir=\"0\"", "", "");
    assert!(rgb(&p, 62, 35).0 < 255);
    assert!(
        r["slides"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "approximation")
    );
}

#[test]
fn autofit_shadow_and_rotated_geometry_match_explicit_final_dimensions() {
    let d = tempfile::tempdir().unwrap();
    for follows in [0, 1] {
        for fitted in [false, true] {
            let mode = if fitted { "spAutoFit" } else { "noAutofit" };
            let (y, h) = if fitted { (35, 10) } else { (16, 48) };
            let xfrm =
                transform(20, y, 60, h).replace("<a:xfrm>", r#"<a:xfrm rot="1800000" flipH="1">"#);
            let shape = format!(
                r#"<p:sp><p:spPr>{xfrm}<a:prstGeom prst="rightArrow"/><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill><a:effectLst><a:outerShdw blurRad="9525" dist="95250" dir="5400000" rotWithShape="{follows}"><a:srgbClr val="000000"><a:alpha val="50000"/></a:srgbClr></a:outerShdw></a:effectLst></p:spPr><p:txBody><a:bodyPr lIns="0" rIns="0" tIns="0" bIns="0" anchor="ctr"><a:{mode}/></a:bodyPr><a:p><a:pPr><a:defRPr sz="600"><a:latin typeface="Rhodopsin Metrics Test"/></a:defRPr></a:pPr><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r></a:p></p:txBody></p:sp>"#
            );
            let input = d.path().join(format!("{follows}-{fitted}.pptx"));
            let out = d.path().join(format!("{follows}-{fitted}"));
            write_deck(&input, page("sld", "", "", &shape), vec![]);
            check(run(&input, &out, false));
            assert!(
                image(&out)
                    .pixels()
                    .iter()
                    .filter(|p| (120..200).contains(&p.red())
                        && p.red() == p.green()
                        && p.green() == p.blue())
                    .count()
                    > 10
            );
        }
        assert_eq!(
            image(&d.path().join(format!("{follows}-false"))).data(),
            image(&d.path().join(format!("{follows}-true"))).data()
        );
    }
}
