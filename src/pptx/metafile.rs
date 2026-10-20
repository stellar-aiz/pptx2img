//! Translate EMF and placeable WMF pictures to SVG, written from MS-EMF and MS-WMF.
//!
//! The record reader (`emf`, `wmf`) drives one shared player: every coordinate is mapped
//! to the output space when its record is played (world transform, then the
//! window/viewport mapping), so transforms, paths, and clip regions need no SVG
//! transform of their own. Embedded bitmaps are decoded (`dib`) and encoded as PNG under
//! the PNG/JPEG size policy in `raster`. Text records are not drawn and are reported;
//! records outside the subset are skipped with one diagnostic per record type.
//! Malformed input and inputs beyond the limits below are rejected as a whole, so the
//! caller omits the picture instead of drawing part of a corrupt file.
use super::{Code, raster};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::fmt::Write;

mod dib;
mod emf;
mod wmf;

/// Most records played from one picture.
const MAX_RECORDS: usize = 500_000;
/// Most points read from all drawing records of one picture.
const MAX_POINTS: usize = 2_000_000;
/// Most SVG elements written for one picture (the SVG gate also caps nodes at 100,000).
const MAX_ELEMENTS: usize = 50_000;
/// Most source bitmap pixels decoded from one picture, across all bitmap records.
const MAX_BITMAP_PIXELS: usize = 67_108_864;
/// Deepest `SaveDC` nesting.
const MAX_SAVED: usize = 1_024;
/// Most graphics objects alive at once.
const MAX_OBJECTS: usize = 65_536;

/// Whether `data` starts like an EMF (`EMR_HEADER` with the `" EMF"` signature at offset
/// 40) or a placeable WMF (key `0x9AC6CDD7`).
pub(super) fn is_metafile(data: &[u8]) -> bool {
    emf::sniff(data) || wmf::sniff(data)
}

/// A translated picture: a self-contained SVG document, its natural size in px at
/// 96 dpi, and diagnostics without the part name (the caller appends it).
pub(super) struct Translated {
    pub svg: Vec<u8>,
    pub size: (f64, f64),
    pub notes: Vec<(Code, String)>,
}

/// Translate an EMF or placeable WMF picture, or return why it cannot be drawn.
pub(super) fn translate(data: &[u8]) -> Result<Translated, String> {
    if emf::sniff(data) {
        emf::translate(data)
    } else if wmf::sniff(data) {
        wmf::translate(data)
    } else {
        Err("Not an EMF or placeable WMF picture".into())
    }
}

/// Little-endian reads with bounds checks; `None` means the record is too short.
#[derive(Clone, Copy)]
struct Bytes<'a>(&'a [u8]);
impl<'a> Bytes<'a> {
    fn get<const N: usize>(self, at: usize) -> Option<[u8; N]> {
        self.0.get(at..at.checked_add(N)?)?.try_into().ok()
    }
    fn u16(self, at: usize) -> Option<u16> {
        self.get(at).map(u16::from_le_bytes)
    }
    fn i16(self, at: usize) -> Option<i16> {
        self.get(at).map(i16::from_le_bytes)
    }
    fn u32(self, at: usize) -> Option<u32> {
        self.get(at).map(u32::from_le_bytes)
    }
    fn i32(self, at: usize) -> Option<i32> {
        self.get(at).map(i32::from_le_bytes)
    }
    fn f32(self, at: usize) -> Option<f32> {
        self.get(at).map(f32::from_le_bytes)
    }
    /// `len` bytes from `at`, both given as record fields.
    fn slice(self, at: u32, len: u32) -> Option<&'a [u8]> {
        let at = at as usize;
        self.0.get(at..at.checked_add(len as usize)?)
    }
}

