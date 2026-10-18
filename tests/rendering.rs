//! 継承・複合図形・診断を、独立に生成した PPTX と画素で検証する。
use resvg::tiny_skia::{Color, Pixmap};
use std::{fs, io::Write, path::Path, process::Command};
use zip::{ZipWriter, write::SimpleFileOptions};
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PR: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
fn rel(kind: &str, target: &str) -> String {
    format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/{kind}" Target="{target}"/></Relationships>"#
    )
}
fn page(tag: &str, attrs: &str, background: &str, shapes: &str) -> String {
    format!(
        r#"<p:{tag} xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}" {attrs}><p:cSld>{background}<p:spTree>{shapes}</p:spTree></p:cSld></p:{tag}>"#
    )
}
fn transform(x: i32, y: i32, w: i32, h: i32) -> String {
    format!(
        r#"<a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        x * 9525,
        y * 9525,
        w * 9525,
        h * 9525
    )
}
fn rect(x: i32, y: i32, w: i32, h: i32, color: &str) -> String {
    format!(
        r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></p:spPr></p:sp>"#,
        transform(x, y, w, h)
    )
}
fn placeholder(index: u32, kind: &str, props: &str, text: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{index}" name="タイトル"/><p:nvPr><p:ph idx="{index}" type="{kind}"/></p:nvPr></p:nvSpPr><p:spPr>{props}</p:spPr>{text}</p:sp>"#
    )
}
fn write_deck(path: &Path, slide: String, extra: Vec<(&str, Vec<u8>)>) {
    let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
    let mut parts=vec![
        ("_rels/.rels",rel("officeDocument","ppt/presentation.xml").into_bytes()),
        ("ppt/presentation.xml",format!(r#"<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="r1"/></p:sldIdLst><p:sldSz cx="952500" cy="952500"/></p:presentation>"#).into_bytes()),
        ("ppt/_rels/presentation.xml.rels",rel("slide","slides/slide1.xml").into_bytes()),
        ("ppt/slides/slide1.xml",slide.into_bytes()),
    ];
    parts.extend(extra);
    for (name, data) in parts {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap();
}
fn run(path: &Path, out: &Path, strict: bool) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pptx2img"));
    cmd.arg(path)
        .arg("--out")
        .arg(out)
        .args(["--width", "100", "--no-system-fonts"])
        .arg("--font-file")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/metrics.ttf"
        ));
    if strict {
        cmd.arg("--strict");
    }
    cmd.output().unwrap()
}
fn image(out: &Path) -> Pixmap {
    Pixmap::load_png(out.join("slide-001.png")).unwrap()
}
fn rgb(image: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let p = image.pixel(x, y).unwrap();
    (p.red(), p.green(), p.blue())
}
fn check(output: std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn inherits_background_placeholder_and_stacks_master_layout_slide() {
    for hide in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.pptx");
        let out = dir.path().join("out");
        let master = page(
            "sldMaster",
            "",
            r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFF00"/></a:solidFill></p:bgPr></p:bg>"#,
            &(rect(0, 0, 10, 10, "0000FF")
                + &placeholder(
                    99,
                    "body",
                    &(transform(20, 20, 20, 20)
                        + r#"<a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>"#),
                    "",
                )),
        );
        let layout = page(
            "sldLayout",
            "",
            "",
            &(rect(0, 10, 10, 10, "FF0000") + &placeholder(7, "body", "", "")),
        );
        let slide = page(
            "sld",
            if hide { "showMasterSp=\"0\"" } else { "" },
            "",
            &placeholder(
                7,
                "body",
                r#"<a:solidFill><a:srgbClr val="00FF00"/></a:solidFill>"#,
                "",
            ),
        );
        write_deck(
            &input,
            slide,
            vec![
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    rel("slideLayout", "../layouts/layout.xml").into_bytes(),
                ),
                ("ppt/layouts/layout.xml", layout.into_bytes()),
                (
                    "ppt/layouts/_rels/layout.xml.rels",
                    rel("slideMaster", "../masters/master.xml").into_bytes(),
                ),
                ("ppt/masters/master.xml", master.into_bytes()),
            ],
        );
        check(run(&input, &out, true));
        let img = image(&out);
        assert_eq!(rgb(&img, 90, 90), (255, 255, 0));
        assert_eq!(rgb(&img, 25, 25), (0, 255, 0));
        assert_eq!(
            rgb(&img, 5, 5),
            if hide { (255, 255, 0) } else { (0, 0, 255) }
        );
        assert_eq!(
            rgb(&img, 5, 15),
            if hide { (255, 255, 0) } else { (255, 0, 0) }
        );
    }
}
#[test]
fn group_scales_child_coordinates_and_keeps_warning_identity() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let group = format!(
        r#"<p:grpSp><p:grpSpPr><a:xfrm flipH="1"><a:off x="190500" y="190500"/><a:ext cx="571500" cy="571500"/><a:chOff x="95250" y="95250"/><a:chExt cx="285750" cy="285750"/></a:xfrm></p:grpSpPr>{}<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="42" name="未対応グラフ"/></p:nvGraphicFramePr></p:graphicFrame></p:grpSp>"#,
        rect(10, 10, 10, 10, "FF0000")
    );
    write_deck(&input, page("sld", "", "", &group), vec![]);
    check(run(&input, &out, false));
    let img = image(&out);
    assert_eq!(rgb(&img, 70, 30), (255, 0, 0));
    assert_eq!(rgb(&img, 30, 30), (255, 255, 255));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let d = &manifest["slides"][0]["diagnostics"][0];
    assert_eq!(d["code"], "omitted");
    assert_eq!(d["object_id"], "42");
    assert_eq!(d["object_name"], "未対応グラフ");
    assert_eq!(d["part"], "ppt/slides/slide1.xml");
    assert!(d["location"].as_str().unwrap().starts_with("spTree/0/"));
    let strict = dir.path().join("strict");
    assert!(!run(&input, &strict, true).status.success());
    assert!(!strict.exists());
}
#[test]
fn renders_table_fills_and_merged_cell_without_drawing_covered_cell() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let frame = r#"<p:graphicFrame><p:xfrm><a:off x="95250" y="95250"/><a:ext cx="762000" cy="381000"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblGrid><a:gridCol w="381000"/><a:gridCol w="381000"/></a:tblGrid><a:tr h="190500"><a:tc gridSpan="2"><a:tcPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:tcPr></a:tc><a:tc hMerge="1"><a:tcPr><a:solidFill><a:srgbClr val="0000FF"/></a:solidFill></a:tcPr></a:tc></a:tr><a:tr h="190500"><a:tc><a:tcPr><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></a:tcPr></a:tc><a:tc><a:tcPr><a:solidFill><a:srgbClr val="0000FF"/></a:solidFill></a:tcPr></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#;
    write_deck(&input, page("sld", "", "", frame), vec![]);
    check(run(&input, &out, true));
    let img = image(&out);
    assert_eq!(rgb(&img, 20, 20), (255, 0, 0));
    assert_eq!(rgb(&img, 70, 20), (255, 0, 0));
    assert_eq!(rgb(&img, 20, 40), (0, 255, 0));
    assert_eq!(rgb(&img, 70, 40), (0, 0, 255));
}
#[test]
fn image_crop_clips_pixels_and_invalid_crop_leaves_no_output() {
    let mut png = Pixmap::new(20, 10).unwrap();
    png.fill(Color::from_rgba8(255, 0, 0, 255));
    for y in 0..10 {
        for x in 10..20 {
            let index = (y * 20 + x) * 4;
            png.data_mut()[index..index + 4].copy_from_slice(&[0, 0, 255, 255]);
        }
    }
    for (left, valid) in [(50000, true), (100000, false)] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.pptx");
        let out = dir.path().join("out");
        let picture = format!(
            r#"<p:pic><p:blipFill><a:blip r:embed="r1"/><a:srcRect l="{left}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
            transform(20, 20, 60, 60)
        );
        write_deck(
            &input,
            page("sld", "", "", &picture),
            vec![
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    rel("image", "../media/a.png").into_bytes(),
                ),
                ("ppt/media/a.png", png.encode_png().unwrap()),
            ],
        );
        let result = run(&input, &out, true);
        if valid {
            check(result);
            let img = image(&out);
            assert_eq!(rgb(&img, 30, 30), (0, 0, 255));
            assert_eq!(rgb(&img, 10, 30), (255, 255, 255));
        } else {
            assert!(!result.status.success());
            assert!(!out.exists());
        }
    }
}
#[test]
fn explicit_font_renders_with_no_system_fonts_and_records_actual_face() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let body = r#"<p:txBody><a:bodyPr lIns="0" tIns="0"/><a:p><a:r><a:rPr sz="2400"><a:latin typeface="Pptx2img Metrics Test"/></a:rPr><a:t>AA</a:t></a:r></a:p></p:txBody>"#;
    let shape = format!(
        "<p:sp><p:spPr>{}</p:spPr>{body}</p:sp>",
        transform(0, 0, 100, 100)
    );
    write_deck(&input, page("sld", "", "", &shape), vec![]);
    check(run(&input, &out, false));
    let img = image(&out);
    assert!(
        img.pixels().iter().filter(|p| p.red() < 128).count() > 100,
        "文字の画素がありません"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema_version"], 2);
    assert_eq!(manifest["system_fonts"], false);
    assert_eq!(manifest["fonts"][0]["family"], "Pptx2img Metrics Test");
}

