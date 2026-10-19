//! Verify that parts no renderer reads are not loaded or counted against the size limits.
mod support;
use resvg::tiny_skia::{Color, Pixmap};
use std::{fs, io::Write};
use support::*;
use zip::{ZipWriter, write::SimpleFileOptions};

const MIB: usize = 1024 * 1024;
const MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";

fn poster() -> Vec<u8> {
    let mut png = Pixmap::new(2, 2).unwrap();
    png.fill(Color::from_rgba8(0, 0, 255, 255));
    png.encode_png().unwrap()
}
/// A video shape that references its playback part and a poster image, as PowerPoint saves it.
fn video_deck(path: &std::path::Path, media: Vec<u8>, poster: Vec<u8>) {
    let pic = format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="2" name="Video"/><p:cNvPicPr/><p:nvPr><a:videoFile r:link="video"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" r:embed="media"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="poster"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"/></p:spPr></p:pic>"#,
        transform(0, 0, 100, 100)
    );
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="video" Type="{R}/video" Target="../media/media1.mov"/><Relationship Id="media" Type="{MEDIA}" Target="../media/media1.mov"/><Relationship Id="poster" Type="{R}/image" Target="../media/image1.png"/></Relationships>"#
    );
    write_deck(
        path,
        page("sld", "", "", &pic),
        vec![
            ("ppt/slides/_rels/slide1.xml.rels", rels.into_bytes()),
            ("ppt/media/media1.mov", media),
            ("ppt/media/image1.png", poster),
        ],
    );
}
fn diagnostics(out: &std::path::Path) -> Vec<serde_json::Value> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    manifest["slides"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn oversized_video_is_skipped_and_poster_renders() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("video.pptx");
    let out = dir.path().join("out");
    video_deck(&path, vec![0; 40 * MIB], poster());
    check(run(&path, &out, false));
    assert_eq!(rgb(&image(&out), 50, 50), (0, 0, 255));
    let records = diagnostics(&out);
    assert!(
        records.iter().any(|d| d["code"] == "unsupported"
            && d["message"] == "Audio and video playback is unsupported"),
        "{records:?}"
    );
}
#[test]
fn strict_rejects_video_playback_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("video.pptx");
    let out = dir.path().join("out");
    video_deck(&path, b"not a real video".to_vec(), poster());
    let output = run(&path, &out, true);
    assert!(!output.status.success());
    assert!(!out.exists());
}
#[test]
fn oversized_rendered_image_still_hits_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.pptx");
    let out = dir.path().join("out");
    let mut png = poster();
    png.resize(40 * MIB, 0);
    video_deck(&path, b"small".to_vec(), png);
    let output = run(&path, &out, false);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Uncompressed PPTX exceeds the limit"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out.exists());
}
#[test]
fn unreferenced_oversized_part_is_not_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("orphan.pptx");
    let out = dir.path().join("out");
    write_deck(
        &path,
        page("sld", "", "", &rect(0, 0, 100, 100, "00FF00")),
        vec![
            ("docProps/thumbnail.jpeg", vec![0; 40 * MIB]),
            ("ppt/embeddings/oleObject1.bin", vec![0; 40 * MIB]),
        ],
    );
    check(run(&path, &out, false));
    assert_eq!(rgb(&image(&out), 50, 50), (0, 255, 0));
}
#[test]
fn part_count_limit_covers_skipped_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.pptx");
    let out = dir.path().join("out");
    write_deck(&path, page("sld", "", "", ""), vec![]);
    // Rewrite with extra unreferenced entries so the archive exceeds 10,000 entries.
    let mut zip = ZipWriter::new_append(
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap(),
    )
    .unwrap();
    for index in 0..10_000 {
        zip.start_file(
            format!("ppt/media/unused{index}.bin"),
            SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"x").unwrap();
    }
    zip.finish().unwrap();
    let output = run(&path, &out, false);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Part count exceeds the limit"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out.exists());
}
