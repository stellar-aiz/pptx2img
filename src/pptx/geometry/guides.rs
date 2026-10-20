//! Preset outlines evaluated from DrawingML shape guides (ECMA-376 Part 1 §20.1.9).
//!
//! Each definition below transcribes the guide list, text rectangle, and paths of one
//! preset from the DrawingMLGeometries part of ECMA-376 Part 1. The evaluator is an
//! independent implementation of the guide formulas (§20.1.10.27) and path commands.
use super::*;
use std::collections::HashMap;
use std::f64::consts::PI;

/// Angles in guides are 60000ths of a degree.
const DEGREE: f64 = 60000.0;

pub(super) fn find(kind: &str) -> Option<&'static str> {
    DEFINITIONS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, definition)| *definition)
}

pub(super) fn render(
    node: Node<'_, '_>,
    definition: &str,
    w: f64,
    h: f64,
    style: &str,
    warnings: &mut Warnings,
) -> Geometry {
    let mut guides = Guides::new(w, h);
    let defaults: Vec<(&str, f64)> = definition
        .lines()
        .filter_map(|line| line.strip_prefix("av "))
        .filter_map(|line| {
            let (name, value) = line.split_once(' ')?;
            Some((name, value.parse().ok()?))
        })
        .collect();
    for (name, value) in defaults.iter().zip(adjustments(node, &defaults, warnings)) {
        guides.values.insert(name.0.into(), value);
    }
    let mut text = None;
    let mut paths = Vec::new();
    let mut approximations: Vec<String> = Vec::new();
    for line in definition.lines() {
        let mut tokens = line.split_whitespace();
        let Some(head) = tokens.next() else { continue };
        let args: Vec<&str> = tokens.collect();
        match head {
            "av" => {}
            "approx" => {
                let note = line
                    .trim_start()
                    .strip_prefix("approx")
                    .unwrap_or("")
                    .trim();
                if !approximations.iter().any(|n| n == note) {
                    approximations.push(note.to_string());
                }
            }
            "gd" => {
                let value = guides.formula(&args[1..]);
                guides.values.insert(args[0].into(), value);
            }
            "rect" => {
                let [l, t, r, b] = [0, 1, 2, 3].map(|i| guides.get(args[i]));
                text = Some((l, t, r - l, b - t));
            }
            "path" => paths.push(Path::new(&args, w, h)),
            command => {
                let Some(path) = paths.last_mut() else {
                    continue;
                };
                let values: Vec<f64> = args.iter().map(|a| guides.get(a)).collect();
                path.command(command, &values);
            }
        }
    }
    for note in approximations {
        warnings.push(Code::Approximation, format!("Preset shape: {note}"));
    }
    // Only paths without fill keep line-end markers.
    let svg = paint(&paths, style, |path| !path.fill);
    finish(svg, &paths, text, (w, h), "Preset shape", warnings)
}

/// The geometry, or an empty outline with the whole-shape text area (and a warning)
/// when a guide made a path or the text rectangle non-finite.
fn finish(
    svg: String,
    paths: &[Path],
    text: Option<(f64, f64, f64, f64)>,
    (w, h): (f64, f64),
    kind: &str,
    warnings: &mut Warnings,
) -> Geometry {
    let text = text.unwrap_or((0.0, 0.0, w, h));
    let text_finite = [text.0, text.1, text.2, text.3]
        .iter()
        .all(|v| v.is_finite());
    if !text_finite || paths.iter().any(|p| !p.finite) {
        warn(
            warnings,
            format!("{kind} guides produced non-finite values; outline omitted"),
        );
        return Geometry {
            svg: String::new(),
            text: (0.0, 0.0, w, h),
        };
    }
    Geometry { svg, text }
}

/// Paint every path inside one group so per-path fill/stroke overrides inherit the
/// shape style. `markers` decides which paths keep line-end markers.
fn paint(paths: &[Path], style: &str, markers: impl Fn(&Path) -> bool) -> String {
    let mut svg = format!("<g {style}>");
    for path in paths {
        let mut attrs = String::new();
        if !path.fill {
            attrs.push_str(" fill=\"none\"");
        }
        if !markers(path) {
            attrs.push_str(" marker-start=\"none\" marker-end=\"none\"");
        }
        if !path.stroke {
            attrs.push_str(" stroke=\"none\"");
        }
        svg.push_str(&format!("<path d=\"{}\"{attrs}/>", path.data.trim()));
    }
    svg.push_str("</g>");
    svg
}

/// Custom geometry (`a:custGeom`, §20.1.9.8): guides from `a:avLst` and `a:gdLst`, the
/// text rectangle from `a:rect`, and `a:pathLst` drawn with the preset path evaluator.
/// Guides and `a:rect` are evaluated in EMU, the shape coordinate space; each path is
/// scaled from its own `w`/`h` space (EMU when omitted) to the `w`×`h` pixel box.
pub(super) fn custom(
    node: Node<'_, '_>,
    w: f64,
    h: f64,
    style: &str,
    warnings: &mut Warnings,
) -> Geometry {
    let mut guides = Guides::new(w * EMU, h * EMU);
    let lists = ["avLst", "gdLst"].map(|name| child(node, A, name));
    for guide in lists
        .iter()
        .flatten()
        .flat_map(|list| list.children())
        .filter(|n| n.has_tag_name((A, "gd")))
    {
        let tokens: Vec<&str> = guide
            .attribute("fmla")
            .unwrap_or("")
            .split_whitespace()
            .collect();
        let value = guides.formula(&tokens);
        guides
            .values
            .insert(guide.attribute("name").unwrap_or("").into(), value);
    }
    let text = child(node, A, "rect").map(|rect| {
        let [l, t, r, b] =
            ["l", "t", "r", "b"].map(|k| guides.get(rect.attribute(k).unwrap_or("")) / EMU);
        (l, t, r - l, b - t)
    });
    let mut paths = Vec::new();
    for source in child(node, A, "pathLst")
        .into_iter()
        .flat_map(|list| list.children())
        .filter(|n| n.has_tag_name((A, "path")))
    {
        let size = |key: &str, extent: f64| {
            let value = number(source, key, 0.0);
            if value > 0.0 {
                extent / value
            } else {
                1.0 / EMU
            }
        };
        let fill = source.attribute("fill").unwrap_or("norm");
        if fill != "norm" && fill != "none" {
            warnings.push(
                Code::Approximation,
                format!("Custom geometry path fill mode {fill} is drawn as a normal fill"),
            );
        }
        let mut path = Path::scaled(
            fill != "none",
            !matches!(source.attribute("stroke"), Some("0" | "false")),
            (size("w", w), size("h", h)),
        );
        for segment in source.children().filter(|n| n.is_element()) {
            let command = match segment.tag_name().name() {
                "moveTo" => "M",
                "lnTo" => "L",
                "cubicBezTo" => "C",
                "quadBezTo" => "Q",
                "arcTo" => "A",
                "close" => "Z",
                other => {
                    warn(
                        warnings,
                        format!("Unsupported custom geometry path command: {other}"),
                    );
                    continue;
                }
            };
            // SVG path data must start with a move; DrawingML starts at the origin.
            if path.data.is_empty() && command != "M" {
                path.command("M", &[0.0, 0.0]);
            }
            let values: Vec<f64> = if command == "A" {
                ["wR", "hR", "stAng", "swAng"]
                    .iter()
                    .map(|k| guides.get(segment.attribute(*k).unwrap_or("")))
                    .collect()
            } else {
                segment
                    .children()
                    .filter(|n| n.has_tag_name((A, "pt")))
                    .flat_map(|pt| ["x", "y"].map(|k| guides.get(pt.attribute(k).unwrap_or(""))))
                    .collect()
            };
            path.command(command, &values);
        }
        paths.push(path);
    }
    // Line-end markers belong on open paths only.
    let svg = paint(&paths, style, |path| !path.data.contains('Z'));
    finish(svg, &paths, text, (w, h), "Custom geometry", warnings)
}

