//! Verify the CLI contract using real ZIP/XML inputs and PNG pixels.
use std::{fs, io::Write, path::Path, process::Command};
use zip::{ZipWriter, write::SimpleFileOptions};
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PR: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
fn fixture(path: &Path, extra: &str) {
    let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
    let mut add = |name: &str, value: String| {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(value.as_bytes()).unwrap();
    };
    add(
        "_rels/.rels",
        format!(
            r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#
        ),
    );
    // Verify presentation order when it differs from filename order.
    add(
        "ppt/presentation.xml",
        format!(
            r#"<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="r2"/><p:sldId id="257" r:id="r1"/></p:sldIdLst><p:sldSz cx="952500" cy="952500"/></p:presentation>"#
        ),
    );
    add(
        "ppt/_rels/presentation.xml.rels",
        format!(
            r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="r2" Type="{R}/slide" Target="slides/slide2.xml"/></Relationships>"#
        ),
    );
    for (index, color) in [(1, "FF0000"), (2, "0000FF")] {
        add(
            &format!("ppt/slides/slide{index}.xml"),
            format!(
                r#"<p:sld xmlns:p="{P}" xmlns:a="{A}"><p:cSld><p:spTree><p:sp><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></p:spPr></p:sp>{extra}</p:spTree></p:cSld></p:sld>"#
            ),
        );
    }
    zip.finish().unwrap();
}
fn run(input: &Path, output: &Path, strict: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rhodopsin"));
    command
        .arg(input)
        .arg("--out")
        .arg(output)
        .args([
            "--width",
            "100",
            "--font-fallback",
            "--no-system-fonts",
            "--font-file",
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/samples.ttf"));
    if strict {
        command.arg("--strict");
    }
    command.output().unwrap()
}
#[test]
fn draws_pixels_in_presentation_order_and_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pptx");
    let out = dir.path().join("images");
    fixture(&input, "");
    let result = run(&input, &out, true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let blue = resvg::tiny_skia::Pixmap::load_png(out.join("slide-001.png")).unwrap();
    assert_eq!((blue.width(), blue.height()), (100, 100));
    let pixel = blue.pixel(50, 50).unwrap();
    assert_eq!((pixel.red(), pixel.green(), pixel.blue()), (0, 0, 255));
    let red = resvg::tiny_skia::Pixmap::load_png(out.join("slide-002.png")).unwrap();
    assert_eq!(red.pixel(50, 50).unwrap().red(), 255);
    let before = fs::read(out.join("manifest.json")).unwrap();
    assert!(!run(&input, &out, false).status.success());
    assert_eq!(before, fs::read(out.join("manifest.json")).unwrap());
}
#[test]
fn strict_rejects_unsupported_objects_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pptx");
    let out = dir.path().join("images");
    fixture(&input, "<p:graphicFrame/>");
    let result = run(&input, &out, true);
    assert!(!result.status.success());
    assert!(!out.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("graphicFrame"));
    assert!(run(&input, &out, false).status.success());
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "preview_with_warnings");
}
#[test]
fn invalid_archive_fails_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("broken.pptx");
    let out = dir.path().join("images");
    fs::write(&input, "broken").unwrap();
    assert!(!run(&input, &out, false).status.success());
    assert!(!out.exists());
}
#[test]
fn renders_real_pptx_with_japanese_and_embedded_image() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("basic.pptx");
    let out = dir.path().join("images");
    fs::write(&input, include_bytes!("../examples/basic.pptx")).unwrap();
    let result = run(&input, &out, false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let image = resvg::tiny_skia::Pixmap::load_png(out.join("slide-001.png")).unwrap();
    let pixel = image.pixel(72, 26).unwrap();
    assert_eq!((pixel.red(), pixel.green(), pixel.blue()), (228, 185, 87));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "preview_with_warnings");
}
#[test]
fn theme_colors_are_resolved_through_layout_and_master() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("theme.pptx");
    let out = dir.path().join("images");
    fs::write(&input, include_bytes!("../examples/theme.pptx")).unwrap();
    let result = run(&input, &out, false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let image = resvg::tiny_skia::Pixmap::load_png(out.join("slide-001.png")).unwrap();
    let green = image.pixel(15, 25).unwrap();
    let orange = image.pixel(45, 25).unwrap();
    let text_color = image.pixel(75, 25).unwrap();
    assert_eq!((green.red(), green.green(), green.blue()), (36, 90, 81));
    assert_eq!(
        (orange.red(), orange.green(), orange.blue()),
        (195, 123, 73)
    );
    assert_eq!(
        (text_color.red(), text_color.green(), text_color.blue()),
        (0, 0, 0)
    );
}

