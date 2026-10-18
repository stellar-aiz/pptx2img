//! 自作の最小 OOXML パッケージで描画を検証する共通補助。
#![allow(dead_code, unused_imports)]
use resvg::tiny_skia::{Color, Pixmap};
use std::{fs, io::Write, path::Path, process::Command};
use zip::{ZipWriter, write::SimpleFileOptions};
pub const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const PR: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
pub fn rel(kind: &str, target: &str) -> String {
    format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="r1" Type="{R}/{kind}" Target="{target}"/></Relationships>"#
    )
}
pub fn page(tag: &str, attrs: &str, background: &str, shapes: &str) -> String {
    format!(
        r#"<p:{tag} xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}" {attrs}><p:cSld>{background}<p:spTree>{shapes}</p:spTree></p:cSld></p:{tag}>"#
    )
}
pub fn transform(x: i32, y: i32, w: i32, h: i32) -> String {
    format!(
        r#"<a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        x * 9525,
        y * 9525,
        w * 9525,
        h * 9525
    )
}
pub fn rect(x: i32, y: i32, w: i32, h: i32, color: &str) -> String {
    format!(
        r#"<p:sp><p:spPr>{}<a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></p:spPr></p:sp>"#,
        transform(x, y, w, h)
    )
}
pub fn placeholder(index: u32, kind: &str, props: &str, text: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{index}" name="タイトル"/><p:nvPr><p:ph idx="{index}" type="{kind}"/></p:nvPr></p:nvSpPr><p:spPr>{props}</p:spPr>{text}</p:sp>"#
    )
}
pub fn write_deck(path: &Path, slide: String, extra: Vec<(&str, Vec<u8>)>) {
    let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
    let mut parts=vec![
        ("_rels/.rels",rel("officeDocument","ppt/presentation.xml").into_bytes()),
        ("ppt/presentation.xml",format!(r#"<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="r1"/></p:sldIdLst><p:sldSz cx="952500" cy="952500"/></p:presentation>"#).into_bytes()),
        ("ppt/_rels/presentation.xml.rels",rel("slide","slides/slide1.xml").into_bytes()),
        ("ppt/slides/slide1.xml",slide.into_bytes()),
    ];
    for (name, data) in extra {
        parts.retain(|(existing, _)| *existing != name);
        parts.push((name, data));
    }

    for (name, data) in parts {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap();
}
pub fn run(path: &Path, out: &Path, strict: bool) -> std::process::Output {
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
pub fn image(out: &Path) -> Pixmap {
    Pixmap::load_png(out.join("slide-001.png")).unwrap()
}
pub fn rgb(image: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let p = image.pixel(x, y).unwrap();
    (p.red(), p.green(), p.blue())
}
pub fn check(output: std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
