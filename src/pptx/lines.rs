//! DrawingML の線種・端部を SVG の線属性とマーカーへ変換する。
use super::*;

pub(super) fn decorations(
    line: Node<'_, '_>,
    stroke: &str,
    width: f64,
    svg: &mut String,
    warnings: &mut Warnings,
) -> String {
    let cap = match line.attribute("cap").unwrap_or("flat") {
        "flat" => "butt",
        "rnd" => "round",
        "sq" => "square",
        other => {
            warn(warnings, format!("未対応の線端: {other}"));
            "butt"
        }
    };
    let mut attrs = format!("stroke-linecap=\"{cap}\"");
    let join = if child(line, A, "round").is_some() {
        "round"
    } else if child(line, A, "bevel").is_some() {
        "bevel"
    } else {
        "miter"
    };
    attrs.push_str(&format!(" stroke-linejoin=\"{join}\""));
    if let Some(miter) = child(line, A, "miter") {
        attrs.push_str(&format!(
            " stroke-miterlimit=\"{}\"",
            (number(miter, "lim", 800000.0) / 100000.0).max(1.0)
        ));
    }
    if let Some(dash) = child(line, A, "prstDash") {
        let pattern: Option<&[f64]> = match dash.attribute("val").unwrap_or("solid") {
            "solid" => Some(&[]),
            "dot" => Some(&[1.0, 3.0]),
            "dash" => Some(&[4.0, 3.0]),
            "lgDash" => Some(&[8.0, 3.0]),
            "dashDot" => Some(&[4.0, 3.0, 1.0, 3.0]),
            "lgDashDot" => Some(&[8.0, 3.0, 1.0, 3.0]),
            "lgDashDotDot" => Some(&[8.0, 3.0, 1.0, 3.0, 1.0, 3.0]),
            "sysDash" => Some(&[3.0, 1.0]),
            "sysDot" => Some(&[1.0, 1.0]),
            "sysDashDot" => Some(&[3.0, 1.0, 1.0, 1.0]),
            "sysDashDotDot" => Some(&[3.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
            _ => None,
        };
        if let Some(pattern) = pattern {
            if !pattern.is_empty() && width > 0.0 {
                attrs.push_str(&format!(
                    " stroke-dasharray=\"{}\"",
                    pattern
                        .iter()
                        .map(|v| (v * width).to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        } else {
            warn(warnings, "未対応のプリセット破線です");
        }
    }
    if ["headEnd", "tailEnd"].iter().any(|tag| {
        child(line, A, tag).is_some_and(|n| {
            n.attribute("type")
                .is_some_and(|t| matches!(t, "triangle" | "stealth" | "arrow" | "diamond" | "oval"))
        })
    }) {
        // stellar:debt(scope) 矢印と線本体の接合は重ね描き。upgrade: 端部形状に応じた線の短縮
        warnings.push(
            Code::Approximation,
            "矢印端は近似です。線本体との接合部は重ね描きします",
        );
    }
    for (tag, side) in [("headEnd", "start"), ("tailEnd", "end")] {
        let Some(end) = child(line, A, tag) else {
            continue;
        };
        let kind = end.attribute("type").unwrap_or("none");
        if kind == "none" || width <= 0.0 {
            continue;
        }
        let dimension = |key| match end.attribute(key).unwrap_or("med") {
            "sm" => 2.0,
            "lg" => 5.0,
            _ => 3.0,
        };
        let (length, height) = (dimension("len"), dimension("w"));
        let middle = height / 2.0;
        let drawing = match kind {
            "triangle" => format!("<path d=\"M0 0 L{length} {middle} L0 {height} Z\"/>"),
            "stealth" => format!(
                "<path d=\"M0 0 L{length} {middle} L0 {height} L{} {middle} Z\"/>",
                length / 3.0
            ),
            "arrow" => format!(
                "<path d=\"M0 0 L{length} {middle} L0 {height}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1\"/>"
            ),
            "diamond" => format!(
                "<path d=\"M0 {middle} L{} 0 L{length} {middle} L{} {height} Z\"/>",
                length / 2.0,
                length / 2.0
            ),
            "oval" => format!(
                "<ellipse cx=\"{}\" cy=\"{middle}\" rx=\"{}\" ry=\"{middle}\"/>",
                length / 2.0,
                length / 2.0
            ),
            _ => {
                warn(warnings, format!("未対応の矢印端: {kind}"));
                continue;
            }
        };
        let id = format!("line-end-{}-{side}", svg.len());
        let reference = if matches!(kind, "diamond" | "oval") {
            length / 2.0
        } else {
            length
        };
        svg.push_str(&format!("<defs><marker id=\"{id}\" markerUnits=\"strokeWidth\" markerWidth=\"{}\" markerHeight=\"{}\" viewBox=\"-1 -1 {} {}\" refX=\"{reference}\" refY=\"{middle}\" orient=\"auto-start-reverse\" overflow=\"visible\"><g fill=\"{stroke}\">{drawing}</g></marker></defs>",length+2.0,height+2.0,length+2.0,height+2.0));
        attrs.push_str(&format!(" marker-{side}=\"url(#{id})\""));
    }
    if line.attribute("cmpd").is_some_and(|v| v != "sng") {
        warn(warnings, "複合線は未対応です");
    }
    if line.attribute("algn").is_some_and(|v| v != "ctr") {
        warn(warnings, "内側配置の線は中央配置で近似します");
    }
    attrs
}