#[test]
fn inherited_text_settings_match_explicit_settings_and_local_values_win() {
    let dir = tempfile::tempdir().unwrap();
    let body = |props: &str, run: &str| {
        format!(r#"<p:txBody><a:bodyPr {props}/><a:p>{run}</a:p></p:txBody>"#)
    };
    let text = r#"<a:r><a:rPr sz="1200"/><a:t>AA</a:t></a:r>"#;
    let defaults = r#"<a:pPr algn="r"><a:defRPr sz="2400"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:latin typeface="Pptx2img Metrics Test"/></a:defRPr></a:pPr>"#;
    let inherited = placeholder(7, "title", "", &body("", text));
    let explicit = placeholder(
        7,
        "title",
        &transform(10, 10, 80, 80),
        &body(
            "lIns=\"0\" rIns=\"0\" tIns=\"0\" bIns=\"0\" anchor=\"b\"",
            &(defaults.to_owned() + text),
        ),
    );
    for (name, shapes, extras) in [
        (
            "inherited",
            inherited,
            vec![
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    rel("slideLayout", "../layouts/layout.xml").into_bytes(),
                ),
                (
                    "ppt/layouts/layout.xml",
                    page(
                        "sldLayout",
                        "",
                        "",
                        &placeholder(
                            7,
                            "title",
                            &transform(10, 10, 80, 80),
                            &body(
                                "lIns=\"0\" rIns=\"0\" tIns=\"0\" bIns=\"0\" anchor=\"b\"",
                                defaults,
                            ),
                        ),
                    )
                    .into_bytes(),
                ),
            ],
        ),
        ("explicit", explicit, vec![]),
    ] {
        let input = dir.path().join(format!("{name}.pptx"));
        let out = dir.path().join(name);
        write_deck(&input, page("sld", "", "", &shapes), extras);
        check(run(&input, &out, false));
    }
    assert_eq!(
        image(&dir.path().join("inherited")).data(),
        image(&dir.path().join("explicit")).data()
    );
    let img = image(&dir.path().join("inherited"));
    assert!(img.pixels().iter().any(|p| p.red() > 200 && p.green() < 50));
}

