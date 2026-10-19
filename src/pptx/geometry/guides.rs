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
    for line in definition.lines() {
        let mut tokens = line.split_whitespace();
        let Some(head) = tokens.next() else { continue };
        let args: Vec<&str> = tokens.collect();
        match head {
            "av" => {}
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
    let text = text.unwrap_or((0.0, 0.0, w, h));
    let text_finite = [text.0, text.1, text.2, text.3]
        .iter()
        .all(|v| v.is_finite());
    if !text_finite || paths.iter().any(|p| !p.finite) {
        warn(
            warnings,
            "Preset shape guides produced non-finite values; outline omitted",
        );
        return Geometry {
            svg: String::new(),
            text: (0.0, 0.0, w, h),
        };
    }
    // Paint every path inside one group so per-path fill/stroke overrides inherit the
    // shape style; only open paths without fill keep line-end markers.
    let mut svg = format!("<g {style}>");
    for path in paths {
        let mut attrs = String::new();
        if !path.fill {
            attrs.push_str(" fill=\"none\"");
        } else {
            attrs.push_str(" marker-start=\"none\" marker-end=\"none\"");
        }
        if !path.stroke {
            attrs.push_str(" stroke=\"none\"");
        }
        svg.push_str(&format!("<path d=\"{}\"{attrs}/>", path.data.trim()));
    }
    svg.push_str("</g>");
    Geometry { svg, text }
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
        Self {
            data: String::new(),
            fill: !attrs.contains(&"nofill"),
            stroke: !attrs.contains(&"nostroke"),
            scale: (size("w=", w), size("h=", h)),
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
/// `av name default`, `gd name formula`, `rect l t r b`, `path [w=] [h=] [nofill]
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
