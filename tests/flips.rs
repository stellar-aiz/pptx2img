//! Flipped shapes and groups mirror their geometry but never mirror their text (Issue #52).
mod support;
use std::collections::BTreeSet;
use support::*;

/// Text box in slide pixels (the test slide is 100x100 px): x, y, width, height.
const BOX: (i32, i32, i32, i32) = (5, 10, 90, 40);
/// Uneven glyph gaps ("A" is narrow, "あ" is wide) make a mirror image distinguishable
/// from a translation, and each glyph sits low in its line, so a 180-degree turn is too.
const BODY: &str = r#"<p:txBody><a:bodyPr lIns="0" rIns="0" tIns="0" bIns="0" anchor="t"/><a:p><a:pPr algn="l"/><a:r><a:rPr sz="1800"><a:latin typeface="Rhodopsin Metrics Test"/><a:ea typeface="Rhodopsin Metrics Test"/><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:rPr><a:t>AAあA</a:t></a:r></a:p></p:txBody>"#;

type Pixels = BTreeSet<(i32, i32)>;

fn flips(h: bool, v: bool) -> String {
    format!(
        "{}{}",
        if h { r#" flipH="1""# } else { "" },
        if v { r#" flipV="1""# } else { "" }
    )
}
fn xfrm(flip: &str, (x, y, w, h): (i32, i32, i32, i32), child: bool) -> String {
    let child = if child {
        format!(
            r#"<a:chOff x="{}" y="{}"/><a:chExt cx="{}" cy="{}"/>"#,
            x * 9525,
            y * 9525,
            w * 9525,
            h * 9525
        )
    } else {
        String::new()
    };
    format!(
        r#"<a:xfrm{flip}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/>{child}</a:xfrm>"#,
        x * 9525,
        y * 9525,
        w * 9525,
        h * 9525
    )
}
fn shape(flip: &str, preset: &str, fill: &str, body: &str) -> String {
    format!(
        r#"<p:sp><p:spPr>{}<a:prstGeom prst="{preset}"/>{fill}</p:spPr>{body}</p:sp>"#,
        xfrm(flip, BOX, false)
    )
}
fn text_shape(flip: &str, preset: &str, fill: &str) -> String {
    shape(flip, preset, fill, BODY)
}
fn group(flip: &str, children: &str) -> String {
    format!(
        r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="9" name="g"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr>{}</p:grpSpPr>{children}</p:grpSp>"#,
        xfrm(flip, BOX, true)
    )
}
/// A one-cell table frame at `BOX` whose cell holds `BODY` (without the shape-only bodyPr).
fn table() -> String {
    let (x, y, w, h) = BOX;
    let text = BODY
        .replace("<p:txBody>", "<a:txBody>")
        .replace("</p:txBody>", "</a:txBody>");
    format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="8" name="t"/></p:nvGraphicFramePr><p:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblPr/><a:tblGrid><a:gridCol w="{}"/></a:tblGrid><a:tr h="{}"><a:tc>{text}<a:tcPr marL="0" marR="0" marT="0" marB="0" anchor="t"/></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        x * 9525,
        y * 9525,
        w * 9525,
        h * 9525,
        w * 9525,
        h * 9525
    )
}
fn render(shapes: &str) -> resvg::tiny_skia::Pixmap {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("flips.pptx");
    let out = d.path().join("out");
    write_deck(&input, page("sld", "", "", shapes), vec![]);
    check(run(&input, &out, false));
    image(&out)
}
fn select(image: &resvg::tiny_skia::Pixmap, keep: impl Fn((u8, u8, u8)) -> bool) -> Pixels {
    let mut pixels = Pixels::new();
    for y in 0..image.height() {
        for x in 0..image.width() {
            if keep(rgb(image, x, y)) {
                pixels.insert((x as i32, y as i32));
            }
        }
    }
    pixels
}
/// Dark (glyph) pixels.
fn glyphs(image: &resvg::tiny_skia::Pixmap) -> Pixels {
    select(image, |(r, g, b)| r < 96 && g < 96 && b < 96)
}
/// Painted (non-background) pixels.
fn painted(image: &resvg::tiny_skia::Pixmap) -> Pixels {
    select(image, |(r, g, b)| r < 64 || g < 64 || b < 64)
}
/// Green (shape fill) pixels.
fn green(image: &resvg::tiny_skia::Pixmap) -> Pixels {
    select(image, |(r, g, b)| r < 96 && g > 160 && b < 96)
}
fn map(pixels: &Pixels, f: impl Fn((i32, i32)) -> (i32, i32)) -> Pixels {
    pixels.iter().copied().map(f).collect()
}
/// Mirror around the text box's vertical centre line.
fn mirror_x(pixels: &Pixels) -> Pixels {
    let (x, _, w, _) = BOX;
    map(pixels, |(px, py)| (2 * x + w - 1 - px, py))
}
/// Rotate 180 degrees around the text box centre.
fn rotate_180(pixels: &Pixels) -> Pixels {
    let (x, y, w, h) = BOX;
    map(pixels, |(px, py)| (2 * x + w - 1 - px, 2 * y + h - 1 - py))
}
/// Move the set so its bounding box starts at the origin.
fn normalize(pixels: &Pixels) -> Pixels {
    let x0 = pixels.iter().map(|p| p.0).min().unwrap();
    let y0 = pixels.iter().map(|p| p.1).min().unwrap();
    map(pixels, |(px, py)| (px - x0, py - y0))
}
/// Equal within a small anti-aliasing tolerance, and never empty.
fn assert_same(actual: &Pixels, expected: &Pixels, what: &str) {
    assert!(expected.len() > 50, "{what}: too few pixels to compare");
    let differing = actual.symmetric_difference(expected).count();
    assert!(
        differing * 50 <= expected.len(),
        "{what}: {differing} of {} pixels differ: {:?}",
        expected.len(),
        actual
            .symmetric_difference(expected)
            .take(40)
            .collect::<Vec<_>>()
    );
}
fn assert_differs(actual: &Pixels, unexpected: &Pixels, what: &str) {
    let differing = actual.symmetric_difference(unexpected).count();
    assert!(
        differing * 10 > unexpected.len(),
        "{what}: only {differing} pixels differ"
    );
}

#[test]
fn flipped_rectangle_keeps_text_unmirrored() {
    let plain = glyphs(&render(&text_shape("", "rect", "")));
    // The text itself is not symmetric, so the checks below can tell a mirror apart.
    assert_differs(&mirror_x(&plain), &plain, "mirror of plain text");
    assert_differs(&rotate_180(&plain), &plain, "rotation of plain text");

    let flip_h = glyphs(&render(&text_shape(&flips(true, false), "rect", "")));
    assert_same(&flip_h, &plain, "flipH reads like the unflipped text");
    let flip_v = glyphs(&render(&text_shape(&flips(false, true), "rect", "")));
    assert_same(
        &flip_v,
        &rotate_180(&plain),
        "flipV turns the text 180 degrees",
    );
    let flip_hv = glyphs(&render(&text_shape(&flips(true, true), "rect", "")));
    assert_same(
        &flip_hv,
        &rotate_180(&plain),
        "flipH+flipV turns the text 180 degrees",
    );
}

#[test]
fn flipped_arrow_mirrors_outline_but_not_text() {
    let fill = r#"<a:solidFill><a:srgbClr val="00FF00"/></a:solidFill>"#;
    // The outline alone is mirrored.
    let outline = painted(&render(&shape("", "rightArrow", fill, "")));
    let flipped_outline = painted(&render(&shape(&flips(true, false), "rightArrow", fill, "")));
    assert_differs(&mirror_x(&outline), &outline, "mirror of the arrow");
    assert_same(
        &flipped_outline,
        &mirror_x(&outline),
        "flipped arrow outline",
    );
    // With text, the fill still lies inside the mirrored outline.
    let plain = render(&text_shape("", "rightArrow", fill));
    let flipped = render(&text_shape(&flips(true, false), "rightArrow", fill));
    let fill_pixels = green(&flipped);
    assert!(fill_pixels.len() > 500);
    assert!(
        fill_pixels.is_subset(&flipped_outline),
        "flipped arrow fill leaves the mirrored outline"
    );
    // The text moves with the mirrored text rectangle but its glyphs are not mirrored.
    let (plain_text, flipped_text) = (glyphs(&plain), glyphs(&flipped));
    assert_same(
        &normalize(&flipped_text),
        &normalize(&plain_text),
        "arrow text is not mirrored",
    );
    assert_differs(
        &normalize(&flipped_text),
        &normalize(&mirror_x(&plain_text)),
        "arrow text is not a mirror image",
    );
}

#[test]
fn flipped_groups_keep_child_text_unmirrored() {
    let plain = glyphs(&render(&text_shape("", "rect", "")));
    let shape = text_shape("", "rect", "");
    let flipped_child = text_shape(&flips(true, false), "rect", "");
    for (deck, expected, what) in [
        (
            group(&flips(true, false), &shape),
            plain.clone(),
            "flipH group",
        ),
        (
            group(&flips(false, true), &shape),
            rotate_180(&plain),
            "flipV group",
        ),
        (
            group(&flips(true, false), &flipped_child),
            plain.clone(),
            "flipH group with a flipH child",
        ),
        (
            group(&flips(false, true), &group(&flips(true, false), &shape)),
            rotate_180(&plain),
            "flipH group nested in a flipV group",
        ),
    ] {
        assert_same(&glyphs(&render(&deck)), &expected, what);
    }
}

#[test]
fn group_flip_does_not_leak_to_later_siblings() {
    // An empty flipped group precedes the text shape; the shape must render unchanged.
    let plain = glyphs(&render(&text_shape("", "rect", "")));
    let empty = group(&flips(true, false), "");
    let after = glyphs(&render(&format!("{empty}{}", text_shape("", "rect", ""))));
    assert_same(&after, &plain, "sibling after a flipped group");
}

#[test]
fn flipped_groups_keep_table_cell_text_unmirrored() {
    let plain = glyphs(&render(&table()));
    assert_differs(&mirror_x(&plain), &plain, "mirror of plain cell text");
    assert_differs(&rotate_180(&plain), &plain, "rotation of plain cell text");
    for (deck, expected, what) in [
        (
            group(&flips(true, false), &table()),
            plain.clone(),
            "table in a flipH group",
        ),
        (
            group(&flips(false, true), &table()),
            rotate_180(&plain),
            "table in a flipV group",
        ),
        (
            group(&flips(true, true), &table()),
            rotate_180(&plain),
            "table in a flipH+flipV group",
        ),
        (
            group(&flips(false, true), &group(&flips(true, false), &table())),
            rotate_180(&plain),
            "table in a flipH group nested in a flipV group",
        ),
    ] {
        assert_same(&glyphs(&render(&deck)), &expected, what);
    }
}
