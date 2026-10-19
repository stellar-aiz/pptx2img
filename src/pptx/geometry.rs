//! Outlines and text areas independently implemented from ECMA-376 Part 1 presets.
use super::*;
mod guides;

pub(super) struct Geometry {
    pub svg: String,
    /// Top-left position, width, and height in shape-local coordinates.
    pub text: (f64, f64, f64, f64),
}

fn adjustments(node: Node<'_, '_>, defaults: &[(&str, f64)], warnings: &mut Warnings) -> Vec<f64> {
    let mut values: Vec<_> = defaults.iter().map(|(_, value)| *value).collect();
    if let Some(list) = child(node, A, "avLst") {
        for guide in list.children().filter(|n| n.has_tag_name((A, "gd"))) {
            let index = defaults
                .iter()
                .position(|(name, _)| Some(*name) == guide.attribute("name"));
            let value = guide
                .attribute("fmla")
                .and_then(|s| s.strip_prefix("val "))
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite());
            if let (Some(index), Some(value)) = (index, value) {
                values[index] = value;
            } else {
                warn(
                    warnings,
                    "Cannot resolve preset shape adjustment formulas; using defaults",
                );
            }
        }
    }
    values
}

pub(super) fn preset(
    node: Node<'_, '_>,
    w: f64,
    h: f64,
    style: &str,
    warnings: &mut Warnings,
) -> Option<Geometry> {
    let kind = node.attribute("prst")?;
    let definition = guides::find(kind);
    if definition.is_none()
        && !matches!(
            kind,
            "homePlate" | "upArrow" | "rightArrow" | "rightBrace" | "wedgeRoundRectCallout"
        )
    {
        return None;
    }
    // Connectors stay visible as lines when one extent is zero.
    if (w == 0.0 || h == 0.0) && !kind.contains("Connector") {
        return Some(Geometry {
            svg: String::new(),
            text: (0.0, 0.0, w, h),
        });
    }
    if let Some(definition) = definition {
        return Some(guides::render(node, definition, w, h, style, warnings));
    }
    let ss = w.min(h);
    let (path, text) = match kind {
        "homePlate" => {
            let a = adjustments(node, &[("adj", 50000.0)], warnings)[0];
            let x = w - (ss * a / 100000.0).clamp(0.0, w);
            (
                format!("M0 0 H{x} L{w} {} L{x} {h} H0 Z", h / 2.0),
                (0.0, 0.0, (x + w) / 2.0, h),
            )
        }
        "rightArrow" | "upArrow" => {
            let a = adjustments(node, &[("adj1", 50000.0), ("adj2", 50000.0)], warnings);
            // Build the up arrow by swapping axes and transforming the same outline counterclockwise.
            let (u, v) = if kind == "upArrow" { (h, w) } else { (w, h) };
            let head = (ss * a[1] / 100000.0).clamp(0.0, u);
            let x = u - head;
            let y = v * (1.0 - a[0].clamp(0.0, 100000.0) / 100000.0) / 2.0;
            let mut points = vec![
                (0.0, y),
                (x, y),
                (x, 0.0),
                (u, v / 2.0),
                (x, v),
                (x, v - y),
                (0.0, v - y),
            ];
            let right = x + y * head / (v / 2.0);
            let text = if kind == "upArrow" {
                for point in &mut points {
                    *point = (point.1, u - point.0);
                }
                (y, u - right, v - 2.0 * y, right)
            } else {
                (0.0, y, right, v - 2.0 * y)
            };
            let path = points
                .iter()
                .enumerate()
                .map(|(i, (x, y))| format!("{} {x} {y}", if i == 0 { "M" } else { "L" }))
                .collect::<Vec<_>>()
                .join(" ")
                + " Z";
            (path, text)
        }
        "rightBrace" => {
            let a = adjustments(node, &[("adj1", 8333.0), ("adj2", 50000.0)], warnings);
            let mid = h * a[1].clamp(0.0, 100000.0) / 100000.0;
            let radius = (ss * a[0] / 100000.0).clamp(0.0, mid.min(h - mid) / 2.0);
            let half = w / 2.0;
            let path = format!(
                "M0 0 A{half} {radius} 0 0 1 {half} {radius} V{} A{half} {radius} 0 0 0 {w} {mid} A{half} {radius} 0 0 0 {half} {} V{} A{half} {radius} 0 0 1 0 {h}",
                mid - radius,
                mid + radius,
                h - radius
            );
            let inset = radius * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
            // Close the fill without adding an outline at the left edge.
            return Some(Geometry {
                svg: format!(
                    "<g {style}><path d=\"{path} Z\" stroke=\"none\"/><path d=\"{path}\" fill=\"none\"/></g>"
                ),
                text: (
                    0.0,
                    inset,
                    half * std::f64::consts::FRAC_1_SQRT_2,
                    h - 2.0 * inset,
                ),
            });
        }
        _ => {
            let a = adjustments(
                node,
                &[("adj1", -20833.0), ("adj2", 62500.0), ("adj3", 16667.0)],
                warnings,
            );
            let dx = w * a[0] / 100000.0;
            let dy = h * a[1] / 100000.0;
            let (xp, yp) = (w / 2.0 + dx, h / 2.0 + dy);
            let vertical = dy.abs() > (dx * h / w).abs();
            let (x1, x2) = if dx > 0.0 {
                (w * 7.0 / 12.0, w * 10.0 / 12.0)
            } else {
                (w * 2.0 / 12.0, w * 5.0 / 12.0)
            };
            let (y1, y2) = if dy > 0.0 {
                (h * 7.0 / 12.0, h * 10.0 / 12.0)
            } else {
                (h * 2.0 / 12.0, h * 5.0 / 12.0)
            };
            let top = if vertical && dy <= 0.0 {
                (xp, yp)
            } else {
                (x1, 0.0)
            };
            let bottom = if vertical && dy > 0.0 {
                (xp, yp)
            } else {
                (x1, h)
            };
            let left = if !vertical && dx <= 0.0 {
                (xp, yp)
            } else {
                (0.0, y1)
            };
            let right = if !vertical && dx > 0.0 {
                (xp, yp)
            } else {
                (w, y1)
            };
            let r = (ss * a[2] / 100000.0).clamp(0.0, ss / 2.0);
            let path = format!(
                "M0 {r} A{r} {r} 0 0 1 {r} 0 L{x1} 0 L{} {} L{x2} 0 H{} A{r} {r} 0 0 1 {w} {r} L{w} {y1} L{} {} L{w} {y2} V{} A{r} {r} 0 0 1 {} {h} L{x2} {h} L{} {} L{x1} {h} H{r} A{r} {r} 0 0 1 0 {} L0 {y2} L{} {} L0 {y1} Z",
                top.0,
                top.1,
                w - r,
                right.0,
                right.1,
                h - r,
                w - r,
                bottom.0,
                bottom.1,
                h - r,
                left.0,
                left.1
            );
            let inset = r * 0.29289;
            (path, (inset, inset, w - 2.0 * inset, h - 2.0 * inset))
        }
    };
    Some(Geometry {
        svg: format!("<path d=\"{path}\" {style}/>"),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_rect_follows_adjusted_outline_and_degenerate_size_is_finite() {
        let source = format!(
            r#"<a:prstGeom xmlns:a="{A}" prst="homePlate"><a:avLst><a:gd name="adj" fmla="val 100000"/></a:avLst></a:prstGeom>"#
        );
        let doc = Document::parse(&source).unwrap();
        let value = preset(doc.root_element(), 60.0, 40.0, "", &mut Warnings::default()).unwrap();
        assert_eq!(value.text, (0.0, 0.0, 40.0, 40.0));
        let zero = preset(doc.root_element(), 0.0, 40.0, "", &mut Warnings::default()).unwrap();
        assert!(zero.svg.is_empty());
    }
}