#[test]
fn identical_warnings_on_two_objects_keep_two_diagnostic_targets() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let objects=(1..=2).map(|id|format!(r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="グラフ{id}"/></p:nvGraphicFramePr></p:graphicFrame>"#)).collect::<String>();
    write_deck(&input, page("sld", "", "", &objects), vec![]);
    check(run(&input, &out, false));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let slide = &manifest["slides"][0];
    assert_eq!(slide["warnings"].as_array().unwrap().len(), 1);
    assert_eq!(slide["diagnostics"].as_array().unwrap().len(), 2);
    assert_ne!(
        slide["diagnostics"][0]["object_id"],
        slide["diagnostics"][1]["object_id"]
    );
}

#[test]
fn invalid_table_merge_is_rejected_before_output() {
    for attrs in ["hMerge=\"1\"", "gridSpan=\"2\"", "rowSpan=\"0\""] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.pptx");
        let out = dir.path().join("out");
        let shape = format!(
            r#"<p:graphicFrame><p:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblGrid><a:gridCol w="952500"/></a:tblGrid><a:tr h="952500"><a:tc {attrs}/></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#
        );
        write_deck(&input, page("sld", "", "", &shape), vec![]);
        let result = run(&input, &out, false);
        assert!(!result.status.success());
        assert!(!out.exists());
        assert!(String::from_utf8_lossy(&result.stderr).contains("結合セル"));
    }
}