struct Guides {
    values: HashMap<String, f64>,
    w: f64,
    h: f64,
}
impl Guides {
    fn new(w: f64, h: f64) -> Self {
        Self {
            values: HashMap::new(),
            w,
            h,
        }
    }
    /// Resolve a guide name, a built-in shape guide (§20.1.9.11), or a literal.
    fn get(&self, name: &str) -> f64 {
        if let Some(value) = self.values.get(name) {
            return *value;
        }
        if let Ok(value) = name.parse::<f64>() {
            return value;
        }
        let (w, h) = (self.w, self.h);
        let ss = w.min(h);
        let ratio = |prefix: &str, base: f64| {
            name.strip_prefix(prefix)
                .and_then(|n| n.parse::<f64>().ok())
                .map(|n| base / n)
        };
        let builtin = match name {
            "l" | "t" => Some(0.0),
            "r" | "w" => Some(w),
            "b" | "h" => Some(h),
            "hc" => Some(w / 2.0),
            "vc" => Some(h / 2.0),
            "ss" => Some(ss),
            "ls" => Some(w.max(h)),
            _ => None,
        };
        builtin
            .or_else(|| ratio("wd", w))
            .or_else(|| ratio("hd", h))
            .or_else(|| ratio("ssd", ss))
            .or_else(|| {
                // cd2, cd4, cd8, 3cd4, 3cd8, 5cd8, 7cd8: fractions of a full circle.
                let (count, divisor) = name.split_once("cd")?;
                let count = if count.is_empty() {
                    1.0
                } else {
                    count.parse::<f64>().ok()?
                };
                Some(count * 21_600_000.0 / divisor.parse::<f64>().ok()?)
            })
            .unwrap_or(f64::NAN)
    }
    /// Evaluate one guide formula (§20.1.10.27 ST_GeomGuideFormula).
    fn formula(&self, tokens: &[&str]) -> f64 {
        let arg = |i: usize| tokens.get(i + 1).map_or(f64::NAN, |t| self.get(t));
        let (x, y, z) = (arg(0), arg(1), arg(2));
        let radians = |angle: f64| angle / DEGREE * PI / 180.0;
        match tokens.first().copied().unwrap_or("") {
            "val" => x,
            "*/" => x * y / z,
            "+-" => x + y - z,
            "+/" => (x + y) / z,
            "?:" => {
                if x > 0.0 {
                    y
                } else {
                    z
                }
            }
            "abs" => x.abs(),
            "at2" => y.atan2(x) * 180.0 / PI * DEGREE,
            "cat2" => x * z.atan2(y).cos(),
            "sat2" => x * z.atan2(y).sin(),
            "cos" => x * radians(y).cos(),
            "sin" => x * radians(y).sin(),
            "tan" => x * radians(y).tan(),
            "max" => x.max(y),
            "min" => x.min(y),
            "mod" => (x * x + y * y + z * z).sqrt(),
            "pin" => {
                if y < x {
                    x
                } else if y > z {
                    z
                } else {
                    y
                }
            }
            "sqrt" => x.sqrt(),
            _ => f64::NAN,
        }
    }
}

/// Round away floating-point noise (well below a device pixel) for compact SVG.
fn tidy(value: f64) -> f64 {
    let rounded = (value * 1e6).round() / 1e6;
    if rounded == 0.0 { 0.0 } else { rounded }
}

