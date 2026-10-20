//! EMF and placeable WMF pictures, translated to SVG (issue #78).
//!
//! The metafiles under `fixtures/metafile/` are written by
//! `tests/visual/make_metafile_pictures.py` from MS-EMF / MS-WMF; the expectations below
//! are the shapes its docstrings describe. Other metafiles are built here byte by byte.
mod support;
use std::fs;
use support::*;

/// One slide (100 x 100 px at `--width 100`) whose only shape is a full-slide picture.
fn render_picture(name: &str, data: Vec<u8>) -> (resvg::tiny_skia::Pixmap, serde_json::Value) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pptx");
    let out = dir.path().join("out");
    let picture = format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="metafile"/></p:nvPicPr><p:blipFill><a:blip r:embed="img"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
        transform(0, 0, 100, 100)
    );
    let rels = format!(
        r#"<Relationships xmlns="{PR}"><Relationship Id="img" Type="{R}/image" Target="../media/{name}"/></Relationships>"#
    );
    let part = format!("ppt/media/{name}");
    write_deck(
        &input,
        page("sld", "", "", &picture),
        vec![
            ("ppt/slides/_rels/slide1.xml.rels", rels.into_bytes()),
            (part.as_str(), data),
        ],
    );
    check(run(&input, &out, false));
    let manifest = serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    (image(&out), manifest)
}
fn fixture(name: &str) -> Vec<u8> {
    fs::read(format!(
        "{}/tests/fixtures/metafile/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
/// `(code, message)` of every diagnostic on the slide.
fn diagnostics(manifest: &serde_json::Value) -> Vec<(String, String)> {
    manifest["slides"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_string(),
                d["message"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}
/// Assert the colour at output pixel (`x`, `y`) within a small tolerance (antialiasing
/// and bitmap smoothing). Bitmap samples avoid the black diagonal of the test pattern.
fn assert_color(
    img: &resvg::tiny_skia::Pixmap,
    (x, y): (u32, u32),
    want: (u8, u8, u8),
    what: &str,
) {
    let got = rgb(img, x, y);
    let close = |a: u8, b: u8| a.abs_diff(b) <= 12;
    assert!(
        close(got.0, want.0) && close(got.1, want.1) && close(got.2, want.2),
        "{what} at ({x}, {y}): {got:?}, want {want:?}"
    );
}

const RED: (u8, u8, u8) = (220, 30, 30);
const GREEN: (u8, u8, u8) = (20, 160, 60);
const BLUE: (u8, u8, u8) = (30, 60, 220);
const YELLOW: (u8, u8, u8) = (250, 210, 30);
const GRAY: (u8, u8, u8) = (150, 150, 150);
const WHITE: (u8, u8, u8) = (255, 255, 255);

/// Fixture coordinates are 0..200 units on a 100 px picture: pixel = unit / 2.
fn px(x: u32, y: u32) -> (u32, u32) {
    (x / 2, y / 2)
}

/// An output pixel, its expected colour, and what is drawn there.
type Sample = ((u32, u32), (u8, u8, u8), &'static str);

#[test]
fn fixture_groups_render_inside_the_picture_frame() {
    let cases: &[(&str, &[Sample])] = &[
        (
            "emf-shapes.emf",
            &[
                (px(50, 35), RED, "rectangle"),
                (px(150, 35), GREEN, "ellipse"),
                (px(50, 130), BLUE, "POLYGON16 triangle"),
                (px(50, 178), YELLOW, "round rectangle"),
                (px(100, 70), WHITE, "background"),
            ],
        ),
        (
            "emf-shapes-header108.emf",
            &[
                (px(50, 35), RED, "rectangle"),
                (px(150, 35), GREEN, "ellipse"),
            ],
        ),
        (
            "emf-path.emf",
            &[
                (px(60, 70), RED, "filled closed path"),
                (px(140, 150), GREEN, "rectangle inside the clip path"),
                (px(110, 150), WHITE, "rectangle outside the clip path"),
            ],
        ),
        (
            "emf-transform.emf",
            &[
                (px(40, 40), RED, "square at the origin"),
                (px(134, 62), BLUE, "rotated square"),
                (px(150, 150), GREEN, "scaled and translated square"),
                (px(100, 20), WHITE, "unrotated position of the blue square"),
            ],
        ),
        (
            "emf-clip.emf",
            &[
                (px(65, 65), RED, "inside IntersectClipRect"),
                (px(130, 40), GRAY, "red rectangle outside the clip"),
                (px(120, 120), BLUE, "blue rectangle"),
                (px(150, 150), GRAY, "ExcludeClipRect hole"),
            ],
        ),
        (
            "emf-bitmap.emf",
            &[
                (px(40, 20), RED, "StretchDIBits top-left quadrant"),
                (px(75, 30), GREEN, "StretchDIBits top-right quadrant"),
                (px(25, 75), BLUE, "StretchDIBits bottom-left quadrant"),
                (
                    px(176, 150),
                    YELLOW,
                    "second StretchDIBits bottom-right quadrant",
                ),
                (px(124, 34), BLUE, "BitBlt bottom-left quadrant"),
            ],
        ),
        (
            "wmf-shapes.wmf",
            &[
                (px(50, 35), RED, "rectangle"),
                (px(150, 35), GREEN, "ellipse"),
                (px(50, 130), BLUE, "triangle"),
            ],
        ),
        (
            "wmf-clip-bitmap.wmf",
            &[
                (px(40, 26), RED, "StretchDIB inside the clip"),
                (px(80, 80), WHITE, "StretchDIB outside the clip"),
            ],
        ),
    ];
    for (name, samples) in cases {
        let (img, manifest) = render_picture(name, fixture(name));
        for &(at, color, what) in *samples {
            assert_color(&img, at, color, &format!("{name}: {what}"));
        }
        let records = diagnostics(&manifest);
        let unexpected: Vec<_> = records
            .iter()
            .filter(|(code, message)| code != "unsupported" || !message.contains("(META_TEXTOUT)"))
            .collect();
        assert!(unexpected.is_empty(), "{name}: {records:?}");
    }
}

#[test]
fn text_records_are_reported_once_and_the_rest_renders() {
    let (_, manifest) = render_picture("emf-text.emf", fixture("emf-text.emf"));
    assert_eq!(
        diagnostics(&manifest),
        [(
            "unsupported".into(),
            "Text in metafile pictures is not rendered (EMR_EXTTEXTOUTW): ppt/media/emf-text.emf"
                .into()
        )]
    );
    let (img, manifest) = render_picture("wmf-clip-bitmap.wmf", fixture("wmf-clip-bitmap.wmf"));
    assert_color(&img, px(40, 26), RED, "bitmap next to the text record");
    assert_eq!(
        diagnostics(&manifest),
        [(
            "unsupported".into(),
            "Text in metafile pictures is not rendered (META_TEXTOUT): ppt/media/wmf-clip-bitmap.wmf"
                .into()
        )]
    );
}

#[test]
fn all_three_emf_header_sizes_render_the_same() {
    let header88 = fixture("emf-shapes.emf");
    // The 100-byte header adds cbPixelFormat, offPixelFormat, and bOpenGL (all zero).
    let mut header100 = header88[..88].to_vec();
    header100.extend([0; 12]);
    header100.extend(&header88[88..]);
    header100[4..8].copy_from_slice(&100u32.to_le_bytes());
    let total = header100.len() as u32;
    header100[48..52].copy_from_slice(&total.to_le_bytes());
    let (a, _) = render_picture("a.emf", header88);
    let (b, _) = render_picture("b.emf", header100);
    let (c, _) = render_picture("c.emf", fixture("emf-shapes-header108.emf"));
    assert_eq!(a.data(), b.data());
    assert_eq!(a.data(), c.data());
}

// -- metafiles built here -----------------------------------------------------------

fn record(kind: u32, payload: &[u8]) -> Vec<u8> {
    let mut data = kind.to_le_bytes().to_vec();
    data.extend((8 + payload.len() as u32).to_le_bytes());
    data.extend(payload);
    data
}
fn ints(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
/// An 88-byte EMF header (100 x 100 device pixels on a 96-dpi reference device),
/// `records`, and EMR_EOF.
fn emf(records: &[Vec<u8>]) -> Vec<u8> {
    let mut body: Vec<u8> = records.concat();
    body.extend(record(14, &ints(&[0, 16, 20])));
    let mut header = ints(&[1, 88, 0, 0, 99, 99, 0, 0, 2646, 2646]);
    header.extend(b" EMF");
    header.extend(ints(&[
        0x10000,
        88 + body.len() as i32,
        records.len() as i32 + 2,
    ]));
    header.extend([0, 0, 0, 0]);
    header.extend(ints(&[0, 0, 0, 960, 960, 254, 254]));
    assert_eq!(header.len(), 76 + 12);
    header.extend(body);
    header
}
/// EMR_STRETCHDIBITS drawing a `width` x `height` 1-bit white DIB onto the full picture.
fn stretch_1bit(width: i32, height: i32) -> Vec<u8> {
    let stride = ((width + 31) / 32 * 4) as usize;
    let bits = vec![0xFFu8; stride * height as usize];
    let mut info = ints(&[40, width, height]);
    info.extend(1u16.to_le_bytes());
    info.extend(1u16.to_le_bytes());
    info.extend(ints(&[0, 0, 0, 0, 2, 0]));
    info.extend([0, 0, 0, 0, 255, 255, 255, 0]);
    let mut payload = ints(&[0, 0, 99, 99, 0, 0, 0, 0, width, height]);
    payload.extend(ints(&[
        80,
        info.len() as i32,
        80 + info.len() as i32,
        bits.len() as i32,
    ]));
    payload.extend(ints(&[0, 0x00CC_0020, 100, 100]));
    payload.extend(info);
    payload.extend(bits);
    record(81, &payload)
}
/// A red solid brush selected for the following shapes.
fn red_brush() -> Vec<Vec<u8>> {
    vec![
        record(39, &ints(&[1, 0, 0x001E_1EDC, 0])),
        record(37, &ints(&[1])),
    ]
}

#[test]
fn bitmaps_follow_the_png_jpeg_size_policy() {
    // 4097 x 4096 px is above the embed limit: downscaled with the existing diagnostic.
    let (img, manifest) = render_picture("big.emf", emf(&[stretch_1bit(4097, 4096)]));
    assert_color(&img, (50, 50), WHITE, "downscaled bitmap");
    assert_eq!(
        diagnostics(&manifest),
        [(
            "approximation".into(),
            "Downscaled oversized image from 4097x4096 to 2048x2048 px: ppt/media/big.emf".into()
        )]
    );
    // Above the ceiling: the picture is omitted, naming the part.
    let mut records = red_brush();
    records.push(record(43, &ints(&[0, 0, 99, 99])));
    records.push(stretch_1bit(32_769, 1));
    let (img, manifest) = render_picture("huge.emf", emf(&records));
    assert_color(&img, (50, 50), WHITE, "omitted picture");
    assert_eq!(
        diagnostics(&manifest),
        [(
            "omitted".into(),
            "Image exceeds the downscale ceiling of 67108864 pixels or 32768 px per side (32769x1 px): ppt/media/huge.emf".into()
        )]
    );
}

#[test]
fn unknown_records_are_reported_once_per_type() {
    let mut records = red_brush();
    records.push(record(43, &ints(&[10, 10, 90, 90])));
    // Two EMR_ARC records and one unassigned record type.
    for _ in 0..2 {
        records.push(record(45, &ints(&[0, 0, 10, 10, 0, 0, 10, 10])));
    }
    records.push(record(200, &[]));
    let (img, manifest) = render_picture("arcs.emf", emf(&records));
    assert_color(
        &img,
        (50, 50),
        (220, 30, 30),
        "rectangle before the unknown records",
    );
    assert_eq!(
        diagnostics(&manifest),
        [
            (
                "unsupported".into(),
                "Unsupported metafile record ignored (EMR_ARC): ppt/media/arcs.emf".into()
            ),
            (
                "unsupported".into(),
                "Unsupported metafile record ignored (EMF record type 200): ppt/media/arcs.emf"
                    .into()
            ),
        ]
    );
}

#[test]
fn broken_and_oversized_metafiles_are_omitted_without_aborting_the_deck() {
    let rectangle = record(43, &ints(&[10, 10, 90, 90]));
    let mut truncated = emf(std::slice::from_ref(&rectangle));
    truncated.truncate(truncated.len() - 20);
    let mut bad_size = emf(std::slice::from_ref(&rectangle));
    bad_size[92..96].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
    let mut short_polygon = record(86, &ints(&[0, 0, 0, 0, 1_000_000]));
    short_polygon.extend([0; 8]);
    let length = short_polygon.len() as u32;
    short_polygon[4..8].copy_from_slice(&length.to_le_bytes());
    let many_records = emf(&vec![record(18, &ints(&[1])); 500_001]);
    let many_shapes = emf(&vec![rectangle.clone(); 50_001]);
    let mut wmf = fixture("wmf-shapes.wmf");
    wmf.truncate(wmf.len() - 6);
    for (name, data, message) in [
        (
            "truncated.emf",
            truncated,
            "Malformed EMF picture (ends without EMR_EOF)",
        ),
        (
            "size.emf",
            bad_size,
            "Malformed EMF picture (record at byte 88 has an invalid size)",
        ),
        (
            "points.emf",
            emf(&[short_polygon]),
            "Malformed EMF picture (EMR_POLYGON16 is truncated)",
        ),
        (
            "records.emf",
            many_records,
            "Metafile picture exceeds 500000 records; not rendered",
        ),
        (
            "shapes.emf",
            many_shapes,
            "Metafile picture exceeds 50000 drawing elements; not rendered",
        ),
        (
            "truncated.wmf",
            wmf,
            "Malformed WMF picture (ends without META_EOF)",
        ),
    ] {
        let (img, manifest) = render_picture(name, data);
        assert_color(&img, (50, 50), WHITE, name);
        assert_eq!(
            diagnostics(&manifest),
            [("omitted".into(), format!("{message}: ppt/media/{name}"))],
            "{name}"
        );
    }
}

#[test]
fn deep_clip_chains_are_capped() {
    // Each clip record adds one link to an SVG mask or clip-path chain, which the renderer
    // follows recursively; links beyond the cap are reported and not applied, so
    // thousands of them neither grow the SVG quadratically nor exhaust the stack.
    let mut records = red_brush();
    for i in 0..2_000 {
        let x = i % 40;
        records.push(record(29, &ints(&[x, 0, x + 1, 1])));
        records.push(record(30, &ints(&[0, 0, 100, 100])));
    }
    records.push(record(43, &ints(&[0, 10, 99, 99])));
    let (img, manifest) = render_picture("clips.emf", emf(&records));
    assert_color(
        &img,
        (50, 50),
        (220, 30, 30),
        "rectangle under the clip records",
    );
    assert_eq!(
        diagnostics(&manifest),
        [(
            "approximation".into(),
            "Metafile clip regions nested deeper than 32 are not applied: ppt/media/clips.emf"
                .into()
        )]
    );
}

#[test]
fn distinct_diagnostics_are_bounded() {
    let mut records = red_brush();
    records.push(record(43, &ints(&[10, 10, 90, 90])));
    // 1,000 unknown record types and 1,000 different raster operations.
    for kind in 200..1_200 {
        records.push(record(kind, &[]));
    }
    for rop in 0..1_000 {
        let mut payload = ints(&[0, 0, 0, 0, 0, 0, 1, 1, 0x00AA_0000 | rop]);
        payload.resize(92, 0);
        records.push(record(76, &payload));
    }
    let (img, manifest) = render_picture("notes.emf", emf(&records));
    assert_color(&img, (50, 50), (220, 30, 30), "rectangle");
    let notes = diagnostics(&manifest);
    assert_eq!(notes.len(), 65, "{notes:?}");
    assert_eq!(
        notes.last().unwrap().1,
        "Further metafile diagnostics omitted after 64: ppt/media/notes.emf"
    );
}

#[test]
fn many_wmf_objects_reuse_free_slots() {
    // Fill 60,000 object slots, then delete and recreate the lowest one 60,000 times.
    let record = |function: u16, params: &[u8]| {
        let mut data = ((6 + params.len() as u32) / 2).to_le_bytes().to_vec();
        data.extend(function.to_le_bytes());
        data.extend(params);
        data
    };
    let brush = record(0x02FC, &[0, 0, 0xDC, 0x1E, 0x1E, 0, 0, 0]);
    let mut body = Vec::new();
    for _ in 0..60_000 {
        body.extend(&brush);
    }
    for _ in 0..60_000 {
        body.extend(record(0x01F0, &0u16.to_le_bytes()));
        body.extend(&brush);
    }
    body.extend(record(0x012D, &0u16.to_le_bytes()));
    body.extend(record(0x041B, &[90, 0, 90, 0, 10, 0, 10, 0]));
    body.extend(record(0, &[]));
    let mut wmf = 0x9AC6_CDD7u32.to_le_bytes().to_vec();
    for v in [0i16, 0, 0, 100, 100] {
        wmf.extend(v.to_le_bytes());
    }
    wmf.extend(96u16.to_le_bytes());
    wmf.extend([0; 6]);
    for v in [1u16, 9, 0x0300] {
        wmf.extend(v.to_le_bytes());
    }
    wmf.extend([0; 12]);
    wmf.extend(body);
    let (img, manifest) = render_picture("objects.wmf", wmf);
    assert_color(
        &img,
        (50, 50),
        (220, 30, 30),
        "rectangle with the recreated brush",
    );
    assert_eq!(diagnostics(&manifest), []);
}
