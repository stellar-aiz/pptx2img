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

#[test]
fn table_font_reference_mapping_and_cell_margins_match_explicit_textbox() {
    let d = tempfile::tempdir().unwrap();
    for table in [false, true] {
        let input = d.path().join(format!("{table}.pptx"));
        let out = d.path().join(format!("{table}"));
        let defaults = if table {
            ""
        } else {
            r#"<a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:solidFill><a:srgbClr val="000000"/></a:solidFill>"#
        };
        let body = format!(
            r#"<a:bodyPr lIns="47625" rIns="66675" tIns="28575" bIns="38100"><a:spAutoFit/></a:bodyPr><a:p><a:pPr marL="95250" marR="47625"><a:defRPr sz="1200">{defaults}</a:defRPr></a:pPr><a:r><a:t>Aあ Aあ Aあ</a:t></a:r><a:r><a:rPr><a:latin typeface="Pptx Other"/><a:solidFill><a:srgbClr val="0000FF"/></a:solidFill></a:rPr><a:t>A</a:t></a:r></a:p>"#
        );
        let shape = if table {
            format!(
                r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="2" name="表"/></p:nvGraphicFramePr><p:xfrm><a:off x="95250" y="95250"/><a:ext cx="762000" cy="571500"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblPr><a:tableStyleId>join</a:tableStyleId></a:tblPr><a:tblGrid><a:gridCol w="762000"/></a:tblGrid><a:tr h="571500"><a:tc><a:txBody>{body}</a:txBody><a:tcPr marL="47625" marR="66675" marT="28575" marB="38100"/></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#
            )
        } else {
            format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="本文"/></p:nvSpPr><p:spPr>{}</p:spPr><p:txBody>{body}</p:txBody></p:sp>"#,
                transform(10, 10, 80, 60)
            )
        };
        let presentation_rels = format!(
            r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="r2" Type="{R}/tableStyles" Target="tableStyles.xml"/></Relationships>"#
        );
        let theme = format!(
            r#"<a:theme xmlns:a="{A}"><a:themeElements><a:fontScheme><a:minorFont><a:latin typeface="要求欧文"/><a:ea typeface="要求和文"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#
        );
        let styles = format!(
            r#"<a:tblStyleLst xmlns:a="{A}"><a:tblStyle styleId="join"><a:wholeTbl><a:tcTxStyle><a:fontRef idx="minor"/><a:srgbClr val="000000"/></a:tcTxStyle></a:wholeTbl></a:tblStyle></a:tblStyleLst>"#
        );
        write_deck(
            &input,
            page("sld", "", "", &shape),
            vec![
                (
                    "ppt/_rels/presentation.xml.rels",
                    presentation_rels.into_bytes(),
                ),
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    rel("theme", "../theme/theme.xml").into_bytes(),
                ),
                ("ppt/theme/theme.xml", theme.into_bytes()),
                ("ppt/tableStyles.xml", styles.into_bytes()),
            ],
        );
        let report = pptx2img::convert(
            &input,
            &out,
            &pptx2img::ConvertOptions {
                font_fallback: true,
                width: 100,
                no_system_fonts: true,
                font_file: ["latin", "cjk", "other"]
                    .into_iter()
                    .map(|f| {
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join(format!("tests/fixtures/{f}.ttf"))
                    })
                    .collect(),
                font_mapping: std::collections::HashMap::from([
                    ("要求欧文".into(), "Pptx Latin".into()),
                    ("要求和文".into(), "Pptx CJK".into()),
                ]),
                ..Default::default()
            },
        )
        .unwrap();
        let diagnostics = &report.slides[0].diagnostics;
        for (requested, actual) in [("要求欧文", "Pptx Latin"), ("要求和文", "Pptx CJK")] {
            let record = diagnostics
                .iter()
                .find(|d| {
                    d.font.as_ref().is_some_and(|f| {
                        f.requested_family == requested && f.actual_family == actual
                    })
                })
                .unwrap();
            assert_eq!(record.object_id.as_deref(), Some("2"));
            assert_eq!(record.location.contains("/row/1/cell/1"), table);
        }
        assert_eq!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("表セル・縦書きの自動枠拡張")),
            table
        );
        assert!(
            image(&out)
                .pixels()
                .iter()
                .any(|p| p.blue() > 200 && p.red() < 20)
        );
    }
    assert_eq!(
        image(&d.path().join("false")).data(),
        image(&d.path().join("true")).data()
    );
}
