mod support;
use std::fs;
use support::*;

const WHITE: (u8, u8, u8) = (255, 255, 255);
const GREEN: (u8, u8, u8) = (0, 255, 0);

fn props(id: u32, hidden: &str) -> String {
    format!(r#"<p:cNvPr id="{id}" name="n{id}" {hidden}/>"#)
}
fn shape(id: u32, hidden: &str, x: i32, y: i32, color: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr>{}<p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></p:spPr></p:sp>"#,
        props(id, hidden),
        transform(x, y, 10, 10)
    )
}
fn picture(id: u32, hidden: &str, x: i32, y: i32) -> String {
    format!(
        r#"<p:pic><p:nvPicPr>{}<p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="pic"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"/></p:spPr></p:pic>"#,
        props(id, hidden),
        transform(x, y, 10, 10)
    )
}
fn connector(id: u32, hidden: &str, x: i32, y: i32) -> String {
    format!(
        r#"<p:cxnSp><p:nvCxnSpPr>{}<p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr>{}<a:prstGeom prst="rect"/><a:ln w="95250"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:ln></p:spPr></p:cxnSp>"#,
        props(id, hidden),
        transform(x, y, 10, 10)
    )
}
fn group(id: u32, hidden: &str, children: &str) -> String {
    format!(
        r#"<p:grpSp><p:nvGrpSpPr>{}<p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/><a:chOff x="0" y="0"/><a:chExt cx="952500" cy="952500"/></a:xfrm></p:grpSpPr>{children}</p:grpSp>"#,
        props(id, hidden)
    )
}
fn pic_parts(rels_path: &'static str, target: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            rels_path,
            format!(
                r#"<Relationships xmlns="{PR}"><Relationship Id="pic" Type="{R}/image" Target="{target}"/></Relationships>"#
            )
            .into_bytes(),
        ),
        (
            "ppt/media/preview.jpg",
            include_bytes!("fixtures/ole-preview.jpg").to_vec(),
        ),
    ]
}
/// Render in strict mode: hidden elements must not produce any diagnostic.
fn render(slide: String, extra: Vec<(&str, Vec<u8>)>) -> resvg::tiny_skia::Pixmap {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("hidden.pptx");
    let out = d.path().join("out");
    write_deck(&input, slide, extra);
    check(run(&input, &out, true));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert!(
        manifest["slides"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{manifest}"
    );
    image(&out)
}

#[test]
fn hidden_slide_elements_are_not_drawn_and_visible_siblings_are() {
    let shapes = [
        shape(2, r#"hidden="1""#, 0, 0, "FF0000"),
        shape(3, r#"hidden="true""#, 20, 0, "FF0000"),
        picture(4, r#"hidden="1""#, 40, 0),
        connector(5, r#"hidden="1""#, 60, 0),
        // Explicitly not hidden: must still render.
        shape(6, r#"hidden="0""#, 0, 50, "00FF00"),
        shape(7, "", 20, 50, "00FF00"),
        picture(8, "", 40, 50),
    ]
    .concat();
    let img = render(
        page("sld", "", "", &shapes),
        pic_parts("ppt/slides/_rels/slide1.xml.rels", "../media/preview.jpg"),
    );
    for x in [5, 25, 45, 65] {
        assert_eq!(rgb(&img, x, 5), WHITE, "hidden element at x={x}");
    }
    assert_eq!(rgb(&img, 5, 55), GREEN);
    assert_eq!(rgb(&img, 25, 55), GREEN);
    assert_ne!(rgb(&img, 45, 55), WHITE);
}

#[test]
fn hidden_group_hides_children_and_visible_group_keeps_visible_children() {
    let hidden = group(
        2,
        r#"hidden="1""#,
        &(shape(3, "", 0, 0, "FF0000") + &shape(4, "", 20, 0, "FF0000")),
    );
    let mixed = group(
        5,
        "",
        &(shape(6, r#"hidden="1""#, 0, 50, "FF0000")
            + &shape(7, "", 20, 50, "00FF00")
            + &group(8, r#"hidden="1""#, &shape(9, "", 40, 50, "FF0000"))),
    );
    let img = render(page("sld", "", "", &(hidden + &mixed)), vec![]);
    assert_eq!(rgb(&img, 5, 5), WHITE);
    assert_eq!(rgb(&img, 25, 5), WHITE);
    assert_eq!(rgb(&img, 5, 55), WHITE);
    assert_eq!(rgb(&img, 25, 55), GREEN);
    assert_eq!(rgb(&img, 45, 55), WHITE);
}

#[test]
fn hidden_layout_shape_and_master_picture_are_not_drawn_on_slides() {
    let master = page(
        "sldMaster",
        "",
        "",
        &(picture(2, r#"hidden="1""#, 0, 0) + &shape(3, "", 20, 0, "00FF00")),
    );
    let layout = page(
        "sldLayout",
        "",
        "",
        &(shape(2, r#"hidden="1""#, 40, 0, "FF0000") + &shape(3, "", 60, 0, "00FF00")),
    );
    let mut extra = vec![
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
    ];
    extra.extend(pic_parts(
        "ppt/masters/_rels/master.xml.rels",
        "../media/preview.jpg",
    ));
    let img = render(page("sld", "", "", &shape(2, "", 80, 0, "00FF00")), extra);
    assert_eq!(rgb(&img, 5, 5), WHITE, "hidden master picture");
    assert_eq!(rgb(&img, 45, 5), WHITE, "hidden layout shape");
    for x in [25, 65, 85] {
        assert_eq!(rgb(&img, x, 5), GREEN, "visible shape at x={x}");
    }
}