#[test]
fn nested_group_limit_keeps_valid_input_and_rejects_excess_before_output() {
    for depth in [64, 65, 66] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.pptx");
        let out = dir.path().join("out");
        let mut shape = rect(0, 0, 10, 10, "000000");
        for _ in 0..depth {
            shape = format!(
                r#"<p:grpSp><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/><a:chOff x="0" y="0"/><a:chExt cx="952500" cy="952500"/></a:xfrm></p:grpSpPr>{shape}</p:grpSp>"#
            );
        }
        write_deck(&input, page("sld", "", "", &shape), vec![]);
        let result = run(&input, &out, false);
        if depth == 64 {
            check(result);
            assert_eq!(rgb(&image(&out), 5, 5), (0, 0, 0));
        } else {
            // 通常のエラー終了を要求する。クラッシュを入力拒否として扱わない。
            assert_eq!(
                result.status.code(),
                Some(1),
                "終了状態: {:?}, 標準エラー: {}",
                result.status,
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(!out.exists());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains("64 段"),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}

#[test]
fn missing_font_and_glyph_are_reported_separately() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let body = r#"<p:txBody><a:bodyPr/><a:p><a:r><a:rPr><a:latin typeface="存在しない書体"/><a:ea typeface="存在しない書体"/></a:rPr><a:t>A未</a:t></a:r></a:p></p:txBody>"#;
    let shape = placeholder(7, "title", &transform(0, 0, 100, 100), body);
    write_deck(&input, page("sld", "", "", &shape), vec![]);
    check(run(&input, &out, false));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let records = manifest["slides"][0]["diagnostics"].as_array().unwrap();
    for code in ["font_substitution", "missing_glyph", "approximation"] {
        assert!(
            records
                .iter()
                .any(|d| d["code"] == code && d["object_id"] == "7")
        );
    }
    assert!(image(&out).pixels().iter().any(|p| p.red() < 128));
}

#[test]
fn rounded_rectangle_adjustment_and_empty_outline() {
    for (adjustment, corner_is_red) in [("val 0", true), ("val 50000", false)] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("rounded.pptx");
        let out = dir.path().join("out");
        let shapes = format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="{adjustment}"/></a:avLst></a:prstGeom><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:ln/></p:spPr></p:sp>"#,
            transform(10, 10, 80, 40)
        );
        write_deck(&input, page("sld", "", "", &shapes), vec![]);
        check(run(&input, &out, true));
        let rendered = image(&out);
        assert_eq!(rgb(&rendered, 50, 10), (255, 0, 0));
        assert_eq!(rgb(&rendered, 50, 30), (255, 0, 0));
        assert_eq!(rgb(&rendered, 11, 11) == (255, 0, 0), corner_is_red);
    }
}

#[test]
fn font_mapping_matches_api_and_cli_and_aggregates_targets() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("mapping.pptx");
    let body = r#"<p:txBody><a:bodyPr/><a:p><a:r><a:rPr><a:latin typeface="要求書体"/><a:ea typeface="要求書体"/></a:rPr><a:t>Aあ</a:t></a:r></a:p></p:txBody>"#;
    write_deck(
        &input,
        page(
            "sld",
            "",
            "",
            &(placeholder(7, "title", &transform(0, 0, 100, 40), body)
                + &placeholder(8, "title", &transform(0, 50, 100, 40), body)),
        ),
        vec![],
    );
    let font = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cjk.ttf");
    let cli_out = dir.path().join("cli");
    let cli = Command::new(env!("CARGO_BIN_EXE_pptx2img"))
        .arg(&input)
        .arg("--out")
        .arg(&cli_out)
        .args([
            "--width",
            "100",
            "--no-system-fonts",
            "--font-map",
            "要求書体=Pptx CJK",
            "--font-file",
        ])
        .arg(&font)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&cli.stderr);
    assert_eq!(
        stderr.matches("フォント代替: 要求書体").count(),
        1,
        "{stderr}"
    );
    check(cli);
    let api_out = dir.path().join("api");
    let report = pptx2img::convert(
        &input,
        &api_out,
        &pptx2img::ConvertOptions {
            width: 100,
            no_system_fonts: true,
            font_file: vec![font],
            font_mapping: std::collections::HashMap::from([("要求書体".into(), "Pptx CJK".into())]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        fs::read(cli_out.join("slide-001.png")).unwrap(),
        fs::read(api_out.join("slide-001.png")).unwrap()
    );
    let diagnostics: Vec<_> = report.slides[0]
        .diagnostics
        .iter()
        .filter(|d| d.font.is_some())
        .collect();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].font.as_ref().unwrap().reason,
        "explicit_mapping"
    );
    assert_eq!(diagnostics[0].object_id.as_deref(), Some("7"));
    assert_eq!(diagnostics[1].object_id.as_deref(), Some("8"));
}