#[test]
fn default_output_is_next_to_input_and_explicit_output_takes_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("入力 資料");
    fs::create_dir(&nested).unwrap();
    let input = nested.join("資料.v2.pptx");
    fixture(&input, "");
    let out = nested.join("資料.v2.rhodopsin");
    let invoke = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rhodopsin"));
        command
            .current_dir(dir.path())
            .arg(Path::new("入力 資料").join("資料.v2.pptx"))
            .args(["--width", "100", "--no-system-fonts", "--font-fallback"]);
        command.output().unwrap()
    };
    let result = invoke();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.starts_with("status: completed\noutput:\n  created: true\n"));
    assert!(stdout.contains("資料.v2.rhodopsin\""));
    assert!(stdout.contains("  slides: 2\n"));
    assert!(out.join("slide-001.png").is_file());
    assert!(out.join("slide-002.png").is_file());
    let before = fs::read(out.join("manifest.json")).unwrap();
    let repeated = invoke();
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("Output already exists"));
    assert_eq!(before, fs::read(out.join("manifest.json")).unwrap());
    let explicit = dir.path().join("指定出力");
    assert!(run(&input, &explicit, true).status.success());
    assert!(explicit.join("manifest.json").is_file());
}

#[test]
fn library_matches_cli_pixels_and_manifest() {
    use rhodopsin::{ConvertOptions, convert};
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pptx");
    fixture(&input, "<p:graphicFrame/>");
    let cli_out = dir.path().join("cli");
    let lib_out = dir.path().join("lib");
    assert!(run(&input, &cli_out, false).status.success());
    let settings = ConvertOptions {
        width: 100,
        no_system_fonts: true,
        font_file: vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/samples.ttf")],
        ..Default::default()
    };
    let report = convert(&input, &lib_out, &settings).unwrap();
    assert_eq!(report.slides.len(), 2);
    assert!(!report.slides[0].diagnostics.is_empty());
    for name in ["slide-001.png", "slide-002.png", "manifest.json"] {
        assert_eq!(
            fs::read(cli_out.join(name)).unwrap(),
            fs::read(lib_out.join(name)).unwrap()
        );
    }
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(lib_out.join("manifest.json")).unwrap()
        )
        .unwrap()
    );
    assert!(convert(&input, &lib_out, &settings).is_err());
    assert_eq!(
        fs::read(cli_out.join("manifest.json")).unwrap(),
        fs::read(lib_out.join("manifest.json")).unwrap()
    );
}

#[test]
fn library_returns_strict_diagnostics_and_validates_width() {
    use rhodopsin::{ConvertOptions, DiagnosticCode, StrictModeError, convert};
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pptx");
    let out = dir.path().join("images");
    fixture(&input, "<p:graphicFrame/>");
    let settings = ConvertOptions {
        strict: true,
        no_system_fonts: true,
        ..Default::default()
    };
    let error = convert(&input, &out, &settings).unwrap_err();
    let strict = error.downcast_ref::<StrictModeError>().unwrap();
    assert!(
        strict.slides[0]
            .warnings
            .iter()
            .any(|w| w.contains("graphicFrame"))
    );
    assert!(
        strict.slides[0]
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::Omitted)
    );
    assert!(!out.exists());
    for width in [0, 63, 4097, u32::MAX] {
        let settings = ConvertOptions {
            width,
            ..Default::default()
        };
        assert!(
            convert(&input, &out, &settings)
                .unwrap_err()
                .to_string()
                .contains("width")
        );
        assert!(!out.exists());
    }
    fs::write(&input, "broken").unwrap();
    assert!(convert(&input, &out, &ConvertOptions::default()).is_err());
    assert!(!out.exists());
}

