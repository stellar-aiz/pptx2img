mod support;
use support::*;
const STYLE: &str = r#"<a:tblStyle styleId="own"><a:wholeTbl><a:tcTxStyle b="on"><a:srgbClr val="FFFFFF"/></a:tcTxStyle><a:tcStyle><a:fill><a:solidFill><a:srgbClr val="245A51"/></a:solidFill></a:fill></a:tcStyle></a:wholeTbl><a:firstRow><a:tcStyle><a:fill><a:solidFill><a:srgbClr val="C37B49"/></a:solidFill></a:fill></a:tcStyle></a:firstRow><a:band1H><a:tcStyle><a:fill><a:solidFill><a:srgbClr val="112233"/></a:solidFill></a:fill></a:tcStyle></a:band1H></a:tblStyle>"#;
fn render(
    styled: bool,
    flags: &str,
    id: &str,
    explicit: &str,
) -> (resvg::tiny_skia::Pixmap, serde_json::Value) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("table.pptx");
    let out = d.path().join("out");
    let mut rows = String::new();
    for row in 0..3 {
        let fill = if styled {
            explicit.to_owned()
        } else {
            r#"<a:solidFill><a:srgbClr val="245A51"/></a:solidFill>"#.into()
        };
        let props = if styled { "" } else { r#" b="1""# };
        let color = if styled {
            ""
        } else {
            r#"<a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill>"#
        };
        let body = format!(
            r#"<a:txBody><a:bodyPr/><a:p><a:pPr><a:defRPr{props}>{color}</a:defRPr></a:pPr><a:r><a:rPr sz="600"><a:latin typeface="Metrics Test"/></a:rPr><a:t>A</a:t></a:r></a:p></a:txBody>"#
        );
        rows.push_str(&format!(
            r#"<a:tr h="190500"><a:tc>{body}<a:tcPr>{fill}</a:tcPr></a:tc></a:tr>"#
        ));
        let _ = row;
    }
    let style = if styled {
        format!("<a:tableStyleId>{id}</a:tableStyleId>")
    } else {
        String::new()
    };
    let shapes = format!(
        r#"<p:graphicFrame><p:xfrm><a:off x="190500" y="190500"/><a:ext cx="571500" cy="571500"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblPr {flags}>{style}</a:tblPr><a:tblGrid><a:gridCol w="571500"/></a:tblGrid>{rows}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#
    );
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="r2" Type="{R}/tableStyles" Target="tableStyles.xml"/></Relationships>"#
    );
    let styles = format!(r#"<a:tblStyleLst xmlns:a="{A}">{STYLE}</a:tblStyleLst>"#);
    write_deck(
        &path,
        page("sld", "", "", &shapes),
        vec![
            ("ppt/_rels/presentation.xml.rels", rels.into_bytes()),
            ("ppt/tableStyles.xml", styles.into_bytes()),
        ],
    );
    check(run(&path, &out, false));
    (
        image(&out),
        serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap(),
    )
}
#[test]
fn style_fill_and_text_match_explicit_formatting() {
    let (a, _) = render(true, "", "own", "");
    let (b, _) = render(false, "", "", "");
    assert_eq!(a.data(), b.data());
}
#[test]
fn first_row_and_banding_switch_only_selected_regions() {
    let (a, _) = render(true, "firstRow=\"1\" bandRow=\"1\"", "own", "");
    assert_eq!(rgb(&a, 70, 30), (195, 123, 73));
    assert_eq!(rgb(&a, 70, 50), (17, 34, 51));
    assert_eq!(rgb(&a, 70, 70), (36, 90, 81));
    let (b, _) = render(true, "", "own", "");
    assert_eq!(rgb(&b, 70, 30), (36, 90, 81));
}
#[test]
fn explicit_no_fill_wins_and_unknown_style_is_diagnosed() {
    let (a, _) = render(true, "firstRow=\"1\"", "own", "<a:noFill/>");
    assert_eq!(rgb(&a, 70, 30), (255, 255, 255));
    let (_, report) = render(true, "", "missing", "");
    assert!(
        report["slides"][0]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("missing"))
    );
}
