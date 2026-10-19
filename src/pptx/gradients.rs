//! Map DrawingML gradient fills (ECMA-376 Part 1 §20.1.8.33) to SVG gradients.
use super::*;

/// Add a gradient definition to `svg` and return its paint reference, or `None` when the
/// gradient has no usable stops. `size` is the fill box in local coordinates.
pub(super) fn define(
    node: Node<'_, '_>,
    theme: &Theme,
    size: (f64, f64),
    rotated: bool,
    svg: &mut String,
    warnings: &mut Warnings,
) -> Option<String> {
    let (w, h) = size;
    let mut stops: Vec<(f64, String)> = child(node, A, "gsLst")
        .into_iter()
        .flat_map(|list| list.children().filter(|n| n.has_tag_name((A, "gs"))))
        .map(|stop| {
            let offset = (number(stop, "pos", 0.0) / 100000.0).clamp(0.0, 1.0);
            (
                offset,
                colors::resolve(stop, "#000000", theme, None, warnings),
            )
        })
        .collect();
    if stops.is_empty() {
        warn(
            warnings,
            "Gradient fill has no stops; using the default fill",
        );
        return None;
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let stops = stops
        .iter()
        .map(|(offset, color)| {
            // SVG stop colors carry opacity separately.
            let (rgb, alpha) = color.split_at(7.min(color.len()));
            let opacity = u8::from_str_radix(alpha, 16).map_or(1.0, |a| a as f64 / 255.0);
            format!("<stop offset=\"{offset}\" stop-color=\"{rgb}\" stop-opacity=\"{opacity}\"/>")
        })
        .collect::<String>();
    if rotated && theme::bool_attr(node, "rotWithShape") == Some(false) {
        warnings.push(
            Code::Approximation,
            "Gradient fills with rotWithShape=\"0\" rotate with the shape",
        );
    }
    if child(node, A, "tileRect").is_some_and(|n| n.attributes().any(|a| a.value() != "0")) {
        warnings.push(Code::Approximation, "Gradient tile rectangles are ignored");
    }
    let id = format!("gradient-{}", svg.len());
    let units = "gradientUnits=\"userSpaceOnUse\"";
    if let Some(path) = child(node, A, "path") {
        let kind = path.attribute("path").unwrap_or("circle");
        // fillToRect insets locate the focus; stop position 0 sits at its center.
        let inset =
            |key| child(path, A, "fillToRect").map_or(0.0, |r| number(r, key, 0.0) / 100000.0);
        let (l, t, r, b) = (inset("l"), inset("t"), inset("r"), inset("b"));
        let (fx, fy) = (w * (l + 1.0 - r) / 2.0, h * (t + 1.0 - b) / 2.0);
        let radius = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
            .iter()
            .map(|(x, y)| ((x - fx).powi(2) + (y - fy).powi(2)).sqrt())
            .fold(0.0, f64::max);
        // Only a centered point focus is verified; elsewhere the radius is a guess.
        let centered = [l, t, r, b].iter().all(|v| (v - 0.5).abs() < 1e-9);
        if kind != "circle" || !centered {
            warnings.push(
                Code::Approximation,
                format!(
                    "Gradient path \"{kind}\" is approximated by a circular radial gradient from the focus"
                ),
            );
        }
        svg.push_str(&format!(
            "<defs><radialGradient id=\"{id}\" {units} cx=\"{fx}\" cy=\"{fy}\" r=\"{radius}\">{stops}</radialGradient></defs>"
        ));
    } else {
        let lin = child(node, A, "lin");
        let angle =
            lin.map_or(0.0, |n| number(n, "ang", 0.0)) / 60000.0 * std::f64::consts::PI / 180.0;
        let (dx, dy) = (angle.cos(), angle.sin());
        // With `scaled`, the angle applies to a unit square stretched over the box.
        let scaled = lin.and_then(|n| theme::bool_attr(n, "scaled")) == Some(true);
        if scaled && (w - h).abs() > 1e-9 && (dx * dy).abs() > 1e-9 {
            warnings.push(
                Code::Approximation,
                "Scaled gradient angles are approximated by stretching the gradient with the shape",
            );
        }
        let (bw, bh, transform) = if scaled {
            (1.0, 1.0, format!(" gradientTransform=\"scale({w} {h})\""))
        } else {
            (w, h, String::new())
        };
        let half = (bw * dx.abs() + bh * dy.abs()) / 2.0;
        let (mx, my) = (bw / 2.0, bh / 2.0);
        svg.push_str(&format!(
            "<defs><linearGradient id=\"{id}\" {units}{transform} x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{stops}</linearGradient></defs>",
            mx - half * dx,
            my - half * dy,
            mx + half * dx,
            my + half * dy
        ));
    }
    Some(format!("url(#{id})"))
}