struct Path {
    data: String,
    fill: bool,
    stroke: bool,
    /// Scale from the path coordinate space to the shape box.
    scale: (f64, f64),
    current: (f64, f64),
    finite: bool,
}
impl Path {
    fn new(attrs: &[&str], w: f64, h: f64) -> Self {
        let size = |key: &str, extent: f64| {
            attrs
                .iter()
                .find_map(|a| a.strip_prefix(key))
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| *v > 0.0)
                .map_or(1.0, |v| extent / v)
        };
        Self::scaled(
            !attrs.contains(&"nofill"),
            !attrs.contains(&"nostroke"),
            (size("w=", w), size("h=", h)),
        )
    }
    fn scaled(fill: bool, stroke: bool, scale: (f64, f64)) -> Self {
        Self {
            data: String::new(),
            fill,
            stroke,
            scale,
            current: (0.0, 0.0),
            finite: true,
        }
    }
    fn point(&mut self, x: f64, y: f64) -> (f64, f64) {
        let point = (x * self.scale.0, y * self.scale.1);
        self.finite &= point.0.is_finite() && point.1.is_finite();
        point
    }
    fn command(&mut self, command: &str, values: &[f64]) {
        let mut points = Vec::new();
        if command != "A" {
            for pair in values.chunks(2) {
                if let [x, y] = pair {
                    points.push(self.point(*x, *y));
                }
            }
        }
        let list = |points: &[(f64, f64)]| {
            points
                .iter()
                .map(|(x, y)| format!("{} {}", tidy(*x), tidy(*y)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        match command {
            "M" | "L" | "C" | "Q" => {
                self.data.push_str(&format!(" {command}{}", list(&points)));
                if let Some(last) = points.last() {
                    self.current = *last;
                }
            }
            "Z" => self.data.push_str(" Z"),
            "A" => self.arc(values),
            _ => self.finite = false,
        }
    }
    /// arcTo (§20.1.9.3): angles are visual angles on the ellipse, clockwise in y-down
    /// coordinates; the arc starts at the current point.
    fn arc(&mut self, values: &[f64]) {
        let [wr, hr, start, sweep] =
            [0, 1, 2, 3].map(|i| values.get(i).copied().unwrap_or(f64::NAN));
        let (wr, hr) = (wr * self.scale.0, hr * self.scale.1);
        if ![wr, hr, start, sweep].iter().all(|v| v.is_finite()) {
            self.finite = false;
            return;
        }
        // Convert a visual angle to the ellipse's parametric angle.
        let parametric = |angle: f64| {
            let radians = angle / DEGREE * PI / 180.0;
            (wr * radians.sin()).atan2(hr * radians.cos())
        };
        let t1 = parametric(start);
        let center = (
            self.current.0 - wr * t1.cos(),
            self.current.1 - hr * t1.sin(),
        );
        let turns = sweep / DEGREE / 360.0;
        let mut delta = parametric(start + sweep) - t1;
        if turns.abs() >= 1.0 {
            delta = 2.0 * PI * turns.signum();
        } else if sweep > 0.0 {
            delta = delta.rem_euclid(2.0 * PI);
        } else if sweep < 0.0 {
            delta = -(-delta).rem_euclid(2.0 * PI);
        } else {
            return;
        }
        // Emit segments of at most a quarter turn: the large-arc flag stays 0 and no
        // segment has diametrically opposite (ambiguous) end points.
        let segments = (delta.abs() / (PI / 2.0) - 1e-9).ceil().max(1.0) as usize;
        let flag = u8::from(delta > 0.0);
        for i in 1..=segments {
            let t = t1 + delta * i as f64 / segments as f64;
            let end = (center.0 + wr * t.cos(), center.1 + hr * t.sin());
            self.data.push_str(&format!(
                " A{} {} 0 0 {flag} {} {}",
                tidy(wr),
                tidy(hr),
                tidy(end.0),
                tidy(end.1)
            ));
            self.current = end;
        }
    }
}

/// Selected presets from ECMA-376 Part 1 DrawingMLGeometries, one guide per line:
/// `av name default`, `gd name formula`, `approx note` (an approximation warning),
/// `rect l t r b`, `path [w=] [h=] [nofill]
/// [nostroke]`, then `M`/`L`/`C`/`Q` points, `A wR hR stAng swAng`, and `Z`.
const DEFINITIONS: &[(&str, &str)] = &[
    (
        "chevron",
        "av adj 50000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd x3 */ x2 1 2
gd dx +- x2 0 x1
gd il ?: dx x1 l
gd ir ?: dx x2 r
rect il t ir b
path
M l t
L x2 t
L r vc
L x2 b
L l b
L x1 vc
Z",
    ),
    (
        "triangle",
        "av adj 50000
gd a pin 0 adj 100000
gd x1 */ w a 200000
gd x2 */ w a 100000
gd x3 +- x1 wd2 0
rect x1 vc x3 b
path
M l b
L x2 t
L r b
Z",
    ),
    (
        "downArrow",
        "av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dy1 */ ss a2 100000
gd y1 +- b 0 dy1
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
gd dy2 */ x1 dy1 wd2
gd y2 +- y1 dy2 0
rect x1 t x2 y2
path
M l y1
L x1 y1
L x1 t
L x2 t
L x2 y1
L r y1
L hc b
Z",
    ),
    (
        "leftArrow",
        "av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx2 */ ss a2 100000
gd x2 +- l dx2 0
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd dx1 */ y1 dx2 hd2
gd x1 +- x2  0 dx1
rect x1 y1 r y2
path
M l vc
L x2 t
L x2 y1
L r y1
L r y2
L x2 y2
L x2 b
Z",
    ),
    (
        "leftRightArrow",
        "av adj1 50000
av adj2 50000
gd maxAdj2 */ 50000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd x2 */ ss a2 100000
gd x3 +- r 0 x2
gd dy */ h a1 200000
gd y1 +- vc 0 dy
gd y2 +- vc dy 0
gd dx1 */ y1 x2 hd2
gd x1 +- x2 0 dx1
gd x4 +- x3 dx1 0
rect x1 y1 x4 y2
path
M l vc
L x2 t
L x2 y1
L x3 y1
L x3 t
L r vc
L x3 b
L x3 y2
L x2 y2
L x2 b
Z",
    ),
    (
        "upDownArrow",
        "av adj1 50000
av adj2 50000
gd maxAdj2 */ 50000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd y2 */ ss a2 100000
gd y3 +- b 0 y2
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
gd dy1 */ x1 y2 wd2
gd y1 +- y2 0 dy1
gd y4 +- y3 dy1 0
rect x1 y1 x2 y4
path
M l y2
L hc t
L r y2
L x2 y2
L x2 y3
L r y3
L hc b
L l y3
L x1 y3
L x1 y2
Z",
    ),
    (
        "diamond",
        "gd ir */ w 3 4
gd ib */ h 3 4
rect wd4 hd4 ir ib
path
M l vc
L hc t
L r vc
L hc b
Z",
    ),
    (
        "trapezoid",
        "av adj 25000
gd maxAdj */ 50000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 200000
gd x2 */ ss a 100000
gd x3 +- r 0 x2
gd x4 +- r 0 x1
gd il */ wd3 a maxAdj
gd it */ hd3 a maxAdj
gd ir +- r 0 il
rect il it ir b
path
M l b
L x2 t
L x3 t
L r b
Z",
    ),
    // The published pie text rectangle lists `l=il t=ir r=it b=ib`; this uses the
    // inscribed square `il it ir ib` that the guide names describe.
    (
        "pie",
        "av adj1 0
av adj2 16200000
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd wt2 sin wd2 enAng
gd ht2 cos hd2 enAng
gd dx2 cat2 wd2 ht2 wt2
gd dy2 sat2 hd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd il +- hc 0 idx
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
rect il it ir ib
path
M x1 y1
A wd2 hd2 stAng swAng
L hc vc
Z",
    ),
    (
        "star5",
        "av adj 19098
av hf 105146
av vf 110557
gd a pin 0 adj 50000
gd swd2 */ wd2 hf 100000
gd shd2 */ hd2 vf 100000
gd svc */ vc  vf 100000
gd dx1 cos swd2 1080000
gd dx2 cos swd2 18360000
gd dy1 sin shd2 1080000
gd dy2 sin shd2 18360000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- svc 0 dy1
gd y2 +- svc 0 dy2
gd iwd2 */ swd2 a 50000
gd ihd2 */ shd2 a 50000
gd sdx1 cos iwd2 20520000
gd sdx2 cos iwd2 3240000
gd sdy1 sin ihd2 3240000
gd sdy2 sin ihd2 20520000
gd sx1 +- hc 0 sdx1
gd sx2 +- hc 0 sdx2
gd sx3 +- hc sdx2 0
gd sx4 +- hc sdx1 0
gd sy1 +- svc 0 sdy1
gd sy2 +- svc 0 sdy2
gd sy3 +- svc ihd2 0
gd yAdj +- svc 0 ihd2
rect sx1 sy1 sx4 sy3
path
M x1 y1
L sx2 sy1
L hc t
L sx3 sy1
L x4 y1
L sx4 sy2
L x3 y2
L hc sy3
L x2 y2
L sx1 sy2
Z",
    ),
    (
        "cloud",
        "gd il */ w 2977 21600
gd it */ h 3262 21600
gd ir */ w 17087 21600
gd ib */ h 17337 21600
gd g27 */ w 67 21600
gd g28 */ h 21577 21600
gd g29 */ w 21582 21600
gd g30 */ h 1235 21600
rect il it ir ib
path w=43200 h=43200
M 3900 14370
A 6753 9190 -11429249 7426832
A 5333 7267 -8646143 5396714
A 4365 5945 -8748475 5983381
A 4857 6595 -7859164 7034504
A 5333 7273 -4722533 6541615
A 6775 9220 -2776035 7816140
A 5785 7867 37501 6842000
A 6752 9215 1347096 6910353
A 7720 10543 3974558 4542661
A 4360 5918 -16496525 8804134
A 4345 5945 -14809710 9151131
Z
path w=43200 h=43200 nofill
M 4693 26177
A 4345 5945 5204520 1585770
M 6928 34899
A 4360 5918 4416628 686848
M 16478 39090
A 6752 9215 8257449 844866
M 28827 34751
A 6752 9215 387196 959901
M 34129 22954
A 5785 7867 -4217541 4255042
M 41798 15354
A 5333 7273 1819082 1665090
M 38324 5426
A 4857 6595 -824660 891534
M 29078 3952
A 4857 6595 -8950887 1091722
M 22141 4720
A 4365 5945 -9809656 1061181
M 14000 5192
A 6753 9190 -4002417 739161
M 4127 15789
A 6753 9190 9459261 711490",
    ),
    (
        "round2SameRect",
        "av adj1 16667
av adj2 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd tx1 */ ss a1 100000
gd tx2 +- r 0 tx1
gd bx1 */ ss a2 100000
gd bx2 +- r 0 bx1
gd by1 +- b 0 bx1
gd d +- tx1 0 bx1
gd tdx */ tx1 29289 100000
gd bdx */ bx1 29289 100000
gd il ?: d tdx bdx
gd ir +- r 0 il
gd ib +- b 0 bdx
rect il tdx ir ib
path
M tx1 t
L tx2 t
A tx1 tx1 3cd4 cd4
L r by1
A bx1 bx1 0 cd4
L bx1 b
A bx1 bx1 cd4 cd4
L l tx1
A tx1 tx1 cd2 cd4
Z",
    ),
    (
        "wedgeEllipseCallout",
        "av adj1 -20833
av adj2 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd sdx */ dxPos h 1
gd sdy */ dyPos w 1
gd pang at2 sdx sdy
gd stAng +- pang 660000 0
gd enAng +- pang 0 660000
gd dx1 cos wd2 stAng
gd dy1 sin hd2 stAng
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd dx2 cos wd2 enAng
gd dy2 sin hd2 enAng
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd stAng1 at2 dx1 dy1
gd enAng1 at2 dx2 dy2
gd swAng1 +- enAng1 0 stAng1
gd swAng2 +- swAng1 21600000 0
gd swAng ?: swAng1 swAng1 swAng2
gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd il +- hc 0 idx
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
rect il it ir ib
path
M xPos yPos
L x1 y1
A wd2 hd2 stAng1 swAng
Z",
    ),
    (
        "flowChartAlternateProcess",
        "gd x2 +- r 0 ssd6
gd y2 +- b 0 ssd6
gd il */ ssd6 29289 100000
gd ir +- r 0 il
gd ib +- b 0 il
rect il il ir ib
path
M l ssd6
A ssd6 ssd6 cd2 cd4
L x2 t
A ssd6 ssd6 3cd4 cd4
L r y2
A ssd6 ssd6 0 cd4
L ssd6 b
A ssd6 ssd6 cd4 cd4
Z",
    ),
    (
        "flowChartMultidocument",
        "gd y2 */ h 3675 21600
gd y8 */ h 20782 21600
gd x3 */ w 9298 21600
gd x4 */ w 12286 21600
gd x5 */ w 18595 21600
rect l y2 x5 y8
path w=21600 h=21600 nostroke
M 0 20782
C 9298 23542 9298 18022 18595 18022
L 18595 3675
L 0 3675
Z
M 1532 3675
L 1532 1815
L 20000 1815
L 20000 16252
C 19298 16252 18595 16352 18595 16352
L 18595 3675
Z
M 2972 1815
L 2972 0
L 21600 0
L 21600 14392
C 20800 14392 20000 14467 20000 14467
L 20000 1815
Z
path w=21600 h=21600 nofill
M 0 3675
L 18595 3675
L 18595 18022
C 9298 18022 9298 23542 0 20782
Z
M 1532 3675
L 1532 1815
L 20000 1815
L 20000 16252
C 19298 16252 18595 16352 18595 16352
M 2972 1815
L 2972 0
L 21600 0
L 21600 14392
C 20800 14392 20000 14467 20000 14467
path w=21600 h=21600 nofill nostroke
M 0 20782
C 9298 23542 9298 18022 18595 18022
L 18595 16352
C 18595 16352 19298 16252 20000 16252
L 20000 14467
C 20000 14467 20800 14392 21600 14392
L 21600 0
L 2972 0
L 2972 1815
L 1532 1815
L 1532 3675
L 0 3675
Z",
    ),
    // Next tier: transcribed from the same DrawingMLGeometries part. A `shade` line marks
    // presets whose published definition also has lighten/darken path fills, which are
    // not drawn (one approximation warning per shape).
    (
        "rtTriangle",
        "gd it */ h 7 12
gd ir */ w 7 12
gd ib */ h 11 12
gd il */ w 1 12
rect il it ir ib
path
M l b
L l t
L r b
Z",
    ),
    (
        "foldedCorner",
        "av adj 16667
gd a pin 0 adj 50000
gd dy2 */ ss a 100000
gd dy1 */ dy2 1 5
gd x1 +- r 0 dy2
gd x2 +- x1 dy1 0
gd y2 +- b 0 dy2
gd y1 +- y2 dy1 0
rect l t r y2
approx lighten and darken path shading is not drawn
path nostroke
M l t
L r t
L r y2
L x1 b
L l b
Z
path nofill
M r y2
L x2 y1
L x1 b
L l b
L l t
L r t
Z",
    ),
    (
        "borderCallout1",
        "av adj1 18750
av adj2 -8333
av adj3 112500
av adj4 -38333
gd y1 */ h adj1 100000
gd x1 */ w adj2 100000
gd y2 */ h adj3 100000
gd x2 */ w adj4 100000
rect l t r b
path
M l t
L r t
L r b
L l b
Z
path nofill
M x1 y1
L x2 y2",
    ),
    (
        "borderCallout2",
        "av adj1 18750
av adj2 -8333
av adj3 18750
av adj4 -16667
av adj5 112500
av adj6 -46667
gd y1 */ h adj1 100000
gd x1 */ w adj2 100000
gd y2 */ h adj3 100000
gd x2 */ w adj4 100000
gd y3 */ h adj5 100000
gd x3 */ w adj6 100000
rect l t r b
path
M l t
L r t
L r b
L l b
Z
path nofill
M x1 y1
L x2 y2
L x3 y3",
    ),
    (
        "flowChartDocument",
        "gd y1 */ h 17322 21600
rect l t r y1
path w=21600 h=21600
M 0 0
L 21600 0
L 21600 17322
C 10800 17322 10800 23922 0 20172
Z",
    ),
    (
        "flowChartMagneticDisk",
        "gd y1 */ h 1 6
gd y2 */ y1 2 1
gd y3 */ h 5 6
rect l y2 r y3
path nostroke
M l y1
A wd2 y1 cd2 cd2
L r y3
A wd2 y1 0 cd2
Z
path nofill
M l y1
A wd2 y1 cd2 cd2
L r y3
A wd2 y1 0 cd2
Z
M l y1
A wd2 y1 cd2 -10800000",
    ),
    (
        "flowChartTerminator",
        "gd il */ w 1018 21600
gd ir */ w 20582 21600
gd it */ h 3163 21600
gd ib */ h 18437 21600
rect il it ir ib
path w=21600 h=21600
M 3475 0
L 18125 0
A 3475 10800 3cd4 cd2
L 3475 21600
A 3475 10800 cd4 cd2
Z",
    ),
    (
        "flowChartExtract",
        "gd x2 */ w 3 4
rect wd4 vc x2 b
path w=2 h=2
M 1 0
L 2 2
L 0 2
Z",
    ),
    (
        "flowChartConnector",
        "gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd il +- hc 0 idx
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
rect il it ir ib
path
M l vc
A wd2 hd2 cd2 cd4
A wd2 hd2 3cd4 cd4
A wd2 hd2 0 cd4
A wd2 hd2 cd4 cd4
Z",
    ),
    (
        "wedgeRectCallout",
        "av adj1 -20833
av adj2 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd dq */ dxPos h w
gd ady abs dyPos
gd adq abs dq
gd dz +- ady 0 adq
gd xg1 ?: dxPos 7 2
gd xg2 ?: dxPos 10 5
gd x1 */ w xg1 12
gd x2 */ w xg2 12
gd yg1 ?: dyPos 7 2
gd yg2 ?: dyPos 10 5
gd y1 */ h yg1 12
gd y2 */ h yg2 12
gd t1 ?: dxPos l xPos
gd xl ?: dz l t1
gd t2 ?: dyPos x1 xPos
gd xt ?: dz t2 x1
gd t3 ?: dxPos xPos r
gd xr ?: dz r t3
gd t4 ?: dyPos xPos x1
gd xb ?: dz t4 x1
gd t5 ?: dxPos y1 yPos
gd yl ?: dz y1 t5
gd t6 ?: dyPos t yPos
gd yt ?: dz t6 t
gd t7 ?: dxPos yPos y1
gd yr ?: dz y1 t7
gd t8 ?: dyPos yPos b
gd yb ?: dz t8 b
rect l t r b
path
M l t
L x1 t
L xt yt
L x2 t
L r t
L r y1
L xr yr
L r y2
L r b
L x2 b
L xb yb
L x1 b
L l b
L l y2
L xl yl
L l y1
Z",
    ),
    (
        "donut",
        "av adj 25000
gd a pin 0 adj 50000
gd dr */ ss a 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd il +- hc 0 idx
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
rect il it ir ib
path
M l vc
A wd2 hd2 cd2 cd4
A wd2 hd2 3cd4 cd4
A wd2 hd2 0 cd4
A wd2 hd2 cd4 cd4
Z
M dr vc
A iwd2 ihd2 cd2 -5400000
A iwd2 ihd2 cd4 -5400000
A iwd2 ihd2 0 -5400000
A iwd2 ihd2 3cd4 -5400000
Z",
    ),
    (
        "arc",
        "av adj1 16200000
av adj2 0
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd wt2 sin wd2 enAng
gd ht2 cos hd2 enAng
gd dx2 cat2 wd2 ht2 wt2
gd dy2 sat2 hd2 ht2 wt2
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd sw0 +- 21600000 0 stAng
gd da1 +- swAng 0 sw0
gd g1 max x1 x2
gd ir ?: da1 r g1
gd sw3 +- cd4 0 stAng
gd sw4 +- 27000000 0 stAng
gd sw5 ?: sw3 sw3 sw4
gd da2 +- swAng 0 sw5
gd g5 max y1 y2
gd ib ?: da2 b g5
gd sw6 +- cd2 0 stAng
gd sw7 +- 32400000 0 stAng
gd sw8 ?: sw6 sw6 sw7
gd da3 +- swAng 0 sw8
gd g9 min x1 x2
gd il ?: da3 l g9
gd sw9 +- 3cd4 0 stAng
gd sw10 +- 37800000 0 stAng
gd sw11 ?: sw9 sw9 sw10
gd da4 +- swAng 0 sw11
gd g13 min y1 y2
gd it ?: da4 t g13
rect il it ir ib
path nostroke
M x1 y1
A wd2 hd2 stAng swAng
L hc vc
Z
path nofill
M x1 y1
A wd2 hd2 stAng swAng",
    ),
    (
        "blockArc",
        "av adj1 10800000
av adj2 0
av adj3 25000
gd stAng pin 0 adj1 21599999
gd istAng pin 0 adj2 21599999
gd a3 pin 0 adj3 50000
gd sw1 +- istAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd iswAng +- 0 0 swAng
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd wt3 sin wd2 istAng
gd ht3 cos hd2 istAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd dx3 cat2 wd2 ht3 wt3
gd dy3 sat2 hd2 ht3 wt3
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd x3 +- hc dx3 0
gd y3 +- vc dy3 0
gd dr */ ss a3 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
gd wt2 sin iwd2 istAng
gd ht2 cos ihd2 istAng
gd wt4 sin iwd2 stAng
gd ht4 cos ihd2 stAng
gd dx2 cat2 iwd2 ht2 wt2
gd dy2 sat2 ihd2 ht2 wt2
gd dx4 cat2 iwd2 ht4 wt4
gd dy4 sat2 ihd2 ht4 wt4
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd x4 +- hc dx4 0
gd y4 +- vc dy4 0
gd sw0 +- 21600000 0 stAng
gd da1 +- swAng 0 sw0
gd g1 max x1 x2
gd g2 max x3 x4
gd g3 max g1 g2
gd ir ?: da1 r g3
gd sw3 +- cd4 0 stAng
gd sw4 +- 27000000 0 stAng
gd sw5 ?: sw3 sw3 sw4
gd da2 +- swAng 0 sw5
gd g5 max y1 y2
gd g6 max y3 y4
gd g7 max g5 g6
gd ib ?: da2 b g7
gd sw6 +- cd2 0 stAng
gd sw7 +- 32400000 0 stAng
gd sw8 ?: sw6 sw6 sw7
gd da3 +- swAng 0 sw8
gd g9 min x1 x2
gd g10 min x3 x4
gd g11 min g9 g10
gd il ?: da3 l g11
gd sw9 +- 3cd4 0 stAng
gd sw10 +- 37800000 0 stAng
gd sw11 ?: sw9 sw9 sw10
gd da4 +- swAng 0 sw11
gd g13 min y1 y2
gd g14 min y3 y4
gd g15 min g13 g14
gd it ?: da4 t g15
rect il it ir ib
path
M x1 y1
A wd2 hd2 stAng swAng
L x2 y2
A iwd2 ihd2 istAng iswAng
Z",
    ),
    (
        "can",
        "av adj 25000
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 200000
gd y2 +- y1 y1 0
gd y3 +- b 0 y1
rect l y2 r y3
approx lighten and darken path shading is not drawn
path nostroke
M l y1
A wd2 y1 cd2 cd2
L r y3
A wd2 y1 0 cd2
Z
path nofill
M r y1
A wd2 y1 0 cd2
A wd2 y1 cd2 cd2
L r y3
A wd2 y1 0 cd2
L l y1",
    ),
    (
        "snip1Rect",
        "av adj 16667
gd a pin 0 adj 50000
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
gd it */ dx1 1 2
gd ir +- r 0 it
rect l it ir b
path
M l t
L x1 t
L r dx1
L r b
L l b
Z",
    ),
    (
        "mathPlus",
        "av adj1 23520
gd a1 pin 0 adj1 73490
gd dx1 */ w 73490 200000
gd dy1 */ h 73490 200000
gd dx2 */ ss a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- vc 0 dy1
gd y2 +- vc 0 dx2
gd y3 +- vc dx2 0
gd y4 +- vc dy1 0
rect l t r b
path
M x1 y2
L x2 y2
L x2 y1
L x3 y1
L x3 y2
L x4 y2
L x4 y3
L x3 y3
L x3 y4
L x2 y4
L x2 y3
L x1 y3
Z",
    ),
    (
        "plus",
        "av adj 25000
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
rect x1 x1 x2 y2
path
M l x1
L x1 x1
L x1 t
L x2 t
L x2 x1
L r x1
L r y2
L x2 y2
L x2 b
L x1 b
L x1 y2
L l y2
Z",
    ),
    (
        "leftBracket",
        "av adj 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
gd dx1 cos w 2700000
gd dy1 sin y1 2700000
gd il +- r 0 dx1
gd it +- y1 0 dy1
gd ib +- b dy1 y1
rect il it r ib
path nostroke
M r b
A w y1 cd4 cd4
L l y1
A w y1 cd2 cd4
Z
path nofill
M r b
A w y1 cd4 cd4
L l y1
A w y1 cd2 cd4",
    ),
    (
        "rightBracket",
        "av adj 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
gd dx1 cos w 2700000
gd dy1 sin y1 2700000
gd ir +- l dx1 0
gd it +- y1 0 dy1
gd ib +- b dy1 y1
rect l it ir ib
path nostroke
M l t
A w y1 3cd4 cd4
L r y2
A w y1 0 cd4
Z
path nofill
M l t
A w y1 3cd4 cd4
L r y2
A w y1 0 cd4",
    ),
    (
        "leftBrace",
        "av adj1 8333
av adj2 50000
gd a2 pin 0 adj2 100000
gd q1 +- 100000 0 a2
gd q2 min q1 a2
gd q3 */ q2 1 2
gd maxAdj1 */ q3 h ss
gd a1 pin 0 adj1 maxAdj1
gd y1 */ ss a1 100000
gd y3 */ h a2 100000
gd y4 +- y3 y1 0
gd dx1 cos wd2 2700000
gd dy1 sin y1 2700000
gd il +- r 0 dx1
gd it +- y1 0 dy1
gd ib +- b dy1 y1
rect il it r ib
path nostroke
M r b
A wd2 y1 cd4 cd4
L hc y4
A wd2 y1 0 -5400000
A wd2 y1 cd4 -5400000
L hc y1
A wd2 y1 cd2 cd4
Z
path nofill
M r b
A wd2 y1 cd4 cd4
L hc y4
A wd2 y1 0 -5400000
A wd2 y1 cd4 -5400000
L hc y1
A wd2 y1 cd2 cd4",
    ),
    (
        "parallelogram",
        "av adj 25000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x2 */ ss a 100000
gd x6 +- r 0 x2
gd q1 */ 5 a maxAdj
gd q2 +/ 1 q1 12
gd il */ w q2 1
gd it */ h q2 1
gd ir +- r 0 il
gd ib +- b 0 it
rect il it ir ib
path
M l b
L x2 t
L r t
L x6 b
Z",
    ),
    (
        "upArrowCallout",
        "av adj1 25000
av adj2 25000
av adj3 25000
av adj4 64977
gd maxAdj2 */ 50000 w ss
gd a2 pin 0 adj2 maxAdj2
gd maxAdj1 */ a2 2 1
gd a1 pin 0 adj1 maxAdj1
gd maxAdj3 */ 100000 h ss
gd a3 pin 0 adj3 maxAdj3
gd q2 */ a3 ss h
gd maxAdj4 +- 100000 0 q2
gd a4 pin 0 adj4 maxAdj4
gd dx1 */ ss a2 100000
gd dx2 */ ss a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd dy3 */ ss a3 100000
gd y3 +- t dy3 0
gd bh */ h a4 100000
gd y2 +- b 0 bh
rect l y2 r b
path
M l y2
L x2 y2
L x2 y3
L x1 y3
L hc t
L x4 y3
L x3 y3
L x3 y2
L r y2
L r b
L l b
Z",
    ),
    (
        "downArrowCallout",
        "av adj1 25000
av adj2 25000
av adj3 25000
av adj4 64977
gd maxAdj2 */ 50000 w ss
gd a2 pin 0 adj2 maxAdj2
gd maxAdj1 */ a2 2 1
gd a1 pin 0 adj1 maxAdj1
gd maxAdj3 */ 100000 h ss
gd a3 pin 0 adj3 maxAdj3
gd q2 */ a3 ss h
gd maxAdj4 +- 100000 0 q2
gd a4 pin 0 adj4 maxAdj4
gd dx1 */ ss a2 100000
gd dx2 */ ss a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd dy3 */ ss a3 100000
gd y3 +- b 0 dy3
gd y2 */ h a4 100000
rect l t r y2
path
M l t
L r t
L r y2
L x3 y2
L x3 y3
L x4 y3
L hc b
L x1 y3
L x2 y3
L x2 y2
L l y2
Z",
    ),
    (
        "rightArrowCallout",
        "av adj1 25000
av adj2 25000
av adj3 25000
av adj4 64977
gd maxAdj2 */ 50000 h ss
gd a2 pin 0 adj2 maxAdj2
gd maxAdj1 */ a2 2 1
gd a1 pin 0 adj1 maxAdj1
gd maxAdj3 */ 100000 w ss
gd a3 pin 0 adj3 maxAdj3
gd q2 */ a3 ss w
gd maxAdj4 +- 100000 0 q2
gd a4 pin 0 adj4 maxAdj4
gd dy1 */ ss a2 100000
gd dy2 */ ss a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc 0 dy2
gd y3 +- vc dy2 0
gd y4 +- vc dy1 0
gd dx3 */ ss a3 100000
gd x3 +- r 0 dx3
gd x2 */ w a4 100000
rect l t x2 b
path
M l t
L x2 t
L x2 y2
L x3 y2
L x3 y1
L r vc
L x3 y4
L x3 y3
L x2 y3
L x2 b
L l b
Z",
    ),
    (
        "bentUpArrow",
        "av adj1 25000
av adj2 25000
av adj3 25000
gd a2 pin 0 adj2 50000
gd a2d */ a2 2 1
gd a1 pin 0 adj1 a2d
gd maxAdj3 */ 100000 h ss
gd a3 pin 0 adj3 maxAdj3
gd dx1 */ ss a2 100000
gd dx2 */ ss a1 200000
gd th */ ss a1 100000
gd dy3 */ ss a3 100000
gd xc +- r 0 dx1
gd xl +- xc 0 dx2
gd xr +- xc dx2 0
gd hl +- xc 0 dx1
gd yb +- b 0 th
rect l t r b
path
M l yb
L xl yb
L xl dy3
L hl dy3
L xc t
L r dy3
L xr dy3
L xr b
L l b
Z",
    ),
    (
        "irregularSeal2",
        "gd il */ w 4046 21600
gd it */ h 6522 21600
gd ir */ w 17554 21600
gd ib */ h 15163 21600
rect il it ir ib
path w=21600 h=21600
M 11462 4342
L 14790 0
L 14525 5777
L 18007 3172
L 16380 6532
L 21600 6645
L 16985 9402
L 18270 11290
L 16380 12310
L 18877 15632
L 14640 14350
L 14942 17370
L 12180 15935
L 11612 18842
L 9872 17370
L 8700 19712
L 7527 18125
L 4917 21600
L 4805 18240
L 1285 17825
L 3330 15370
L 0 12877
L 3935 11592
L 1172 8270
L 5372 7817
L 4502 3625
L 8550 6382
L 9722 1887
Z",
    ),
    (
        "mathMultiply",
        "av adj1 23520
gd a1 pin 0 adj1 51965
gd th */ ss a1 100000
gd a at2 w h
gd sa sin 1 a
gd ca cos 1 a
gd ta tan 1 a
gd dl mod w h 0
gd rw */ dl 51965 100000
gd lM +- dl 0 rw
gd xM */ ca lM 2
gd yM */ sa lM 2
gd dxAM */ sa th 2
gd dyAM */ ca th 2
gd xA +- xM 0 dxAM
gd yA +- yM dyAM 0
gd xB +- xM dxAM 0
gd yB +- yM 0 dyAM
gd xBC +- hc 0 xB
gd yBC */ xBC ta 1
gd yC +- yBC yB 0
gd xD +- r 0 xB
gd xE +- r 0 xA
gd yFE +- vc 0 yA
gd xFE */ yFE 1 ta
gd xF +- xE 0 xFE
gd xL +- r 0 xF
gd yG +- b 0 yA
gd yH +- b 0 yB
gd yI +- b 0 yC
rect l t r b
path
M xA yA
L xB yB
L hc yC
L xD yB
L xE yA
L xF vc
L xE yG
L xD yH
L hc yI
L xB yH
L xA yG
L xL vc
Z",
    ),
    (
        "star7",
        "av adj 34601
av hf 102572
av vf 105210
gd a pin 0 adj 50000
gd swd2 */ wd2 hf 100000
gd shd2 */ hd2 vf 100000
gd svc */ vc vf 100000
gd iwd2 */ swd2 a 50000
gd ihd2 */ shd2 a 50000
gd s val 3085714.285714
gd hs val 1542857.142857
gd o0 val 16200000
gd s1 */ s 1 1
gd o1 +- o0 s1 0
gd s2 */ s 2 1
gd o2 +- o0 s2 0
gd s3 */ s 3 1
gd o3 +- o0 s3 0
gd s4 */ s 4 1
gd o4 +- o0 s4 0
gd s5 */ s 5 1
gd o5 +- o0 s5 0
gd s6 */ s 6 1
gd o6 +- o0 s6 0
gd i0 +- o0 hs 0
gd i1 +- o1 hs 0
gd i2 +- o2 hs 0
gd i3 +- o3 hs 0
gd i4 +- o4 hs 0
gd i5 +- o5 hs 0
gd i6 +- o6 hs 0
gd odx0 cos swd2 o0
gd ody0 sin shd2 o0
gd idx0 cos iwd2 i0
gd idy0 sin ihd2 i0
gd ox0 +- hc odx0 0
gd oy0 +- svc ody0 0
gd ix0 +- hc idx0 0
gd iy0 +- svc idy0 0
gd odx1 cos swd2 o1
gd ody1 sin shd2 o1
gd idx1 cos iwd2 i1
gd idy1 sin ihd2 i1
gd ox1 +- hc odx1 0
gd oy1 +- svc ody1 0
gd ix1 +- hc idx1 0
gd iy1 +- svc idy1 0
gd odx2 cos swd2 o2
gd ody2 sin shd2 o2
gd idx2 cos iwd2 i2
gd idy2 sin ihd2 i2
gd ox2 +- hc odx2 0
gd oy2 +- svc ody2 0
gd ix2 +- hc idx2 0
gd iy2 +- svc idy2 0
gd odx3 cos swd2 o3
gd ody3 sin shd2 o3
gd idx3 cos iwd2 i3
gd idy3 sin ihd2 i3
gd ox3 +- hc odx3 0
gd oy3 +- svc ody3 0
gd ix3 +- hc idx3 0
gd iy3 +- svc idy3 0
gd odx4 cos swd2 o4
gd ody4 sin shd2 o4
gd idx4 cos iwd2 i4
gd idy4 sin ihd2 i4
gd ox4 +- hc odx4 0
gd oy4 +- svc ody4 0
gd ix4 +- hc idx4 0
gd iy4 +- svc idy4 0
gd odx5 cos swd2 o5
gd ody5 sin shd2 o5
gd idx5 cos iwd2 i5
gd idy5 sin ihd2 i5
gd ox5 +- hc odx5 0
gd oy5 +- svc ody5 0
gd ix5 +- hc idx5 0
gd iy5 +- svc idy5 0
gd odx6 cos swd2 o6
gd ody6 sin shd2 o6
gd idx6 cos iwd2 i6
gd idy6 sin ihd2 i6
gd ox6 +- hc odx6 0
gd oy6 +- svc ody6 0
gd ix6 +- hc idx6 0
gd iy6 +- svc idy6 0
rect ix5 iy0 ix1 iy3
path
M ox0 oy0
L ix0 iy0
L ox1 oy1
L ix1 iy1
L ox2 oy2
L ix2 iy2
L ox3 oy3
L ix3 iy3
L ox4 oy4
L ix4 iy4
L ox5 oy5
L ix5 iy5
L ox6 oy6
L ix6 iy6
Z",
    ),
    (
        "cloudCallout",
        "av adj1 -20833
av adj2 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd il */ w 2977 21600
gd it */ h 3262 21600
gd ir */ w 17087 21600
gd ib */ h 17337 21600
gd r1 */ ss 1800 21600
gd r2 */ ss 1200 21600
gd bx1 */ dxPos 4 5
gd by1 */ dyPos 4 5
gd bx2 */ dxPos 19 20
gd by2 */ dyPos 19 20
gd cx1 +- hc bx1 0
gd cy1 +- vc by1 0
gd cx2 +- hc bx2 0
gd cy2 +- vc by2 0
gd lx1 +- cx1 0 r1
gd lx2 +- cx2 0 r2
rect il it ir ib
approx the trailing bubbles are placed by a fitted ratio, not the published guides
path w=43200 h=43200
M 3900 14370
A 6753 9190 -11429249 7426832
A 5333 7267 -8646143 5396714
A 4365 5945 -8748475 5983381
A 4857 6595 -7859164 7034504
A 5333 7273 -4722533 6541615
A 6775 9220 -2776035 7816140
A 5785 7867 37501 6842000
A 6752 9215 1347096 6910353
A 7720 10543 3974558 4542661
A 4360 5918 -16496525 8804134
A 4345 5945 -14809710 9151131
Z
path w=43200 h=43200 nofill
M 4693 26177
A 4345 5945 5204520 1585770
M 6928 34899
A 4360 5918 4416628 686848
M 16478 39090
A 6752 9215 8257449 844866
M 28827 34751
A 6752 9215 387196 959901
M 34129 22954
A 5785 7867 -4217541 4255042
M 41798 15354
A 5333 7273 1819082 1665090
M 38324 5426
A 4857 6595 -824660 891534
M 29078 3952
A 4857 6595 -8950887 1091722
M 22141 4720
A 4365 5945 -9809656 1061181
M 14000 5192
A 6753 9190 -4002417 739161
M 4127 15789
A 6753 9190 9459261 711490

path
M lx2 cy2
A r2 r2 cd2 21600000
Z
path
M lx1 cy1
A r1 r1 cd2 21600000
Z",
    ),
    (
        "curvedDownArrow",
        "av adj1 25000
av adj2 50000
av adj3 25000
gd maxAdj2 */ 50000 w ss
gd a2 pin 0 adj2 maxAdj2
gd a1 pin 0 adj1 100000
gd th */ ss a1 100000
gd aw */ ss a2 100000
gd q1 +/ th aw 4
gd wR +- wd2 0 q1
gd q7 */ wR 2 1
gd q8 */ q7 q7 1
gd q9 */ th th 1
gd q10 +- q8 0 q9
gd q11 sqrt q10
gd idy */ q11 h q7
gd maxAdj3 */ 100000 idy ss
gd a3 pin 0 adj3 maxAdj3
gd ah */ ss a3 100000
gd x3 +- wR th 0
gd q2 */ h h 1
gd q3 */ ah ah 1
gd q4 +- q2 0 q3
gd q5 sqrt q4
gd dx */ q5 wR h
gd x5 +- wR dx 0
gd x7 +- x3 dx 0
gd q6 +- aw 0 th
gd dh */ q6 1 2
gd x4 +- x5 0 dh
gd x8 +- x7 dh 0
gd aw2 */ aw 1 2
gd x6 +- r 0 aw2
gd y1 +- b 0 ah
gd swAng at2 ah dx
gd q12 */ th 1 2
gd dang2 at2 idy q12
gd ix +- wR q12 0
gd iy +- b 0 idy
gd stAng +- 3cd4 swAng 0
gd stAng2 +- 3cd4 0 dang2
gd swAng2 +- dang2 0 cd4
gd swAng3 +- cd4 dang2 0
gd swI +- swAng dang2 0
gd swO +- dang2 0 swAng
rect l t r y1
approx the darker inner face of the band is drawn with the normal fill
path nostroke
M l b
A wR h cd2 swAng3
A wR h stAng2 swAng2
Z
path nostroke
M ix iy
A wR h stAng2 swI
L x8 y1
L x6 b
L x4 y1
L x5 y1
A wR h stAng swO
Z
path nofill
M l b
A wR h cd2 swAng3
A wR h stAng2 swI
L x8 y1
L x6 b
L x4 y1
L x5 y1
A wR h stAng swO
A wR h stAng2 swAng2
Z",
    ),
    (
        "circularArrow",
        "av adj1 12500
av adj2 1142319
av adj3 20457681
av adj4 10800000
av adj5 12500
gd a5 pin 0 adj5 25000
gd a1 pin 0 adj1 a5
gd a2 pin 0 adj2 10800000
gd enAng pin 1 adj3 21599999
gd stAng pin 0 adj4 21599999
gd th */ ss a1 100000
gd thh */ ss a5 100000
gd th2 */ th 1 2
gd rx3 +- wd2 0 thh
gd ry3 +- hd2 0 thh
gd rx1 +- rx3 th2 0
gd ry1 +- ry3 th2 0
gd rx2 +- rx3 0 th2
gd ry2 +- ry3 0 th2
gd rxi +- rx3 0 thh
gd ryi +- ry3 0 thh
gd tipAng +- enAng a2 0
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd sw ?: sw1 sw1 sw2
gd nsw +- 0 0 sw
gd swt sin rx1 stAng
gd sht cos ry1 stAng
gd sdx cat2 rx1 sht swt
gd sdy sat2 ry1 sht swt
gd sx +- hc sdx 0
gd sy +- vc sdy 0
gd owt sin wd2 enAng
gd oht cos hd2 enAng
gd odx cat2 wd2 oht owt
gd ody sat2 hd2 oht owt
gd ox +- hc odx 0
gd oy +- vc ody 0
gd twt sin rx3 tipAng
gd tht cos ry3 tipAng
gd tdx cat2 rx3 tht twt
gd tdy sat2 ry3 tht twt
gd tx +- hc tdx 0
gd ty +- vc tdy 0
gd nwt sin rxi enAng
gd nht cos ryi enAng
gd ndx cat2 rxi nht nwt
gd ndy sat2 ryi nht nwt
gd nx +- hc ndx 0
gd ny +- vc ndy 0
gd ewt sin rx2 enAng
gd eht cos ry2 enAng
gd edx cat2 rx2 eht ewt
gd edy sat2 ry2 eht ewt
gd ex +- hc edx 0
gd ey +- vc edy 0
rect l t r b
path
M sx sy
A rx1 ry1 stAng sw
L ox oy
L tx ty
L nx ny
L ex ey
A rx2 ry2 enAng nsw
Z",
    ),
    (
        "bentConnector3",
        "av adj1 50000
gd x1 */ w adj1 100000
rect l t r b
path nofill
M l t
L x1 t
L x1 b
L r b",
    ),
    (
        "bentConnector4",
        "av adj1 50000
av adj2 50000
gd x1 */ w adj1 100000
gd x2 +/ x1 r 2
gd y2 */ h adj2 100000
gd y1 +/ t y2 2
rect l t r b
path nofill
M l t
L x1 t
L x1 y2
L r y2
L r b",
    ),
    (
        "curvedConnector2",
        "rect l t r b
path nofill
M l t
C wd2 t r hd2 r b",
    ),
    (
        "curvedConnector3",
        "av adj1 50000
gd x2 */ w adj1 100000
gd x1 +/ l x2 2
gd x3 +/ r x2 2
gd y3 */ h 3 4
rect l t r b
path nofill
M l t
C x1 t x2 hd4 x2 vc
C x2 y3 x3 b r b",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    fn geometry(kind: &str, guides: &str, w: f64, h: f64) -> Geometry {
        let source = format!(
            r#"<a:prstGeom xmlns:a="{A}" prst="{kind}"><a:avLst>{guides}</a:avLst></a:prstGeom>"#
        );
        let doc = Document::parse(&source).unwrap();
        let mut warnings = Warnings::default();
        let geometry = render(
            doc.root_element(),
            find(kind).unwrap(),
            w,
            h,
            "",
            &mut warnings,
        );
        assert!(warnings.into_parts().0.is_empty());
        geometry
    }
    fn close(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
        [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)]
            .iter()
            .all(|(x, y)| (x - y).abs() < 1e-6)
    }
    fn custom_geometry(inner: &str, w: f64, h: f64) -> (Geometry, Vec<String>) {
        let source = format!(r#"<a:custGeom xmlns:a="{A}">{inner}</a:custGeom>"#);
        let doc = Document::parse(&source).unwrap();
        let mut warnings = Warnings::default();
        let geometry = custom(doc.root_element(), w, h, "", &mut warnings);
        (geometry, warnings.into_parts().0)
    }
    #[test]
    fn custom_guides_feed_the_path_and_text_rectangle() {
        // 60x40 px shape = 571500x381000 EMU. Guides chain through each other and the
        // built-in w/h/hc; the path space is the shape space (no w/h on the path).
        let (geometry, warnings) = custom_geometry(
            r#"<a:avLst/><a:gdLst><a:gd name="inset" fmla="*/ w 1 4"/><a:gd name="right" fmla="+- w 0 inset"/><a:gd name="mid" fmla="val vc"/><a:gd name="top" fmla="*/ mid 1 2"/></a:gdLst><a:rect l="inset" t="top" r="right" b="b"/><a:pathLst><a:path><a:moveTo><a:pt x="inset" y="t"/></a:moveTo><a:lnTo><a:pt x="right" y="t"/></a:lnTo><a:lnTo><a:pt x="hc" y="mid"/></a:lnTo><a:close/></a:path></a:pathLst>"#,
            60.0,
            40.0,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            geometry.svg.contains("<path d=\"M15 0 L45 0 L30 20 Z\""),
            "{}",
            geometry.svg
        );
        assert!(
            close(geometry.text, (15.0, 10.0, 30.0, 30.0)),
            "{:?}",
            geometry.text
        );
    }
    #[test]
    fn custom_paths_scale_from_their_own_space_and_keep_flags() {
        let (geometry, warnings) = custom_geometry(
            r#"<a:pathLst><a:path w="100" h="100" stroke="0"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:cubicBezTo><a:pt x="50" y="0"/><a:pt x="100" y="50"/><a:pt x="100" y="100"/></a:cubicBezTo><a:quadBezTo><a:pt x="0" y="100"/><a:pt x="0" y="0"/></a:quadBezTo><a:close/></a:path><a:path w="100" h="100" fill="none"><a:moveTo><a:pt x="100" y="50"/></a:moveTo><a:arcTo wR="50" hR="50" stAng="0" swAng="10800000"/></a:path><a:path w="10" h="10" fill="darken" extrusionOk="0"><a:lnTo><a:pt x="10" y="10"/></a:lnTo></a:path></a:pathLst>"#,
            60.0,
            40.0,
        );
        let svg = &geometry.svg;
        assert!(
            svg.contains(r#"<path d="M0 0 C30 0 60 20 60 40 Q0 40 0 0 Z" marker-start="none" marker-end="none" stroke="none"/>"#),
            "{svg}"
        );
        // Open, unfilled arc: half an ellipse from the right edge clockwise to the left.
        assert!(
            svg.contains(r#"<path d="M60 20 A30 20 0 0 1 30 40 A30 20 0 0 1 0 20" fill="none"/>"#),
            "{svg}"
        );
        // A path that starts without a move begins at the origin.
        assert!(svg.contains(r#"<path d="M0 0 L60 40"/>"#), "{svg}");
        assert_eq!(geometry.text, (0.0, 0.0, 60.0, 40.0));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("darken"), "{warnings:?}");
    }
    #[test]
    fn custom_geometry_with_unknown_guides_is_omitted() {
        let (geometry, warnings) = custom_geometry(
            r#"<a:pathLst><a:path><a:moveTo><a:pt x="missing" y="0"/></a:moveTo></a:path></a:pathLst>"#,
            60.0,
            40.0,
        );
        assert!(geometry.svg.is_empty());
        assert!(warnings[0].contains("non-finite"), "{warnings:?}");
    }
    #[test]
    fn formulas_follow_the_guide_operators() {
        let guides = Guides::new(60.0, 40.0);
        let eval = |f: &str| guides.formula(&f.split_whitespace().collect::<Vec<_>>());
        assert_eq!(eval("*/ w 1 2"), 30.0);
        assert_eq!(eval("+- r 10 5"), 65.0);
        assert_eq!(eval("+/ 10 20 3"), 10.0);
        assert_eq!(eval("?: 0 1 2"), 2.0);
        assert_eq!(eval("pin 0 150000 100000"), 100000.0);
        assert_eq!(eval("max wd2 hd2"), 30.0);
        assert_eq!(eval("mod 3 4 0"), 5.0);
        assert!((eval("cos 10 cd4") - 0.0).abs() < 1e-9);
        assert!((eval("sin 10 cd4") - 10.0).abs() < 1e-9);
        assert!((eval("at2 1 1") - 2_700_000.0).abs() < 1e-6);
        assert!((eval("cat2 10 3 4") - 6.0).abs() < 1e-9);
        assert!((eval("sat2 10 3 4") - 8.0).abs() < 1e-9);
        assert_eq!(guides.get("3cd4"), 16_200_000.0);
        assert_eq!(guides.get("ssd8"), 5.0);
        assert!(eval("unknown 1").is_nan());
    }
    #[test]
    fn text_rectangles_follow_spec_guides() {
        assert!(close(
            geometry("chevron", "", 60.0, 40.0).text,
            (20.0, 0.0, 20.0, 40.0)
        ));
        assert!(close(
            geometry("triangle", "", 60.0, 40.0).text,
            (15.0, 20.0, 30.0, 20.0)
        ));
        let r = std::f64::consts::FRAC_1_SQRT_2;
        let pie = geometry("pie", "", 60.0, 40.0).text;
        assert!(
            close(pie, (30.0 - 30.0 * r, 20.0 - 20.0 * r, 60.0 * r, 40.0 * r)),
            "{pie:?}"
        );
    }
    #[test]
    fn callout_and_bracket_text_rectangles_follow_spec_guides() {
        // Callouts keep the whole box as the text area, wherever the tail points.
        for kind in ["wedgeRectCallout", "borderCallout1", "borderCallout2"] {
            assert!(close(
                geometry(kind, "", 60.0, 40.0).text,
                (0.0, 0.0, 60.0, 40.0)
            ));
        }
        // Brackets inset the text by the corner arc: y1 = 40 * 8333 / 100000.
        let (dx, y1) = (60.0 * std::f64::consts::FRAC_1_SQRT_2, 40.0 * 0.08333);
        let inset = y1 * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
        assert!(close(
            geometry("leftBracket", "", 60.0, 40.0).text,
            (60.0 - dx, inset, dx, 40.0 - 2.0 * inset)
        ));
        assert!(close(
            geometry("rightBracket", "", 60.0, 40.0).text,
            (0.0, inset, dx, 40.0 - 2.0 * inset)
        ));
        // The arrow callout text area is the box part, below the up arrow (adj4 64977).
        let up = geometry("upArrowCallout", "", 60.0, 40.0).text;
        assert!(
            close(up, (0.0, 40.0 * 0.35023, 60.0, 40.0 * 0.64977)),
            "{up:?}"
        );
    }
    #[test]
    fn arcs_end_on_the_ellipse_at_the_visual_angle() {
        // Half pie from 0 to 180 degrees: start at the right, sweep clockwise to the left.
        let pie = geometry(
            "pie",
            r#"<a:gd name="adj1" fmla="val 0"/><a:gd name="adj2" fmla="val 10800000"/>"#,
            60.0,
            40.0,
        );
        assert!(
            pie.svg
                .contains("M60 20 A30 20 0 0 1 30 40 A30 20 0 0 1 0 20"),
            "{}",
            pie.svg
        );
        // Closed outlines never carry line-end markers; open connectors keep them.
        assert!(pie.svg.contains("marker-end=\"none\""));
        let connector = geometry("bentConnector3", "", 60.0, 40.0);
        assert!(
            connector
                .svg
                .contains("<path d=\"M0 0 L30 0 L30 40 L60 40\" fill=\"none\"/>")
        );
    }
}
