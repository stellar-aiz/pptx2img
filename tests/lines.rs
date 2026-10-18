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
