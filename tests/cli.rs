//! 実際の ZIP/XML 入力と PNG の画素を使って CLI の契約を確認する。
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
    // ファイル名の順序と異なる presentation 順序を検証する。
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
    let mut command = Command::new(env!("CARGO_BIN_EXE_pptx2img"));
    command
        .arg(input)
        .arg("--out")
        .arg(output)
        .args(["--width", "100"]);
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
