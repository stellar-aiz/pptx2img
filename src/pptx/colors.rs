//! DrawingML の色を解決し、HSL 明度変換を記述順に適用する。
use super::*;
pub(super) fn resolve(
    node: Node<'_, '_>,
    default: &str,
    theme: &Theme,
    placeholder: Option<&str>,
    warnings: &mut Warnings,
) -> String {
    resolve_inner(node, default, theme, placeholder, false, warnings).0
}
pub(super) fn shadow_color(
    node: Node<'_, '_>,
    theme: &Theme,
    warnings: &mut Warnings,
) -> (String, f64) {
    resolve_inner(node, "#000000", theme, None, true, warnings)
}
fn resolve_inner(
    node: Node<'_, '_>,
    default: &str,
    theme: &Theme,
    placeholder: Option<&str>,
    alpha_enabled: bool,
    warnings: &mut Warnings,
) -> (String, f64) {
    for c in node.children().filter(|n| n.is_element()) {
        let rgb = match c.tag_name().name() {
            "prstClr" => match c.attribute("val") {
                Some("black") => Some("#000000".into()),
                Some("white") => Some("#FFFFFF".into()),
                _ => None,
            },
            "srgbClr" => c
                .attribute("val")
                .filter(|s| theme::valid_rgb(s))
                .map(|s| format!("#{s}")),
            "sysClr" => c
                .attribute("lastClr")
                .filter(|s| theme::valid_rgb(s))
                .map(|s| format!("#{s}")),
            "schemeClr" => c
                .attribute("val")
                .and_then(|v| {
                    if v == "phClr" {
                        placeholder
                    } else {
                        theme.color(v)
                    }
                })
                .map(str::to_owned),
            _ => None,
        };
        if let Some(rgb) = rgb {
            let components = components(&rgb);
            let mut light = (components.iter().copied().fold(0.0, f64::max)
                + components.iter().copied().fold(1.0, f64::min))
                / 2.0;
            let mut changed = false;
            let mut alpha = 1.0;
            for transform in c.children().filter(|n| n.is_element()) {
                match transform.tag_name().name() {
                    "lumMod" => {
                        light =
                            (light * number(transform, "val", 100000.0) / 100000.0).clamp(0.0, 1.0);
                        changed = true;
                    }
                    "lumOff" => {
                        light = (light + number(transform, "val", 0.0) / 100000.0).clamp(0.0, 1.0);
                        changed = true;
                    }
                    "alpha" if alpha_enabled => {
                        alpha = (number(transform, "val", 100000.0) / 100000.0).clamp(0.0, 1.0)
                    }
                    "alphaMod" if alpha_enabled => {
                        alpha =
                            (alpha * number(transform, "val", 100000.0) / 100000.0).clamp(0.0, 1.0)
                    }
                    "alphaOff" if alpha_enabled => {
                        alpha = (alpha + number(transform, "val", 0.0) / 100000.0).clamp(0.0, 1.0)
                    }
                    name => warn(warnings, format!("未対応の色変換: {name}")),
                }
            }
            return (
                if changed {
                    luminance(&rgb, 0.0, light)
                } else {
                    rgb
                },
                alpha,
            );
        }
    }
    warn(warnings, "色を解決できないため代替色で描画しました");
    (default.into(), 1.0)
}
fn components(rgb: &str) -> [f64; 3] {
    std::array::from_fn(|i| {
        u8::from_str_radix(&rgb[1 + i * 2..3 + i * 2], 16).unwrap_or(0) as f64 / 255.0
    })
}
fn luminance(rgb: &str, multiplier: f64, offset: f64) -> String {
    let values = components(rgb);
    let max = values.iter().copied().fold(0.0, f64::max);
    let min = values.iter().copied().fold(1.0, f64::min);
    let old = (max + min) / 2.0;
    let new = (old * multiplier + offset).clamp(0.0, 1.0);
    let chroma = max - min;
    let saturation = if chroma == 0.0 {
        0.0
    } else {
        chroma / (1.0 - (2.0 * old - 1.0).abs())
    };
    let new_chroma = (1.0 - (2.0 * new - 1.0).abs()) * saturation;
    let channel = |v: f64| -> u8 {
        let value = if chroma == 0.0 {
            new
        } else {
            (v - min) / chroma * new_chroma + new - new_chroma / 2.0
        };
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    format!(
        "#{:02X}{:02X}{:02X}",
        channel(values[0]),
        channel(values[1]),
        channel(values[2])
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn luminance_preserves_hue_and_clamps() {
        assert_eq!(luminance("#0000FF", 1.0, 0.25), "#8080FF");
        assert_eq!(luminance("#336699", 0.5, 0.0), "#1A334D");
        assert_eq!(luminance("#808080", 0.0, 1.0), "#FFFFFF");
    }
    #[test]
    fn transform_order_and_unknown_are_visible() {
        let source = format!(
            r#"<a:solidFill xmlns:a="{A}"><a:srgbClr val="0000FF"><a:lumMod val="50000"/><a:lumOff val="25000"/><a:alpha val="50000"/></a:srgbClr></a:solidFill>"#
        );
        let d = Document::parse(&source).unwrap();
        let mut warnings = Warnings::default();
        assert_eq!(
            resolve(
                d.root_element(),
                "none",
                &Theme::default(),
                None,
                &mut warnings
            ),
            "#0000FF"
        );
        assert!(!warnings.into_parts().0.is_empty());
    }
}