#[test]
fn font_fallback_is_explicit_and_reports_all_affected_slides() {
    use rhodopsin::{ConvertOptions, FontFallbackError, convert};
    let font = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/latin.ttf");
    // Verify missing families, glyphs, and styles independently of OS fonts.
    for (family, value, attributes, reason) in [
        ("不在の書体", "A", "", "family unavailable"),
        ("Pptx Latin", "あ", "", "U+3042"),
        ("Pptx Latin", "A", "b=\"1\"", "weight or italic"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("fonts.pptx");
        fixture(
            &input,
            &format!(
                r#"<p:sp><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr {attributes}><a:latin typeface="{family}"/><a:ea typeface="{family}"/></a:rPr><a:t>{value}</a:t></a:r></a:p></p:txBody></p:sp>"#
            ),
        );
        let settings = ConvertOptions {
            width: 100,
            no_system_fonts: true,
            font_file: vec![font.clone()],
            ..Default::default()
        };
        let api_out = dir.path().join("api");
        let error = convert(&input, &api_out, &settings).unwrap_err();
        let diagnostic = error.downcast_ref::<FontFallbackError>().unwrap();
        assert_eq!(diagnostic.slides.len(), 2);
        assert!(!api_out.exists());
        let text = diagnostic.to_string();
        assert!(text.contains(family), "{text}");
        assert!(text.contains(reason), "{text}");
        assert!(text.contains("slides 1, 2"), "{text}");
        for (suffix, flags, success) in [
            ("default", vec![], false),
            ("fallback", vec!["--font-fallback"], value != "あ"),
            ("strict", vec!["--font-fallback", "--strict"], false),
        ] {
            let out = dir.path().join(suffix);
            let result = Command::new(env!("CARGO_BIN_EXE_rhodopsin"))
                .arg(&input)
                .arg("--out")
                .arg(&out)
                .args(["--width", "100", "--no-system-fonts", "--font-file"])
                .arg(&font)
                .args(flags)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&result.stderr);
            assert_eq!(result.status.success(), success, "{stderr}");
            assert_eq!(out.exists(), success, "{stderr}");
            if success {
                assert!(out.join("slide-002.png").exists());
                let report: serde_json::Value =
                    serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
                assert!(
                    !report["slides"][0]["diagnostics"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
            } else if suffix == "default" {
                assert!(stderr.contains(family), "{stderr}");
                assert!(
                    stderr.starts_with("status: failed\noutput:\n  created: false\nerrors:\n"),
                    "{stderr}"
                );
                assert!(stderr.contains("suggestion:"), "{stderr}");
                assert!(!stderr.contains("--font-fallback"), "{stderr}");
                assert!(!stderr.contains("recovery:"), "{stderr}");
                assert!(!stderr.contains("constraints:"), "{stderr}");
                assert!(result.stdout.is_empty());
                if value == "あ" {
                    assert!(stderr.contains("code: FONT_GLYPH_MISSING"), "{stderr}");
                    assert!(stderr.contains("character: \"あ\""), "{stderr}");
                    assert!(stderr.contains("codepoint: \"U+3042\""), "{stderr}");
                }
            }
        }
    }
}

#[test]
fn exact_font_needs_no_flag_but_mapping_requires_permission() {
    use rhodopsin::{ConvertOptions, FontFallbackError, convert};
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("fonts.pptx");
    fixture(
        &input,
        r#"<p:sp><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr><a:latin typeface="Pptx Latin"/></a:rPr><a:t>A</a:t></a:r></a:p></p:txBody></p:sp>"#,
    );
    let mut settings = ConvertOptions {
        width: 100,
        no_system_fonts: true,
        font_file: vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/latin.ttf")],
        ..Default::default()
    };
    convert(&input, dir.path().join("exact"), &settings).unwrap();
    settings
        .font_file
        .push(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cjk.ttf"));
    settings
        .font_mapping
        .insert("Pptx Latin".into(), "Pptx CJK".into());
    let out = dir.path().join("mapped");
    assert!(
        convert(&input, &out, &settings)
            .unwrap_err()
            .is::<FontFallbackError>()
    );
    assert!(!out.exists());
    settings.font_fallback = true;
    convert(&input, &out, &settings).unwrap();
    settings.font_file.clear();
    settings.font_fallback = false;
    let out = dir.path().join("no-fonts");
    let error = convert(&input, &out, &settings).unwrap_err();
    assert!(error.is::<FontFallbackError>());
    assert!(error.to_string().contains("Pptx Latin"));
    assert!(!out.exists());
}

#[test]
fn invalid_arguments_are_yaml_but_help_and_version_remain_available() {
    let help = Command::new(env!("CARGO_BIN_EXE_rhodopsin"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("Convert PPTX to PNG or PDF without Office"));
    assert!(help.contains("application bundles and inaccessible paths"));
    assert!(help.contains("Individual missing glyphs"));
    assert!(!help.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c)));

    let invalid = Command::new(env!("CARGO_BIN_EXE_rhodopsin"))
        .arg("--invalid-option")
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(stderr.starts_with("status: failed\n"));
    assert!(stderr.contains("code: INVALID_ARGUMENTS"));
    for arg in ["--help", "--version"] {
        let result = Command::new(env!("CARGO_BIN_EXE_rhodopsin"))
            .arg(arg)
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(!result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn glyph_fallback_completes_with_structured_warning_without_permission() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("glyphs.pptx");
    fixture(
        &input,
        r#"<p:sp><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr><a:latin typeface="Pptx Latin"/><a:ea typeface="Pptx Latin"/></a:rPr><a:t>Aあ</a:t></a:r></a:p></p:txBody></p:sp>"#,
    );
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for strict in [false, true] {
        let out = dir.path().join(if strict { "strict" } else { "normal" });
        let mut command = Command::new(env!("CARGO_BIN_EXE_rhodopsin"));
        command
            .arg(&input)
            .arg("--out")
            .arg(&out)
            .args(["--width", "100", "--no-system-fonts", "--font-file"])
            .arg(fonts.join("latin.ttf"))
            .arg("--font-file")
            .arg(fonts.join("cjk.ttf"));
        if strict {
            command.arg("--strict");
        }
        let result = command.output().unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.success(), !strict, "{stdout}\n{stderr}");
        assert_eq!(out.exists(), !strict);
        if strict {
            assert!(stderr.contains("STRICT_MODE_REJECTED"), "{stderr}");
        } else {
            for expected in [
                "status: completed",
                "warnings:\n",
                "code: FONT_GLYPH_MISSING",
                "family: \"Pptx Latin\"",
                "substitute_family: \"Pptx CJK\"",
                "character: \"あ\"",
                "codepoint: \"U+3042\"",
                "slides: [1,2]",
            ] {
                assert!(stdout.contains(expected), "{expected}: {stdout}");
            }
            assert!(out.join("slide-002.png").exists());
        }
    }
}
