//! Picture fills: `a:blip` effects as one SVG filter, `a:tile` as an SVG pattern, and
//! `a:stretch/a:fillRect` insets.
//!
//! Effects are applied in document order, each primitive reading the previous result.
//! All primitives work on sRGB values (`color-interpolation-filters="sRGB"`).
use super::*;

/// Rec. 709 luma weights, as SVG `feColorMatrix type="saturate"` uses.
const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// Chained filter primitives; each one reads the previous result.
struct Chain {
    primitives: String,
    last: String,
}
impl Chain {
    fn push(&mut self, primitive: impl FnOnce(&str, &str) -> String) {
        let result = format!("e{}", self.primitives.len());
        self.primitives.push_str(&primitive(&self.last, &result));
        self.last = result;
    }
}

/// A row-major 4x5 `feColorMatrix` primitive.
fn matrix(values: [[f64; 5]; 4]) -> impl FnOnce(&str, &str) -> String {
    let values = values
        .iter()
        .flatten()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    move |input, result| {
        format!(
            "<feColorMatrix in=\"{input}\" type=\"matrix\" values=\"{values}\" result=\"{result}\"/>"
        )
    }
}
/// SVG saturate matrix; unlike `type="saturate"` in resvg, `s` above 1 is kept.
fn saturate(s: f64) -> [[f64; 5]; 4] {
    let [r, g, b] = LUMA;
    [
        [r + (1.0 - r) * s, g - g * s, b - b * s, 0.0, 0.0],
        [r - r * s, g + (1.0 - g) * s, b - b * s, 0.0, 0.0],
        [r - r * s, g - g * s, b + (1.0 - b) * s, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]
}
/// Luma in every color channel, plus `offset`.
fn luma(offset: f64) -> [[f64; 5]; 4] {
    let [r, g, b] = LUMA;
    let row = [r, g, b, 0.0, offset];
    [row, row, row, [0.0, 0.0, 0.0, 1.0, 0.0]]
}
/// `feComponentTransfer` with the given functions (`"R"`, `"G"`, `"B"`, `"A"` with attributes).
fn transfer(functions: Vec<(&'static str, String)>) -> impl FnOnce(&str, &str) -> String {
    move |input, result| {
        let functions: String = functions
            .iter()
            .map(|(channel, attrs)| format!("<feFunc{channel} {attrs}/>"))
            .collect();
        format!(
            "<feComponentTransfer in=\"{input}\" result=\"{result}\">{functions}</feComponentTransfer>"
        )
    }
}
/// One transfer function per color channel, from the channel index.
fn rgb(attrs: impl Fn(usize) -> String) -> Vec<(&'static str, String)> {
    ["R", "G", "B"]
        .into_iter()
        .enumerate()
        .map(|(i, c)| (c, attrs(i)))
        .collect()
}
fn rgb_linear(slope: f64, intercept: f64) -> Vec<(&'static str, String)> {
    rgb(|_| format!("type=\"linear\" slope=\"{slope}\" intercept=\"{intercept}\""))
}
/// Brightness and contrast (fractions in -1..1) as one linear map around mid-grey.
// stellar:debt(scope) Office's exact brightness/contrast curve is not specified. upgrade: fit against PowerPoint renders.
fn brightness_contrast(chain: &mut Chain, bright: f64, contrast: f64, warnings: &mut Warnings) {
    if bright == 0.0 && contrast == 0.0 {
        return;
    }
    let contrast = contrast.clamp(-1.0, 0.99);
    let slope = if contrast >= 0.0 {
        1.0 / (1.0 - contrast)
    } else {
        1.0 + contrast
    };
    chain.push(transfer(rgb_linear(slope, 0.5 - 0.5 * slope + bright)));
    warnings.push(
        Code::Approximation,
        "Picture brightness and contrast are approximated",
    );
}
/// `#RRGGBB` as 0..1 channels.
fn channels(hex: &str) -> [f64; 3] {
    colors::components(hex.trim_start_matches('#'))
}
fn percent(node: Node<'_, '_>, attr: &str, default: f64) -> f64 {
    number(node, attr, default) / 100000.0
}
/// An Office 2010 picture correction (`a14:imgEffect` child).
fn office_effect(effect: Node<'_, '_>, chain: &mut Chain, warnings: &mut Warnings) {
    match effect.tag_name().name() {
        "saturation" => {
            let s = percent(effect, "sat", 100000.0).max(0.0);
            if s != 1.0 {
                chain.push(matrix(saturate(s)));
            }
        }
        "brightnessContrast" => brightness_contrast(
            chain,
            percent(effect, "bright", 0.0),
            percent(effect, "contrast", 0.0),
            warnings,
        ),
        // Background removal stores only marks; the removed area is not in the package.
        other => warn(warnings, format!("Unsupported picture effect: {other}")),
    }
}
fn elements<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(|c| c.is_element())
}
/// Extensions of `a:blip/a:extLst`: SVG pictures and local-DPI flags draw nothing.
fn extensions(list: Node<'_, '_>, chain: &mut Chain, warnings: &mut Warnings) {
    let allow = [(ASVG, "svgBlip"), (A14, "useLocalDpi"), (A14, "imgProps")];
    for item in unsupported_extensions(list, &allow) {
        warn(
            warnings,
            format!("Unsupported picture extension: {}", item.tag_name().name()),
        );
    }
    let effects = elements(list)
        .flat_map(|ext| elements(ext))
        .filter(|n| n.has_tag_name((A14, "imgProps")))
        .flat_map(|props| props.children())
        .filter(|n| n.has_tag_name((A14, "imgLayer")))
        .flat_map(|layer| layer.children())
        .filter(|n| n.has_tag_name((A14, "imgEffect")))
        .flat_map(|effect| elements(effect));
    for effect in effects {
        office_effect(effect, chain, warnings);
    }
}
/// Replace `from` with `to` where all three channels match exactly.
// stellar:debt(scope) exact match without tolerance; JPEG noise around the color stays. upgrade: measure Office's tolerance.
fn color_change(chain: &mut Chain, from: [f64; 3], to: (&str, f64), warnings: &mut Warnings) {
    let source = chain.last.clone();
    let indicator = |value: f64| {
        let hit = (value * 255.0).round() as usize;
        let table: Vec<_> = (0..256).map(|i| if i == hit { "1" } else { "0" }).collect();
        format!("type=\"discrete\" tableValues=\"{}\"", table.join(" "))
    };
    chain.push(transfer(vec![
        ("R", indicator(from[0])),
        ("G", indicator(from[1])),
        ("B", indicator(from[2])),
    ]));
    // Alpha is 1 only where all three indicators are 1.
    chain.push(matrix([
        [0.0; 5],
        [0.0; 5],
        [0.0; 5],
        [1.0, 1.0, 1.0, 0.0, -2.0],
    ]));
    let source_in = source.clone();
    chain.push(move |input, result| {
        format!(
            "<feComposite in=\"{input}\" in2=\"{source_in}\" operator=\"in\" result=\"{result}\"/>"
        )
    });
    let mask = chain.last.clone();
    let (color, opacity) = to;
    let flood = format!("{mask}f");
    let kept = format!("{mask}k");
    let color = color.to_string();
    chain.push(move |_, result| {
        format!("<feFlood flood-color=\"{color}\" flood-opacity=\"{opacity}\" result=\"{flood}\"/><feComposite in=\"{flood}\" in2=\"{mask}\" operator=\"in\" result=\"{flood}m\"/><feComposite in=\"{source}\" in2=\"{mask}\" operator=\"out\" result=\"{kept}\"/><feComposite in=\"{flood}m\" in2=\"{kept}\" operator=\"over\" result=\"{result}\"/>")
    });
    warnings.push(
        Code::Approximation,
        "Picture color change is approximated with an exact color match",
    );
}
/// Translate the children of `blip` into one SVG filter appended to `defs`.
/// Returns the filter id, or `None` when no child changes pixels.
pub(super) fn filter(
    blip: Node<'_, '_>,
    theme: &Theme,
    frame: (f64, f64),
    seed: usize,
    defs: &mut String,
    warnings: &mut Warnings,
) -> Option<String> {
    let mut chain = Chain {
        primitives: String::new(),
        last: "SourceGraphic".into(),
    };
    for effect in elements(blip) {
        match effect.tag_name().name() {
            "extLst" => extensions(effect, &mut chain, warnings),
            "alphaModFix" => {
                let amount = percent(effect, "amt", 100000.0).clamp(0.0, 1.0);
                if amount != 1.0 {
                    chain.push(transfer(vec![(
                        "A",
                        format!("type=\"linear\" slope=\"{amount}\""),
                    )]));
                }
            }
            "lum" => brightness_contrast(
                &mut chain,
                percent(effect, "bright", 0.0),
                percent(effect, "contrast", 0.0),
                warnings,
            ),
            "grayscl" => chain.push(matrix(saturate(0.0))),
            "biLevel" => {
                // Shift luma so the threshold lands on 0.5, then step at 0.5.
                let threshold = percent(effect, "thresh", 50000.0);
                chain.push(matrix(luma(0.5 - threshold)));
                chain.push(transfer(rgb(|_| {
                    "type=\"discrete\" tableValues=\"0 1\"".into()
                })));
            }
            "duotone" => {
                let colors: Vec<_> = elements(effect)
                    .map(|n| channels(&colors::element_with_opacity(n, theme, warnings).0))
                    .collect();
                if let [dark, light] = colors[..] {
                    chain.push(matrix(luma(0.0)));
                    chain.push(transfer(rgb(|i| {
                        format!("type=\"table\" tableValues=\"{} {}\"", dark[i], light[i])
                    })));
                    warnings.push(Code::Approximation, "Picture duotone is approximated");
                } else {
                    warn(warnings, "Unsupported picture effect: duotone");
                }
            }
            "clrChange" => {
                let from =
                    child(effect, A, "clrFrom").map(|n| colors::with_opacity(n, theme, warnings));
                let to =
                    child(effect, A, "clrTo").map(|n| colors::with_opacity(n, theme, warnings));
                if let (Some((from, _)), Some((to, alpha))) = (from, to) {
                    color_change(&mut chain, channels(&from), (&to, alpha), warnings);
                } else {
                    warn(warnings, "Unsupported picture effect: clrChange");
                }
            }
            other => warn(warnings, format!("Unsupported picture effect: {other}")),
        }
    }
    if chain.primitives.is_empty() {
        return None;
    }
    let id = format!("picture-filter-{seed}");
    let (w, h) = frame;
    defs.push_str(&format!("<filter id=\"{id}\" filterUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" color-interpolation-filters=\"sRGB\">{}</filter>", chain.primitives));
    Some(id)
}

/// Crop fractions `[left, top, right, bottom]` from `a:srcRect`.
pub(super) type Crop = [f64; 4];
/// `href` cropped by `crop` and stretched over the rectangle `(x, y, w, h)`. With `clip`,
/// the cropped-away parts are clipped to that rectangle; without it the caller's
/// viewport clips (the rectangle is then that whole viewport).
pub(super) fn image(href: &str, crop: Crop, rect: (f64, f64, f64, f64), clip: bool) -> String {
    let [left, top, right, bottom] = crop;
    let (x, y, w, h) = rect;
    let iw = w / (1.0 - left - right);
    let ih = h / (1.0 - top - bottom);
    let image = format!(
        "<image x=\"{}\" y=\"{}\" width=\"{iw}\" height=\"{ih}\" preserveAspectRatio=\"none\" href=\"{href}\"/>",
        -left * iw,
        -top * ih
    );
    if clip {
        format!(
            "<svg x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" overflow=\"hidden\">{image}</svg>"
        )
    } else {
        image
    }
}
/// The rectangle `a:stretch/a:fillRect` insets from a `w` x `h` frame; negative insets extend it.
pub(super) fn fill_rect(blip_fill: Option<Node<'_, '_>>, w: f64, h: f64) -> (f64, f64, f64, f64) {
    let Some(rect) = blip_fill
        .and_then(|n| child(n, A, "stretch"))
        .and_then(|n| child(n, A, "fillRect"))
    else {
        return (0.0, 0.0, w, h);
    };
    let [l, t, r, b] = ["l", "t", "r", "b"].map(|k| percent(rect, k, 0.0));
    (l * w, t * h, w * (1.0 - l - r), h * (1.0 - t - b))
}
/// Largest number of tiles along one axis of the frame.
const MAX_TILES_PER_AXIS: f64 = 256.0;
/// Largest pattern cell, as a multiple of the frame area (the cell is rasterized whole).
const MAX_CELL_AREA_RATIO: f64 = 16.0;
/// Define an `a:tile` fill of `href` (natural size `natural` px, cropped by `crop`) over a
/// `frame` and return the pattern id; fill a shape with `url(#id)`. Returns `None` when
/// the tile is so small or so large that the pattern would defeat the picture size
/// policy; the caller then draws the picture stretched.
pub(super) fn tile_pattern(
    tile: Node<'_, '_>,
    href: &str,
    natural: (f64, f64),
    crop: Crop,
    frame: (f64, f64),
    seed: usize,
    defs: &mut String,
) -> Option<String> {
    let [left, top, right, bottom] = crop;
    let tw = natural.0 * (1.0 - left - right) * percent(tile, "sx", 100000.0).abs();
    let th = natural.1 * (1.0 - top - bottom) * percent(tile, "sy", 100000.0).abs();
    let (w, h) = frame;
    let flip = tile.attribute("flip").unwrap_or("none");
    let (fx, fy) = (flip.contains('x'), flip.contains('y'));
    // A flip mirrors every other tile, so one pattern cell holds up to 2x2 tiles.
    let cw = if fx { 2.0 * tw } else { tw };
    let ch = if fy { 2.0 * th } else { th };
    let bounded = tw.is_finite()
        && th.is_finite()
        && tw * MAX_TILES_PER_AXIS >= w
        && th * MAX_TILES_PER_AXIS >= h
        && cw * ch <= MAX_CELL_AREA_RATIO * w * h;
    if !bounded {
        return None;
    }
    // Alignment places the first tile after scaling and before the offset.
    let algn = tile.attribute("algn").unwrap_or("tl");
    let x = match algn {
        "t" | "ctr" | "b" => (w - tw) / 2.0,
        "tr" | "r" | "br" => w - tw,
        _ => 0.0,
    } + number(tile, "tx", 0.0) / EMU;
    let y = match algn {
        "l" | "ctr" | "r" => (h - th) / 2.0,
        "bl" | "b" | "br" => h - th,
        _ => 0.0,
    } + number(tile, "ty", 0.0) / EMU;
    let id = format!("picture-tile-{seed}");
    // The image is defined once and mirrored by reference, so the data URL is not copied.
    let mut cell = format!(
        "<g id=\"{id}-image\">{}</g>",
        image(href, crop, (0.0, 0.0, tw, th), true)
    );
    let mut mirrored = |sx: f64, sy: f64| {
        let (dx, dy) = ((1.0 - sx) * tw, (1.0 - sy) * th);
        cell.push_str(&format!(
            "<use href=\"#{id}-image\" transform=\"translate({dx} {dy}) scale({sx} {sy})\"/>"
        ));
    };
    if fx {
        mirrored(-1.0, 1.0);
    }
    if fy {
        mirrored(1.0, -1.0);
    }
    if fx && fy {
        mirrored(-1.0, -1.0);
    }
    defs.push_str(&format!("<pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" x=\"{x}\" y=\"{y}\" width=\"{cw}\" height=\"{ch}\">{cell}</pattern>"));
    Some(id)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn blip_filter(inner: &str) -> (Option<String>, String, Warnings) {
        let xml = format!(
            r#"<a:blip xmlns:a="{A}" xmlns:a14="{A14}" xmlns:asvg="{ASVG}">{inner}</a:blip>"#
        );
        let doc = Document::parse(&xml).unwrap();
        let theme = Theme::default();
        let mut defs = String::new();
        let mut warnings = Warnings::default();
        let id = filter(
            doc.root_element(),
            &theme,
            (10.0, 10.0),
            7,
            &mut defs,
            &mut warnings,
        );
        (id, defs, warnings)
    }
    fn codes(warnings: &Warnings) -> Vec<(String, String)> {
        warnings
            .records
            .iter()
            .map(|r| {
                (
                    serde_json::to_value(r.code)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string(),
                    r.message.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn metadata_extensions_draw_nothing() {
        let (id, defs, warnings) = blip_filter(
            r#"<a:extLst><a:ext uri="x"><a14:useLocalDpi val="0"/></a:ext><a:ext uri="y"><asvg:svgBlip/></a:ext></a:extLst><a:alphaModFix/><a:lum/>"#,
        );
        assert_eq!((id, defs.as_str()), (None, ""));
        assert!(codes(&warnings).is_empty());
    }
    #[test]
    fn each_effect_adds_its_primitive() {
        for (inner, primitive) in [
            (
                r#"<a:alphaModFix amt="50000"/>"#,
                r#"<feFuncA type="linear" slope="0.5"/>"#,
            ),
            ("<a:grayscl/>", "feColorMatrix"),
            (r#"<a:biLevel thresh="25000"/>"#, r#"tableValues="0 1""#),
            (r#"<a:lum bright="20000"/>"#, r#"intercept="0.2""#),
        ] {
            let (id, defs, _) = blip_filter(inner);
            assert_eq!(id.as_deref(), Some("picture-filter-7"), "{inner}");
            assert!(
                defs.starts_with(r#"<filter id="picture-filter-7""#),
                "{defs}"
            );
            assert!(defs.contains(primitive), "{inner}: {defs}");
        }
    }
    #[test]
    fn office_corrections_are_mapped_or_named() {
        let effect = |inner: &str| {
            format!(
                r#"<a:extLst><a:ext uri="z"><a14:imgProps><a14:imgLayer><a14:imgEffect>{inner}</a14:imgEffect></a14:imgLayer></a14:imgProps></a:ext></a:extLst>"#
            )
        };
        let (id, _, warnings) = blip_filter(&effect(r#"<a14:saturation sat="0"/>"#));
        assert!(id.is_some());
        assert!(codes(&warnings).is_empty());
        let (id, _, warnings) = blip_filter(&effect(
            "<a14:backgroundRemoval t=\"0\" b=\"1\" l=\"0\" r=\"1\"/>",
        ));
        assert!(id.is_none());
        assert_eq!(
            codes(&warnings),
            [(
                "unsupported".to_string(),
                "Unsupported picture effect: backgroundRemoval".to_string()
            )]
        );
    }
    #[test]
    fn duotone_and_color_change_are_approximations() {
        let (_, defs, warnings) = blip_filter(
            r#"<a:duotone><a:srgbClr val="000080"/><a:prstClr val="white"/></a:duotone><a:clrChange><a:clrFrom><a:srgbClr val="FFFFFF"/></a:clrFrom><a:clrTo><a:srgbClr val="FFFFFF"><a:alpha val="0"/></a:srgbClr></a:clrTo></a:clrChange>"#,
        );
        assert!(defs.contains(r#"tableValues="0 1""#), "{defs}");
        assert!(defs.contains("operator=\"out\""), "{defs}");
        let found = codes(&warnings);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|(code, _)| code == "approximation"));
    }
    #[test]
    fn tile_pattern_scales_aligns_and_flips() {
        let xml = format!(
            r#"<a:tile xmlns:a="{A}" sx="50000" sy="50000" tx="95250" ty="0" flip="x" algn="ctr"/>"#
        );
        let doc = Document::parse(&xml).unwrap();
        let mut defs = String::new();
        let id = tile_pattern(
            doc.root_element(),
            "data:x",
            (40.0, 20.0),
            [0.0; 4],
            (100.0, 50.0),
            3,
            &mut defs,
        );
        assert_eq!(id.as_deref(), Some("picture-tile-3"));
        // 20x10 tile, centred (40, 20), offset 10 px; the x flip doubles the cell width.
        assert!(
            defs.starts_with(r#"<pattern id="picture-tile-3" patternUnits="userSpaceOnUse" x="50" y="20" width="40" height="10">"#),
            "{defs}"
        );
        assert!(defs.contains("scale(-1 1)"), "{defs}");
        assert_eq!(defs.matches("data:x").count(), 1, "{defs}");
    }
    #[test]
    fn absurd_tile_scales_are_refused() {
        for scale in ["1", "100000000", "0"] {
            let xml = format!(r#"<a:tile xmlns:a="{A}" sx="{scale}" sy="{scale}"/>"#);
            let doc = Document::parse(&xml).unwrap();
            let mut defs = String::new();
            let id = tile_pattern(
                doc.root_element(),
                "data:x",
                (100.0, 100.0),
                [0.0; 4],
                (100.0, 100.0),
                0,
                &mut defs,
            );
            assert_eq!((id, defs.as_str()), (None, ""), "sx={scale}");
        }
    }
}