/// An affine map `x' = a x + c y + e`, `y' = b x + d y + f` (MS-EMF `XForm` order).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Affine([f64; 6]);
impl Affine {
    const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    fn apply(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
    /// `self` first, then `next`.
    fn then(&self, next: &Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [na, nb, nc, nd, ne, nf] = next.0;
        Affine([
            na * a + nc * b,
            nb * a + nd * b,
            na * c + nc * d,
            nb * c + nd * d,
            na * e + nc * f + ne,
            nb * e + nd * f + nf,
        ])
    }
    /// Scale applied to lengths (the square root of the area scale).
    fn scale(&self) -> f64 {
        let [a, b, c, d, ..] = self.0;
        (a * d - b * c).abs().sqrt()
    }
    fn is_finite(&self) -> bool {
        self.0.iter().all(|v| v.is_finite())
    }
}

/// A colour as `0xRRGGBB`, from a `ColorRef` (`0x00BBGGRR`).
fn color_ref(value: u32) -> u32 {
    let [r, g, b, _] = value.to_le_bytes();
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

#[derive(Clone, Debug, PartialEq)]
struct Pen {
    /// `None` for `PS_NULL` or a hollow pen brush.
    color: Option<u32>,
    /// Width in logical units; `None` for a cosmetic (one device pixel) pen.
    width: Option<f64>,
    /// Dash lengths in multiples of the drawn width (empty for solid).
    dash: Vec<f64>,
    cap: &'static str,
    join: &'static str,
}
impl Pen {
    const BLACK: Pen = Pen::solid(0x000000);
    const fn solid(color: u32) -> Pen {
        Pen {
            color: Some(color),
            width: None,
            dash: Vec::new(),
            cap: "round",
            join: "round",
        }
    }
    /// A pen from a `PenStyle` bit field (MS-WMF 2.1.1.23, shared by EMF).
    fn from_style(style: u32, width: Option<f64>, color: u32) -> Pen {
        let dash = match style & 0x0F {
            1 => vec![6.0, 2.0],
            2 => vec![1.0, 1.0],
            3 => vec![3.0, 2.0, 1.0, 2.0],
            4 => vec![3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            _ => Vec::new(),
        };
        Pen {
            color: (style & 0x0F != 5).then_some(color),
            width,
            dash,
            cap: match style & 0x0F00 {
                0x0100 => "square",
                0x0200 => "butt",
                _ => "round",
            },
            join: match style & 0xF000 {
                0x1000 => "bevel",
                0x2000 => "miter",
                _ => "round",
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Brush {
    /// `None` for a hollow brush.
    color: Option<u32>,
}

#[derive(Clone, Debug)]
enum Object {
    Pen(Pen),
    Brush(Brush),
    /// Fonts, palettes, regions, and colour spaces: selectable, but not drawn here.
    Other,
}

/// Stock object `index` (`0x80000000 | index` in EMF).
fn stock(index: u32) -> Option<Object> {
    let brush = |color| Object::Brush(Brush { color });
    Some(match index {
        0 => brush(Some(0xFFFFFF)),
        1 => brush(Some(0xC0C0C0)),
        2 => brush(Some(0x808080)),
        3 => brush(Some(0x404040)),
        4 => brush(Some(0x000000)),
        5 => brush(None),
        6 => Object::Pen(Pen::solid(0xFFFFFF)),
        7 => Object::Pen(Pen::BLACK),
        8 => Object::Pen(Pen {
            color: None,
            ..Pen::BLACK
        }),
        18 => brush(Some(0xFFFFFF)),
        19 => Object::Pen(Pen::BLACK),
        10..=17 => Object::Other,
        _ => return None,
    })
}

/// Clip state: a chain of intersected clip paths and a mask that removes excluded areas.
#[derive(Clone, Debug, Default)]
struct Clip {
    /// Id of the innermost `clipPath`; each one clips the previous.
    intersect: Option<String>,
    /// Path data of excluded areas, and the `mask` that removes them.
    excludes: Vec<String>,
    mask: Option<String>,
}

/// Device-context state that `SaveDC`/`RestoreDC` save and restore.
#[derive(Clone, Debug)]
struct Dc {
    map_mode: u32,
    window_org: (f64, f64),
    window_ext: (f64, f64),
    viewport_org: (f64, f64),
    viewport_ext: (f64, f64),
    world: Affine,
    pen: Pen,
    brush: Brush,
    even_odd: bool,
    /// Current position in logical units.
    position: (f64, f64),
    clip: Clip,
}

/// How logical units reach the output (SVG user) space.
enum Page {
    /// Output units are reference-device pixels; `mm` is device pixels per millimetre.
    Emf { mm: (f64, f64) },
    /// Output units are the placeable header's logical units; the window maps onto `bbox`
    /// (`left, top, width, height`).
    Wmf { bbox: (f64, f64, f64, f64) },
}

/// Path data in output units, built from logical points with one transform.
struct Outline {
    matrix: Affine,
    d: String,
    finite: bool,
}
impl Outline {
    fn new(matrix: Affine) -> Self {
        Outline {
            matrix,
            d: String::new(),
            finite: true,
        }
    }
    fn point(&mut self, command: char, points: &[(f64, f64)]) {
        self.d.push(command);
        for (i, &p) in points.iter().enumerate() {
            let (x, y) = self.matrix.apply(p);
            self.finite &= x.is_finite() && y.is_finite();
            if i > 0 {
                self.d.push(' ');
            }
            let _ = write!(self.d, "{} {}", number(x), number(y));
        }
    }
    fn move_to(&mut self, p: (f64, f64)) {
        self.point('M', &[p]);
    }
    fn line_to(&mut self, p: (f64, f64)) {
        self.point('L', &[p]);
    }
    fn cubic_to(&mut self, c1: (f64, f64), c2: (f64, f64), p: (f64, f64)) {
        self.point('C', &[c1, c2, p]);
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
    fn polygon(&mut self, points: &[(f64, f64)], closed: bool) {
        let Some((&first, rest)) = points.split_first() else {
            return;
        };
        self.move_to(first);
        for &p in rest {
            self.line_to(p);
        }
        if closed {
            self.close();
        }
    }
    fn rect(&mut self, (l, t, r, b): (f64, f64, f64, f64)) {
        self.polygon(&[(l, t), (r, t), (r, b), (l, b)], true);
    }
    /// An ellipse inscribed in the box, as four cubic arcs (counter-clockwise, as GDI).
    fn ellipse(&mut self, (l, t, r, b): (f64, f64, f64, f64)) {
        let (cx, cy, rx, ry) = ((l + r) / 2.0, (t + b) / 2.0, (r - l) / 2.0, (b - t) / 2.0);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        self.move_to((cx + rx, cy));
        self.cubic_to((cx + rx, cy - ky), (cx + kx, cy - ry), (cx, cy - ry));
        self.cubic_to((cx - kx, cy - ry), (cx - rx, cy - ky), (cx - rx, cy));
        self.cubic_to((cx - rx, cy + ky), (cx - kx, cy + ry), (cx, cy + ry));
        self.cubic_to((cx + kx, cy + ry), (cx + rx, cy + ky), (cx + rx, cy));
        self.close();
    }
    /// A rectangle whose corners are quarter ellipses of `w` x `h` (full ellipse size).
    fn round_rect(&mut self, (l, t, r, b): (f64, f64, f64, f64), (w, h): (f64, f64)) {
        let rx = (w.abs() / 2.0).min((r - l) / 2.0);
        let ry = (h.abs() / 2.0).min((b - t) / 2.0);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        self.move_to((l + rx, t));
        self.line_to((r - rx, t));
        self.cubic_to((r - rx + kx, t), (r, t + ry - ky), (r, t + ry));
        self.line_to((r, b - ry));
        self.cubic_to((r, b - ry + ky), (r - rx + kx, b), (r - rx, b));
        self.line_to((l + rx, b));
        self.cubic_to((l + rx - kx, b), (l, b - ry + ky), (l, b - ry));
        self.line_to((l, t + ry));
        self.cubic_to((l, t + ry - ky), (l + rx - kx, t), (l + rx, t));
        self.close();
    }
}
/// Control-point distance of a quarter ellipse as a cubic Bézier, per unit radius.
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// A rectangle with `left <= right` and `top <= bottom`.
fn normalized(l: f64, t: f64, r: f64, b: f64) -> (f64, f64, f64, f64) {
    (l.min(r), t.min(b), l.max(r), t.max(b))
}

/// A coordinate rounded to 1/1000 of an output unit.
fn number(v: f64) -> f64 {
    let rounded = (v * 1000.0).round() / 1000.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// A path being recorded between `BeginPath` and `EndPath`.
struct Recording {
    d: String,
    /// Whether a figure is open, so a following line continues it.
    open: bool,
    finite: bool,
}

/// The source of a blit: the bitmap, the source rectangle (`x, y, width, height` in
/// pixels), whether that rectangle uses DIB-function coordinates, and how the alpha
/// channel is read.
struct Source<'a> {
    image: dib::Dib<'a>,
    rect: (f64, f64, f64, f64),
    dib_origin: bool,
    alpha: dib::Alpha,
}

/// Plays records of either format into SVG elements.
struct Player {
    dc: Dc,
    saved: Vec<Dc>,
    page: Page,
    /// Output units of one 1/96-inch pixel, the width of a cosmetic pen.
    hairline: f64,
    /// Output viewport (`x, y, width, height`), also the extent of exclusion masks.
    view: (f64, f64, f64, f64),
    /// `Some` while a path bracket is open or a closed path awaits use.
    path: Option<Recording>,
    recording: bool,
    body: String,
    defs: String,
    elements: usize,
    points: usize,
    pixels: usize,
    ids: usize,
    notes: Vec<(Code, String)>,
}
impl Player {
    fn new(page: Page, hairline: f64, view: (f64, f64, f64, f64)) -> Self {
        Player {
            dc: Dc {
                map_mode: 1,
                window_org: (0.0, 0.0),
                window_ext: (1.0, 1.0),
                viewport_org: (0.0, 0.0),
                viewport_ext: (1.0, 1.0),
                world: Affine::IDENTITY,
                pen: Pen::BLACK,
                brush: Brush {
                    color: Some(0xFFFFFF),
                },
                even_odd: true,
                position: (0.0, 0.0),
                clip: Clip::default(),
            },
            saved: Vec::new(),
            page,
            hairline,
            view,
            path: None,
            recording: false,
            body: String::new(),
            defs: String::new(),
            elements: 0,
            points: 0,
            pixels: 0,
            ids: 0,
            notes: Vec::new(),
        }
    }

    /// Record one diagnostic; a repeated message is kept once.
    fn note(&mut self, code: Code, message: String) {
        if !self.notes.iter().any(|(_, m)| *m == message) {
            self.notes.push((code, message));
        }
    }
    fn unsupported_record(&mut self, name: &str) {
        self.note(
            Code::Unsupported,
            format!("Unsupported metafile record ignored ({name})"),
        );
    }
    fn text_record(&mut self, name: &str) {
        self.note(
            Code::Unsupported,
            format!("Text in metafile pictures is not rendered ({name})"),
        );
    }

    /// Count `n` points against the picture limit before they are read.
    fn take_points(&mut self, n: usize) -> Result<(), String> {
        self.points = self.points.saturating_add(n);
        if self.points > MAX_POINTS {
            return Err(format!(
                "Metafile picture exceeds {MAX_POINTS} points; not rendered"
            ));
        }
        Ok(())
    }
    fn element(&mut self) -> Result<(), String> {
        self.elements += 1;
        if self.elements > MAX_ELEMENTS {
            return Err(format!(
                "Metafile picture exceeds {MAX_ELEMENTS} drawing elements; not rendered"
            ));
        }
        Ok(())
    }
    fn id(&mut self) -> String {
        self.ids += 1;
        format!("m{}", self.ids)
    }

    /// Logical units to output units under the current mapping and world transform.
    fn matrix(&self) -> Affine {
        let dc = &self.dc;
        let page = match self.page {
            Page::Emf { mm } => {
                // Units per millimetre of the fixed map modes (MS-EMF 2.1.21); y grows upwards.
                let metric = |per_mm: f64| (mm.0 / per_mm, -mm.1 / per_mm);
                let (sx, sy) = match dc.map_mode {
                    2 => metric(10.0),
                    3 => metric(100.0),
                    4 => metric(100.0 / 25.4),
                    5 => metric(1000.0 / 25.4),
                    6 => metric(1440.0 / 25.4),
                    7 | 8 => {
                        let ratio = |v: f64, w: f64| if w == 0.0 { 1.0 } else { v / w };
                        let sx = ratio(dc.viewport_ext.0, dc.window_ext.0);
                        let sy = ratio(dc.viewport_ext.1, dc.window_ext.1);
                        if dc.map_mode == 7 {
                            let s = sx.abs().min(sy.abs());
                            (s.copysign(sx), s.copysign(sy))
                        } else {
                            (sx, sy)
                        }
                    }
                    _ => (1.0, 1.0),
                };
                Affine([
                    sx,
                    0.0,
                    0.0,
                    sy,
                    dc.viewport_org.0 - dc.window_org.0 * sx,
                    dc.viewport_org.1 - dc.window_org.1 * sy,
                ])
            }
            Page::Wmf { bbox } => {
                let (x, y, w, h) = bbox;
                let ratio = |v: f64, ext: f64| if ext == 0.0 { 1.0 } else { v / ext };
                let sx = ratio(w, dc.window_ext.0);
                let sy = ratio(h, dc.window_ext.1);
                Affine([
                    sx,
                    0.0,
                    0.0,
                    sy,
                    x - dc.window_org.0 * sx,
                    y - dc.window_org.1 * sy,
                ])
            }
        };
        dc.world.then(&page)
    }

    fn save(&mut self) -> Result<(), String> {
        if self.saved.len() >= MAX_SAVED {
            return Err(format!(
                "Metafile picture nests SaveDC deeper than {MAX_SAVED}; not rendered"
            ));
        }
        self.saved.push(self.dc.clone());
        Ok(())
    }
    /// `RestoreDC`: negative is relative to the top of the stack, positive is absolute.
    fn restore(&mut self, which: i32) {
        let depth = self.saved.len() as i64;
        let index = if which < 0 {
            depth + i64::from(which)
        } else {
            i64::from(which) - 1
        };
        if (0..depth).contains(&index) {
            self.dc = self.saved[index as usize].clone();
            self.saved.truncate(index as usize);
        }
    }

    /// A brush from a `LogBrush` style and `ColorRef` (MS-WMF 2.1.1.4 `BrushStyle`).
    fn brush(&mut self, record: &str, style: u32, color: u32) -> Brush {
        let color = color_ref(color);
        match style {
            0 => Brush { color: Some(color) },
            1 => Brush { color: None },
            2 => {
                self.note(
                    Code::Approximation,
                    format!("Hatched metafile brushes are drawn as solid fills ({record})"),
                );
                Brush { color: Some(color) }
            }
            _ => {
                self.note(
                    Code::Unsupported,
                    format!("Pattern brushes in metafile pictures are not drawn ({record})"),
                );
                Brush { color: None }
            }
        }
    }
    fn select(&mut self, object: Object) {
        match object {
            Object::Pen(pen) => self.dc.pen = pen,
            Object::Brush(brush) => self.dc.brush = brush,
            Object::Other => {}
        }
    }

    // -- drawing ---------------------------------------------------------------

    /// Start a figure for logical drawing: inside a path bracket the figure is
    /// recorded; otherwise it is drawn when `finish` is called.
    fn outline(&self) -> Outline {
        Outline::new(self.matrix())
    }
    /// Draw (or record, inside a path bracket) a finished outline.
    fn finish(&mut self, outline: Outline, fill: bool, stroke: bool) -> Result<(), String> {
        if self.recording {
            if let Some(path) = &mut self.path {
                path.d.push_str(&outline.d);
                path.finite &= outline.finite;
                path.open = !outline.d.ends_with('Z');
            }
            return Ok(());
        }
        if !outline.finite || outline.d.is_empty() {
            return Ok(());
        }
        let width = self.stroke_width(&outline.matrix);
        self.paint(&outline.d, fill, stroke, width)
    }
    /// Pen width in output units under `matrix`, at least one device pixel (as GDI draws).
    fn stroke_width(&self, matrix: &Affine) -> f64 {
        match self.dc.pen.width {
            Some(width) => (width * matrix.scale()).max(self.hairline),
            None => self.hairline,
        }
    }
    /// Emit one `<path>` with the current brush and pen.
    fn paint(&mut self, d: &str, fill: bool, stroke: bool, width: f64) -> Result<(), String> {
        let fill = fill.then_some(self.dc.brush.color).flatten();
        let pen = stroke
            .then(|| self.dc.pen.clone())
            .filter(|p| p.color.is_some());
        if fill.is_none() && pen.is_none() {
            return Ok(());
        }
        self.element()?;
        let mut element = format!("<path d=\"{d}\"");
        match fill {
            Some(color) => {
                let rule = if self.dc.even_odd {
                    "evenodd"
                } else {
                    "nonzero"
                };
                let _ = write!(element, " fill=\"#{color:06x}\" fill-rule=\"{rule}\"");
            }
            None => element.push_str(" fill=\"none\""),
        }
        if let Some(pen) = pen {
            let _ = write!(
                element,
                " stroke=\"#{:06x}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
                pen.color.unwrap_or_default(),
                number(width),
                pen.cap,
                pen.join
            );
            if !pen.dash.is_empty() {
                let dash: Vec<_> = pen
                    .dash
                    .iter()
                    .map(|v| number(v * width).to_string())
                    .collect();
                let _ = write!(element, " stroke-dasharray=\"{}\"", dash.join(" "));
            }
        }
        element.push_str(&self.clip_attributes());
        element.push_str("/>");
        self.body.push_str(&element);
        Ok(())
    }
    fn clip_attributes(&self) -> String {
        let mut attributes = String::new();
        if let Some(id) = &self.dc.clip.intersect {
            let _ = write!(attributes, " clip-path=\"url(#{id})\"");
        }
        if let Some(id) = &self.dc.clip.mask {
            let _ = write!(attributes, " mask=\"url(#{id})\"");
        }
        attributes
    }

    fn polygon(&mut self, points: &[(f64, f64)]) -> Result<(), String> {
        let mut outline = self.outline();
        outline.polygon(points, true);
        self.finish(outline, true, true)
    }
    fn polyline(&mut self, points: &[(f64, f64)]) -> Result<(), String> {
        let mut outline = self.outline();
        outline.polygon(points, false);
        self.finish(outline, false, true)
    }
    /// Several closed figures drawn as one filled shape.
    fn poly_polygon(&mut self, polygons: &[Vec<(f64, f64)>]) -> Result<(), String> {
        let mut outline = self.outline();
        for points in polygons {
            outline.polygon(points, true);
        }
        self.finish(outline, true, true)
    }
    fn poly_polyline(&mut self, lines: &[Vec<(f64, f64)>]) -> Result<(), String> {
        let mut outline = self.outline();
        for points in lines {
            outline.polygon(points, false);
        }
        self.finish(outline, false, true)
    }
    /// `PolyBezier`: a start point and then three points per curve.
    fn bezier(&mut self, points: &[(f64, f64)]) -> Result<(), String> {
        let Some((&start, rest)) = points.split_first() else {
            return Ok(());
        };
        let mut outline = self.outline();
        outline.move_to(start);
        for c in rest.chunks_exact(3) {
            outline.cubic_to(c[0], c[1], c[2]);
        }
        self.finish(outline, false, true)
    }
    /// Lines (`curves == false`) or curves from the current position, which moves to the
    /// last point.
    fn continue_to(&mut self, points: &[(f64, f64)], curves: bool) -> Result<(), String> {
        let Some(&last) = points.last() else {
            return Ok(());
        };
        let mut outline = self.outline();
        let open = self.recording && self.path.as_ref().is_some_and(|p| p.open);
        if !open {
            outline.move_to(self.dc.position);
        }
        if curves {
            for c in points.chunks_exact(3) {
                outline.cubic_to(c[0], c[1], c[2]);
            }
        } else {
            for &p in points {
                outline.line_to(p);
            }
        }
        self.dc.position = last;
        if self.recording {
            if let Some(path) = &mut self.path {
                path.d.push_str(&outline.d);
                path.finite &= outline.finite;
                path.open = true;
            }
            return Ok(());
        }
        self.finish(outline, false, true)
    }
    fn move_to(&mut self, p: (f64, f64)) {
        self.dc.position = p;
        if let (true, Some(path)) = (self.recording, &mut self.path) {
            path.open = false;
        }
    }
    fn rectangle(&mut self, r: (f64, f64, f64, f64)) -> Result<(), String> {
        let mut outline = self.outline();
        outline.rect(normalized(r.0, r.1, r.2, r.3));
        self.finish(outline, true, true)
    }
    fn ellipse(&mut self, r: (f64, f64, f64, f64)) -> Result<(), String> {
        let mut outline = self.outline();
        outline.ellipse(normalized(r.0, r.1, r.2, r.3));
        self.finish(outline, true, true)
    }
    fn round_rect(&mut self, r: (f64, f64, f64, f64), corner: (f64, f64)) -> Result<(), String> {
        let mut outline = self.outline();
        outline.round_rect(normalized(r.0, r.1, r.2, r.3), corner);
        self.finish(outline, true, true)
    }
    /// Fill a logical rectangle with the brush and no outline (`PatBlt`, bitmap-less blits).
    fn fill_rect(&mut self, r: (f64, f64, f64, f64), color: Option<u32>) -> Result<(), String> {
        let mut outline = self.outline();
        outline.rect(normalized(r.0, r.1, r.2, r.3));
        if !outline.finite {
            return Ok(());
        }
        let brush = std::mem::replace(&mut self.dc.brush, Brush { color });
        let result = self.paint(&outline.d, true, false, 0.0);
        self.dc.brush = brush;
        result
    }

    // -- path brackets -----------------------------------------------------------

    fn begin_path(&mut self) {
        self.path = Some(Recording {
            d: String::new(),
            open: false,
            finite: true,
        });
        self.recording = true;
    }
    fn end_path(&mut self) {
        self.recording = false;
    }
    fn close_figure(&mut self) {
        if let (true, Some(path)) = (self.recording, &mut self.path)
            && !path.d.is_empty()
            && !path.d.ends_with('Z')
        {
            path.d.push('Z');
            path.open = false;
        }
    }
    fn abort_path(&mut self) {
        self.path = None;
        self.recording = false;
    }
    /// `FillPath`, `StrokePath`, `StrokeAndFillPath`: draw the closed path and discard it.
    fn draw_path(&mut self, fill: bool, stroke: bool) -> Result<(), String> {
        if self.recording {
            return Ok(());
        }
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        if !path.finite || path.d.is_empty() {
            return Ok(());
        }
        // Filling closes open figures implicitly, as SVG fills open subpaths.
        let width = self.stroke_width(&self.matrix());
        self.paint(&path.d, fill, stroke, width)
    }
    /// `SelectClipPath`: combine the path with the clip region and discard the path.
    fn clip_path(&mut self, mode: u32) -> Result<(), String> {
        if self.recording {
            return Ok(());
        }
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        if path.finite {
            let rule = if self.dc.even_odd {
                "evenodd"
            } else {
                "nonzero"
            };
            self.combine_clip(mode, &path.d, rule)?;
        }
        Ok(())
    }

    // -- clipping ------------------------------------------------------------------

    /// Combine an area in output units with the clip region (`RegionMode`, MS-EMF 2.1.29).
    fn combine_clip(&mut self, mode: u32, d: &str, rule: &str) -> Result<(), String> {
        match mode {
            // RGN_COPY replaces the region; RGN_AND intersects with it.
            5 | 1 => {
                if mode == 5 {
                    self.dc.clip = Clip::default();
                }
                self.intersect(d, rule)
            }
            // RGN_DIFF removes the area.
            4 => self.exclude(d, rule),
            _ => {
                let name = if mode == 2 { "RGN_OR" } else { "RGN_XOR" };
                self.note(
                    Code::Approximation,
                    format!("Metafile clip mode {name} is not applied; the previous clip is kept"),
                );
                Ok(())
            }
        }
    }
    fn intersect(&mut self, d: &str, rule: &str) -> Result<(), String> {
        self.element()?;
        let id = self.id();
        let parent = self
            .dc
            .clip
            .intersect
            .as_ref()
            .map(|p| format!(" clip-path=\"url(#{p})\""))
            .unwrap_or_default();
        let _ = write!(
            self.defs,
            "<clipPath id=\"{id}\"{parent}><path d=\"{d}\" clip-rule=\"{rule}\"/></clipPath>"
        );
        self.dc.clip.intersect = Some(id);
        Ok(())
    }
    fn exclude(&mut self, d: &str, rule: &str) -> Result<(), String> {
        self.element()?;
        let id = self.id();
        self.dc.clip.excludes.push(format!(
            "<path d=\"{d}\" fill=\"black\" fill-rule=\"{rule}\"/>"
        ));
        // The mask spans three viewports per axis; nothing outside the viewport is shown.
        let (x, y, w, h) = self.view;
        let (x, y, w, h) = (x - w, y - h, w * 3.0, h * 3.0);
        let _ = write!(
            self.defs,
            "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"><rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"white\"/>{}</mask>",
            self.dc.clip.excludes.concat()
        );
        self.dc.clip.mask = Some(id);
        Ok(())
    }
    /// `IntersectClipRect` (`exclude == false`) or `ExcludeClipRect` in logical units.
    fn clip_rect(&mut self, r: (f64, f64, f64, f64), exclude: bool) -> Result<(), String> {
        let mut outline = self.outline();
        outline.rect(normalized(r.0, r.1, r.2, r.3));
        if !outline.finite {
            return Ok(());
        }
        if exclude {
            self.exclude(&outline.d, "nonzero")
        } else {
            self.intersect(&outline.d, "nonzero")
        }
    }
    fn reset_clip(&mut self) {
        self.dc.clip = Clip::default();
    }

    // -- bitmaps ---------------------------------------------------------------------

    /// Draw `src` (`x, y, width, height` in pixels, top-down) of a DIB stretched to the
    /// logical rectangle `dest` (`x, y, width, height`; negative sizes mirror).
    fn bitmap(
        &mut self,
        dest: (f64, f64, f64, f64),
        src: (i64, i64, i64, i64),
        image: &dib::Dib<'_>,
        alpha: dib::Alpha,
    ) -> Result<(), String> {
        let (sx, sy, sw, sh) = src;
        let x0 = sx.clamp(0, image.width as i64);
        let y0 = sy.clamp(0, image.height as i64);
        let x1 = (sx + sw).clamp(0, image.width as i64);
        let y1 = (sy + sh).clamp(0, image.height as i64);
        if x1 <= x0 || y1 <= y0 {
            return Ok(());
        }
        let (x0, y0, w, h) = (
            x0 as usize,
            y0 as usize,
            (x1 - x0) as usize,
            (y1 - y0) as usize,
        );
        self.pixels = self.pixels.saturating_add(w.saturating_mul(h));
        if self.pixels > MAX_BITMAP_PIXELS {
            return Err(format!(
                "Metafile picture exceeds {MAX_BITMAP_PIXELS} bitmap pixels; not rendered"
            ));
        }
        let (png, downscaled) =
            raster::encode_rows(w, h, |y, row| image.row(y0 + y, x0, row, alpha))?;
        if let Some((tw, th)) = downscaled {
            self.note(
                Code::Approximation,
                format!("Downscaled oversized image from {w}x{h} to {tw}x{th} px"),
            );
        }
        let matrix = self.matrix();
        let (x, y, dw, dh) = dest;
        let p0 = matrix.apply((x, y));
        let p1 = matrix.apply((x + dw, y));
        let p2 = matrix.apply((x, y + dh));
        let values = [
            p1.0 - p0.0,
            p1.1 - p0.1,
            p2.0 - p0.0,
            p2.1 - p0.1,
            p0.0,
            p0.1,
        ];
        if values.iter().any(|v| !v.is_finite()) {
            return Ok(());
        }
        self.element()?;
        let values: Vec<_> = values.iter().map(|v| v.to_string()).collect();
        // The clip is set on a group: on the image it would be read in the image's
        // transformed user space.
        let _ = write!(
            self.body,
            "<g{}><image width=\"1\" height=\"1\" preserveAspectRatio=\"none\" transform=\"matrix({})\" href=\"data:image/png;base64,{}\"/></g>",
            self.clip_attributes(),
            values.join(" "),
            STANDARD.encode(png)
        );
        Ok(())
    }
    /// A bitmap record whose DIB could not be used: report it and draw the rest.
    fn bitmap_failed(&mut self, name: &str, reason: dib::Error) {
        match reason {
            dib::Error::Unsupported(what) => self.note(
                Code::Unsupported,
                format!("Unsupported bitmap in metafile record ignored ({name}: {what})"),
            ),
            dib::Error::Malformed(what) => self.note(
                Code::Omitted,
                format!("Malformed bitmap in metafile record omitted ({name}: {what})"),
            ),
        }
    }

    /// Raster operation of a blit: draw the source, fill, or approximate.
    fn blit(
        &mut self,
        name: &str,
        rop: u32,
        dest: (f64, f64, f64, f64),
        source: Option<Source<'_>>,
    ) -> Result<(), String> {
        let (x, y, w, h) = dest;
        let rect = (x, y, x + w, y + h);
        match (rop >> 16) & 0xFF {
            // BLACKNESS, WHITENESS, PATCOPY: no source is read.
            0x00 => return self.fill_rect(rect, Some(0x000000)),
            0xFF => return self.fill_rect(rect, Some(0xFFFFFF)),
            0xF0 => return self.fill_rect(rect, self.dc.brush.color),
            // SRCCOPY draws the source as it is.
            0xCC => {}
            _ => self.note(
                Code::Approximation,
                format!("Metafile raster operation 0x{rop:08X} is drawn as a source copy ({name})"),
            ),
        }
        let Some(source) = source else {
            return Ok(());
        };
        // The DIB functions (`StretchDIBits`) measure the source rectangle of a bottom-up
        // DIB from its lower-left corner; blits from a bitmap do not.
        let image = &source.image;
        let (x, y, w, h) = source.rect;
        let (x, y, w, h) = (x as i64, y as i64, w as i64, h as i64);
        let rect = if source.dib_origin && !image.top_down {
            (x, image.height as i64 - y - h, w, h)
        } else {
            (x, y, w, h)
        };
        self.bitmap(dest, rect, image, source.alpha)
    }

    /// Wrap the drawing in an SVG document whose viewport is `view` and natural size `size`.
    fn finish_document(self, size: (f64, f64)) -> Translated {
        let (x, y, w, h) = self.view;
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{x} {y} {w} {h}\" preserveAspectRatio=\"none\"><defs>{}</defs>{}</svg>",
            size.0, size.1, self.defs, self.body
        );
        Translated {
            svg: svg.into_bytes(),
            size,
            notes: self.notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_composition_applies_the_first_map_first() {
        let scale = Affine([2.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
        let shift = Affine([1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
        assert_eq!(scale.then(&shift).apply((1.0, 1.0)), (12.0, 23.0));
        assert_eq!(shift.then(&scale).apply((1.0, 1.0)), (22.0, 63.0));
        assert_eq!(scale.scale(), 6f64.sqrt());
    }
    #[test]
    fn color_ref_is_little_endian_bgr() {
        assert_eq!(color_ref(0x0000_00FF), 0xFF0000);
        assert_eq!(color_ref(0x00FF_0000), 0x0000FF);
    }
    fn fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/fixtures/metafile/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).unwrap()
    }
    fn svg(translated: &Translated) -> &str {
        std::str::from_utf8(&translated.svg).unwrap()
    }
    #[test]
    fn shapes_become_paths_in_reference_device_pixels() {
        let translated = translate(&fixture("emf-shapes.emf")).unwrap();
        let svg = svg(&translated);
        // The 52.92 mm frame on a 1920 x 1080 px, 508 x 286 mm reference device.
        assert!(
            svg.contains(r#"viewBox="0 0 200.01259842519684 199.83776223776223""#),
            "{svg}"
        );
        // Blue POLYGON16 triangle, filled and outlined with the 3-unit black pen.
        assert!(svg.contains(
            r##"<path d="M50 80L90 150L10 150Z" fill="#1e3cdc" fill-rule="evenodd" stroke="#000000" stroke-width="3""##
        ), "{svg}");
        // POLYLINE16 zig-zag with the hollow brush: stroked only.
        assert!(
            svg.contains(
                r##"<path d="M110 150L130 90L150 150L170 90L190 150" fill="none" stroke="#000000""##
            ),
            "{svg}"
        );
        assert_eq!(svg.matches("<path ").count(), 8, "{svg}");
        assert!(translated.notes.is_empty());
        let (w, h) = translated.size;
        assert!(
            (w - 200.0).abs() < 0.02 && (h - 200.0).abs() < 0.02,
            "{w} {h}"
        );
        assert!(!super::super::vector::check_generated(&translated.svg).unwrap());
    }
    #[test]
    fn exclusions_become_masks_and_intersections_chain_clip_paths() {
        let translated = translate(&fixture("emf-clip.emf")).unwrap();
        let svg = svg(&translated);
        assert!(svg.contains(r#"<clipPath id="m1"><path d="M30 30L100 30L100 100L30 100Z""#));
        assert!(svg.contains(r##"<path d="M130 130L170 130L170 170L130 170Z" fill="black""##));
        assert!(svg.contains(r#"mask="url(#m2)""#), "{svg}");
    }
    #[test]
    fn emf_plus_comments_are_reported() {
        let mut data = fixture("emf-shapes.emf");
        let size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let mut comment = 70u32.to_le_bytes().to_vec();
        comment.extend(28u32.to_le_bytes());
        comment.extend(16u32.to_le_bytes());
        comment.extend(b"EMF+");
        comment.extend(0x4001u16.to_le_bytes());
        comment.extend(1u16.to_le_bytes());
        comment.extend([0; 8]);
        data.splice(size..size, comment);
        let translated = translate(&data).unwrap();
        assert_eq!(
            translated.notes,
            [(
                Code::Approximation,
                "EMF+ records are not interpreted; the picture is drawn from its EMF records"
                    .to_string()
            )]
        );
    }
    #[test]
    fn non_finite_transforms_and_bitmap_budgets_are_rejected() {
        let mut data = fixture("emf-transform.emf");
        let at = data
            .windows(4)
            .position(|w| w == 35u32.to_le_bytes())
            .unwrap();
        data[at + 8..at + 12].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(
            translate(&data).err().unwrap(),
            "Malformed EMF picture (EMR_SETWORLDTRANSFORM has a non-finite transform)"
        );
        let mut player = Player::new(Page::Emf { mm: (1.0, 1.0) }, 1.0, (0.0, 0.0, 1.0, 1.0));
        player.pixels = MAX_BITMAP_PIXELS;
        let mut info = 40u32.to_le_bytes().to_vec();
        info.extend(1i32.to_le_bytes());
        info.extend(1i32.to_le_bytes());
        info.extend(1u16.to_le_bytes());
        info.extend(24u16.to_le_bytes());
        info.extend([0; 24]);
        let image = dib::Dib::parse(&info, &[0, 0, 255, 0]).unwrap();
        let error = player
            .bitmap(
                (0.0, 0.0, 1.0, 1.0),
                (0, 0, 1, 1),
                &image,
                dib::Alpha::OPAQUE,
            )
            .unwrap_err();
        assert_eq!(
            error,
            "Metafile picture exceeds 67108864 bitmap pixels; not rendered"
        );
    }
    #[test]
    fn restore_dc_accepts_relative_and_absolute_indices() {
        let mut player = Player::new(Page::Emf { mm: (1.0, 1.0) }, 1.0, (0.0, 0.0, 1.0, 1.0));
        for x in 1..=3 {
            player.dc.position = (f64::from(x), 0.0);
            player.save().unwrap();
        }
        player.restore(-1);
        assert_eq!(player.dc.position, (3.0, 0.0));
        assert_eq!(player.saved.len(), 2);
        player.restore(1);
        assert_eq!(player.dc.position, (1.0, 0.0));
        assert!(player.saved.is_empty());
        player.restore(-1);
        assert_eq!(player.dc.position, (1.0, 0.0));
    }
}
