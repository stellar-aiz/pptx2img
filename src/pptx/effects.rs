//! Build outer shadows from SourceAlpha and composite them behind the original shape.
use super::*;
pub(super) fn begin(
    list: Node<'_, '_>,
    geometry: (f64, f64, f64, f64, f64),
    theme: &Theme,
    svg: &mut String,
    warnings: &mut Warnings,
) -> usize {
    let (w, h, rotation, fx, fy) = geometry;
    let mut count = 0;
    for effect in list.children().filter(|n| n.is_element()) {
        if !effect.has_tag_name((A, "outerShdw")) {
            warn(
                warnings,
                format!("Unsupported shape effect: {}", effect.tag_name().name()),
            );
            continue;
        }
        let blur = number(effect, "blurRad", 0.0).max(0.0) / EMU;
        let distance = number(effect, "dist", 0.0).max(0.0) / EMU;
        let angle = (number(effect, "dir", 0.0) / 60000.0).to_radians();
        let (mut dx, mut dy) = (distance * angle.cos(), distance * angle.sin());
        // resvg applies filter offsets in rendering coordinates.
        // Apply shape flips and rotation to the direction vector only when following the shape.
        if theme::bool_attr(effect, "rotWithShape") != Some(false) {
            let r = rotation.to_radians();
            (dx, dy) = (
                fx * dx * r.cos() - fy * dy * r.sin(),
                fx * dx * r.sin() + fy * dy * r.cos(),
            );
        }
        if number(effect, "sx", 100000.0) != 100000.0
            || number(effect, "sy", 100000.0) != 100000.0
            || number(effect, "kx", 0.0) != 0.0
            || number(effect, "ky", 0.0) != 0.0
        {
            warn(warnings, "Outer shadow scaling and skew are unsupported");
        }
        let (color, alpha) = colors::shadow_color(effect, theme, warnings);
        let id = format!("outer-shadow-{}", svg.len());
        let margin = distance + blur * 3.0 + 2.0;
        // stellar:debt(scope) Approximate blur with Gaussian sigma=radius/2. upgrade: compare Office blur kernels
        if blur > 0.0 {
            warnings.push(
                Code::Approximation,
                "Outer shadow blur is approximated with a Gaussian",
            );
        }
        svg.push_str(&format!("<defs><filter id=\"{id}\" filterUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" color-interpolation-filters=\"sRGB\"><feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"{}\" result=\"blur\"/><feOffset in=\"blur\" dx=\"{dx}\" dy=\"{dy}\" result=\"offset\"/><feFlood flood-color=\"{color}\" flood-opacity=\"{alpha}\"/><feComposite in2=\"offset\" operator=\"in\" result=\"shadow\"/><feMerge><feMergeNode in=\"shadow\"/><feMergeNode in=\"SourceGraphic\"/></feMerge></filter></defs><g filter=\"url(#{id})\">",-margin,-margin,w+2.0*margin,h+2.0*margin,blur/2.0));
        count += 1;
    }
    count
}