#[test]
fn missing_glyph_strict_leaves_no_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("missing.pptx");
    let out = dir.path().join("strict");
    write_deck(
        &input,
        page(
            "sld",
            "",
            "",
            &placeholder(
                1,
                "title",
                &transform(0, 0, 100, 100),
                r#"<p:txBody><a:bodyPr/><a:p><a:r><a:rPr><a:ea typeface="Pptx2img Metrics Test"/></a:rPr><a:t>未</a:t></a:r></a:p></p:txBody>"#,
            ),
        ),
        vec![],
    );
    let output = run(&input, &out, true);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("U+672A"));
    assert!(!out.exists());
}

#[test]
fn shape_autofit_expands_fill_and_text_around_the_anchor() {
    for (anchor, y, top, bottom) in [("t", 20, 20, 68), ("ctr", 30, 11, 59), ("b", 60, 22, 70)] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("autofit.pptx");
        let out = dir.path().join("out");
        let shape = format!(
            r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></p:spPr><p:txBody><a:bodyPr lIns="0" rIns="0" tIns="0" bIns="0" anchor="{anchor}"><a:spAutoFit/></a:bodyPr><a:p><a:pPr><a:defRPr sz="1200"><a:latin typeface="Pptx2img Metrics Test"/></a:defRPr></a:pPr><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r><a:br/><a:r><a:t>A</a:t></a:r></a:p></p:txBody></p:sp>"#,
            transform(0, y, 100, 10)
        );
        write_deck(&input, page("sld", "", "", &shape), vec![]);
        check(run(&input, &out, false));
        let img = image(&out);
        assert_eq!(rgb(&img, 80, top), (0, 255, 0));
        assert_eq!(rgb(&img, 80, bottom - 1), (0, 255, 0));
        assert_eq!(rgb(&img, 80, top - 1), (255, 255, 255));
        assert_eq!(rgb(&img, 80, bottom), (255, 255, 255));
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
        assert!(
            !m["slides"][0]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "text_overflow")
        );
    }
}

#[test]
fn bullet_font_mapping_color_and_local_none_reach_pixels() {
    for disabled in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("bullet.pptx");
        let out = dir.path().join("out");
        let none = if disabled { "<a:buNone/>" } else { "" };
        let body = format!(
            r#"<p:txBody><a:bodyPr lIns="0" tIns="0" rIns="0" bIns="0"/><a:lstStyle><a:lvl1pPr marL="190500" indent="-190500"><a:buChar char="•"/><a:buFont typeface="要求記号書体"/><a:buSzPct val="150000"/><a:buClr><a:srgbClr val="0000FF"/></a:buClr><a:defRPr sz="1200"><a:latin typeface="Pptx2img Metrics Test"/></a:defRPr></a:lvl1pPr></a:lstStyle><a:p><a:pPr>{none}</a:pPr><a:r><a:t>AAAA AAAA AAAA</a:t></a:r></a:p></p:txBody>"#
        );
        write_deck(
            &input,
            page(
                "sld",
                "",
                "",
                &placeholder(1, "title", &transform(0, 0, 100, 100), &body),
            ),
            vec![],
        );
        let report = pptx2img::convert(
            &input,
            &out,
            &pptx2img::ConvertOptions {
                width: 100,
                no_system_fonts: true,
                font_file: vec![
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/metrics.ttf"),
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bullets.ttf"),
                ],
                font_mapping: std::collections::HashMap::from([(
                    "要求記号書体".into(),
                    "Pptx Bullets".into(),
                )]),
                ..Default::default()
            },
        )
        .unwrap();
        let img = image(&out);
        let mut blue = 0;
        for y in 0..100 {
            for x in 0..100 {
                let (r, g, b) = rgb(&img, x, y);
                if b > 200 && r < 100 && g < 100 {
                    blue += 1;
                    assert!(x < 20);
                }
            }
        }
        assert_eq!(blue > 0, !disabled);
        assert_eq!(
            report.slides[0].diagnostics.iter().any(|d| d
                .font
                .as_ref()
                .is_some_and(
                    |f| f.requested_family == "要求記号書体" && f.reason == "explicit_mapping"
                )),
            !disabled
        );
    }
}
