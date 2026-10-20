//! Resolve DrawingML colors and apply color transforms in document order.
use super::*;
/// Resolve a color as `#RRGGBB`, or `#RRGGBBAA` when alpha transforms make it translucent.
pub(super) fn resolve(
    node: Node<'_, '_>,
    default: &str,
    theme: &Theme,
    placeholder: Option<&str>,
    warnings: &mut Warnings,
) -> String {
    match resolve_inner(node, theme, placeholder, warnings) {
        Some((rgb, alpha)) => with_alpha(&rgb, alpha),
        None => default.into(),
    }
}
pub(super) fn shadow_color(
    node: Node<'_, '_>,
    theme: &Theme,
    warnings: &mut Warnings,
) -> (String, f64) {
    with_opacity(node, theme, warnings)
}
/// Resolve `#RRGGBB` and opacity separately (black when unresolvable), for SVG
/// attributes that take opacity on their own, such as shadows and gradient stops.
pub(super) fn with_opacity(
    node: Node<'_, '_>,
    theme: &Theme,
    warnings: &mut Warnings,
) -> (String, f64) {
    resolve_inner(node, theme, None, warnings).unwrap_or_else(|| ("#000000".into(), 1.0))
}
/// Append an 8-bit alpha channel only when the color is not fully opaque.
fn with_alpha(rgb: &str, alpha: f64) -> String {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    if alpha == 255 {
        rgb.into()
    } else {
        format!("{rgb}{alpha:02X}")
    }
}
/// Like `with_opacity`, for one color element itself (for example each `a:duotone` child).
pub(super) fn element_with_opacity(
    element: Node<'_, '_>,
    theme: &Theme,
    warnings: &mut Warnings,
) -> (String, f64) {
    resolve_first(std::iter::once(element), theme, None, warnings)
        .unwrap_or_else(|| ("#000000".into(), 1.0))
}
fn resolve_inner(
    node: Node<'_, '_>,
    theme: &Theme,
    placeholder: Option<&str>,
    warnings: &mut Warnings,
) -> Option<(String, f64)> {
    resolve_first(node.children(), theme, placeholder, warnings)
}
/// The first resolvable color element among `candidates`.
fn resolve_first<'a, 'i: 'a>(
    candidates: impl Iterator<Item = Node<'a, 'i>>,
    theme: &Theme,
    placeholder: Option<&str>,
    warnings: &mut Warnings,
) -> Option<(String, f64)> {
    for c in candidates.filter(|n| n.is_element()) {
        let base = match c.tag_name().name() {
            "prstClr" => match c.attribute("val") {
                Some("black") => Some("#000000"),
                Some("white") => Some("#FFFFFF"),
                _ => None,
            },
            "srgbClr" => c.attribute("val").filter(|s| theme::valid_rgb(s)),
            "sysClr" => c.attribute("lastClr").filter(|s| theme::valid_rgb(s)),
            "schemeClr" => c.attribute("val").and_then(|v| {
                if v == "phClr" {
                    placeholder
                } else {
                    theme.color(v)
                }
            }),
            _ => None,
        };
        let Some(hex) = base.map(|b| b.trim_start_matches('#')) else {
            continue;
        };
        let Some(opaque) = hex.get(..6).filter(|s| theme::valid_rgb(s)) else {
            continue;
        };
        let mut rgb = components(opaque);
        // Keep HSL alongside RGB so hue and saturation survive luminance reaching 0 or 1.
        let mut hsl = to_hsl(rgb);
        let mut changed = false;
        // A placeholder color keeps the alpha of the reference color that supplied it.
        let mut alpha = hex
            .get(6..8)
            .and_then(|a| u8::from_str_radix(a, 16).ok())
            .map_or(1.0, |a| a as f64 / 255.0);
        for transform in c.children().filter(|n| n.is_element()) {
            let value = |default: f64| number(transform, "val", default) / 100000.0;
            let name = transform.tag_name().name();
            match name {
                "lumMod" | "lumOff" | "satMod" => {
                    let (index, scale, offset) = match name {
                        "lumMod" => (2, value(100000.0), 0.0),
                        "lumOff" => (2, 1.0, value(0.0)),
                        _ => (1, value(100000.0), 0.0),
                    };
                    hsl[index] = (hsl[index] * scale + offset).clamp(0.0, 1.0);
                    rgb = from_hsl(hsl);
                    changed = true;
                }
                // ECMA-376 Part 1 20.1.2.3.34 and 20.1.2.3.31: a tint mixes the input with
                // white and a shade with black, in linear RGB (the scRGB gamma).
                "tint" | "shade" => {
                    let amount = value(100000.0).clamp(0.0, 1.0);
                    rgb = rgb.map(|v| {
                        let linear = to_linear(v);
                        to_srgb(if name == "tint" {
                            1.0 - (1.0 - linear) * amount
                        } else {
                            linear * amount
                        })
                    });
                    hsl = to_hsl(rgb);
                    changed = true;
                }
                "alpha" => alpha = value(100000.0).clamp(0.0, 1.0),
                "alphaMod" => alpha = (alpha * value(100000.0)).clamp(0.0, 1.0),
                "alphaOff" => alpha = (alpha + value(0.0)).clamp(0.0, 1.0),
                name => warn(warnings, format!("Unsupported color transform: {name}")),
            }
        }
        if !changed {
            return Some((format!("#{opaque}"), alpha));
        }
        let channel = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        return Some((
            format!(
                "#{:02X}{:02X}{:02X}",
                channel(rgb[0]),
                channel(rgb[1]),
                channel(rgb[2])
            ),
            alpha,
        ));
    }
    warn(warnings, "Cannot resolve color; using a fallback color");
    None
}
fn components(hex: &str) -> [f64; 3] {
    std::array::from_fn(|i| {
        u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0) as f64 / 255.0
    })
}
fn to_hsl([r, g, b]: [f64; 3]) -> [f64; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let light = (max + min) / 2.0;
    let chroma = max - min;
    if chroma == 0.0 {
        return [0.0, 0.0, light];
    }
    let saturation = chroma / (1.0 - (2.0 * light - 1.0).abs());
    let hue = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    [hue, saturation.min(1.0), light]
}
/// Inverse of `to_hsl`; hue is in sextants (0..6).
fn from_hsl([hue, saturation, light]: [f64; 3]) -> [f64; 3] {
    let chroma = (1.0 - (2.0 * light - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hue as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = light - chroma / 2.0;
    [r + m, g + m, b + m]
}
/// IEC 61966-2-1 sRGB transfer functions.
fn to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn to_srgb(v: f64) -> f64 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn resolved(transforms: &str, rgb: &str) -> (String, Vec<String>) {
        let source = format!(
            r#"<a:solidFill xmlns:a="{A}"><a:srgbClr val="{rgb}">{transforms}</a:srgbClr></a:solidFill>"#
        );
        let d = Document::parse(&source).unwrap();
        let mut warnings = Warnings::default();
        let color = resolve(
            d.root_element(),
            "none",
            &Theme::default(),
            None,
            &mut warnings,
        );
        (color, warnings.into_parts().0)
    }
    #[test]
    fn luminance_preserves_hue_and_clamps() {
        let lum = |transforms: &str, rgb| resolved(transforms, rgb).0;
        assert_eq!(lum(r#"<a:lumOff val="25000"/>"#, "0000FF"), "#8080FF");
        assert_eq!(lum(r#"<a:lumMod val="50000"/>"#, "336699"), "#1A334D");
        assert_eq!(
            lum(r#"<a:lumMod val="0"/><a:lumOff val="100000"/>"#, "808080"),
            "#FFFFFF"
        );
        // Hue survives an intermediate black.
        assert_eq!(
            lum(r#"<a:lumMod val="0"/><a:lumOff val="25000"/>"#, "0000FF"),
            "#000080"
        );
    }
    #[test]
    fn tint_shade_and_saturation_follow_documented_formulas() {
        // accent1 4472C4: 20% tint mixes 80% white in linear RGB.
        assert_eq!(resolved(r#"<a:tint val="20000"/>"#, "4472C4").0, "#E9EBF5");
        assert_eq!(resolved(r#"<a:shade val="50000"/>"#, "4472C4").0, "#2F528F");
        assert_eq!(resolved(r#"<a:tint val="100000"/>"#, "4472C4").0, "#4472C4");
        assert_eq!(resolved(r#"<a:shade val="0"/>"#, "4472C4").0, "#000000");
        assert_eq!(resolved(r#"<a:satMod val="0"/>"#, "FF0000").0, "#808080");
        let (_, messages) = resolved(
            r#"<a:tint val="20000"/><a:shade val="50000"/><a:satMod val="120000"/><a:alpha val="50000"/>"#,
            "4472C4",
        );
        assert!(messages.is_empty(), "{messages:?}");
    }
    #[test]
    fn alpha_is_emitted_and_unknown_transforms_are_visible() {
        let (color, messages) = resolved(
            r#"<a:lumMod val="50000"/><a:lumOff val="25000"/><a:alpha val="50000"/><a:hueOff val="1"/>"#,
            "0000FF",
        );
        assert_eq!(color, "#0000FF80");
        assert_eq!(messages, vec!["Unsupported color transform: hueOff"]);
        let (color, _) = resolved(
            r#"<a:alpha val="50000"/><a:alphaMod val="50000"/><a:alphaOff val="-100000"/>"#,
            "000000",
        );
        assert_eq!(color, "#00000000");
        assert_eq!(
            resolved(r#"<a:alpha val="100000"/>"#, "abcdef").0,
            "#abcdef"
        );
    }
}
