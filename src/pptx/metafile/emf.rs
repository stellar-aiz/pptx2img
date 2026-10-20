//! EMF records (MS-EMF 2.3) played into the shared `Player`.
use super::*;
use std::{borrow::Cow, collections::HashMap};

/// `EMR_HEADER` (type 1) with the `" EMF"` signature at offset 40.
pub(super) fn sniff(data: &[u8]) -> bool {
    Bytes(data).u32(0) == Some(1) && data.get(40..44) == Some(b" EMF")
}

fn malformed(what: impl std::fmt::Display) -> String {
    super::malformed("EMF", what)
}

pub(super) fn translate(data: &[u8]) -> Result<Translated, String> {
    let file = Bytes(data);
    let short = || malformed("truncated header");
    let header_size = file.u32(4).ok_or_else(short)? as usize;
    if header_size < 88 || !header_size.is_multiple_of(4) || header_size > data.len() {
        return Err(malformed("invalid header size"));
    }
    let rect = |at: usize| -> Option<[f64; 4]> {
        Some([
            f64::from(file.i32(at)?),
            f64::from(file.i32(at + 4)?),
            f64::from(file.i32(at + 8)?),
            f64::from(file.i32(at + 12)?),
        ])
    };
    let bounds = rect(8).ok_or_else(short)?;
    let frame = rect(24).ok_or_else(short)?;
    let device = (
        f64::from(file.i32(72).ok_or_else(short)?),
        f64::from(file.i32(76).ok_or_else(short)?),
    );
    // `szlMillimeters` is the reference size; the optional `szlMicrometers` (108-byte
    // header) is not used, as writers do not keep it consistent with the device size.
    let millimetres = (
        f64::from(file.i32(80).ok_or_else(short)?),
        f64::from(file.i32(84).ok_or_else(short)?),
    );
    if device.0 <= 0.0 || device.1 <= 0.0 || millimetres.0 <= 0.0 || millimetres.1 <= 0.0 {
        return Err(malformed("invalid reference device size"));
    }
    let mm = (device.0 / millimetres.0, device.1 / millimetres.1);
    // The picture is the frame (0.01 mm) on the reference device. A writer that leaves
    // the frame empty still sets the bounds (device pixels, inclusive).
    let [fl, ft, fr, fb] = frame;
    let (view, size) = if fr > fl && fb > ft {
        let view = (
            fl / 100.0 * mm.0,
            ft / 100.0 * mm.1,
            (fr - fl) / 100.0 * mm.0,
            (fb - ft) / 100.0 * mm.1,
        );
        let px = |hundredths: f64| hundredths / 100.0 / 25.4 * 96.0;
        (view, (px(fr - fl), px(fb - ft)))
    } else {
        let [bl, bt, br, bb] = bounds;
        let (w, h) = (br - bl + 1.0, bb - bt + 1.0);
        if w <= 0.0 || h <= 0.0 {
            return Err(malformed("empty frame and bounds"));
        }
        let px = |v: f64, per_mm: f64| v / per_mm / 25.4 * 96.0;
        ((bl, bt, w, h), (px(w, mm.0), px(h, mm.1)))
    };
    let hairline = mm.0 * 25.4 / 96.0;
    let mut state = Emf {
        player: Player::new(Page::Emf { mm }, hairline, view),
        objects: HashMap::new(),
        plus: None,
    };
    let mut offset = header_size;
    loop {
        let kind = file
            .u32(offset)
            .ok_or_else(|| malformed("ends without EMR_EOF"))?;
        let length = file
            .u32(offset + 4)
            .ok_or_else(|| malformed("ends without EMR_EOF"))? as usize;
        if length < 8 || !length.is_multiple_of(4) || length > data.len() - offset {
            return Err(malformed(format!(
                "record at byte {offset} has an invalid size"
            )));
        }
        state.player.count_record()?;
        if kind == 14 {
            break;
        }
        state.play(kind, Bytes(&data[offset..offset + length]))?;
        offset += length;
    }
    match state.plus {
        Some(true) => state.player.note(
            Code::Approximation,
            "EMF+ records are not interpreted; the picture is drawn from its EMF records".into(),
        ),
        Some(false) => state.player.note(
            Code::Unsupported,
            "EMF+ records are not interpreted; this EMF+ only picture has no EMF drawing".into(),
        ),
        None => {}
    }
    Ok(state.player.finish_document(size))
}

struct Emf {
    player: Player,
    objects: HashMap<u32, Object>,
    /// `Some(dual)` once an EMF+ header comment is seen.
    plus: Option<bool>,
}

impl Emf {
    fn play(&mut self, kind: u32, record: Bytes<'_>) -> Result<(), String> {
        let name = &name(kind);
        let truncated = || malformed(format!("{name} is truncated"));
        let u32_at = |at| record.u32(at).ok_or_else(truncated);
        let i32_at = |at| record.i32(at).map(f64::from).ok_or_else(truncated);
        let point = |at| -> Result<(f64, f64), String> { Ok((i32_at(at)?, i32_at(at + 4)?)) };
        let rect = |at| -> Result<(f64, f64, f64, f64), String> {
            Ok((
                i32_at(at)?,
                i32_at(at + 4)?,
                i32_at(at + 8)?,
                i32_at(at + 12)?,
            ))
        };
        let p = &mut self.player;
        match kind {
            // Poly records: Bounds, Count, then 32-bit (2..6) or 16-bit (85..89) points.
            2..=6 | 85..=89 => {
                let wide = kind <= 6;
                let count = u32_at(24)? as usize;
                let points = p.points(record, 28, count, wide, truncated)?;
                match if wide { kind } else { kind - 83 } {
                    2 => p.bezier(&points)?,
                    3 => p.polygon(&points)?,
                    4 => p.polyline(&points)?,
                    5 => p.continue_to(&points, true)?,
                    _ => p.continue_to(&points, false)?,
                }
            }
            7 | 8 | 90 | 91 => {
                let wide = kind <= 8;
                let polygons = u32_at(24)? as usize;
                let total = u32_at(28)? as usize;
                if polygons > record.0.len() / 4 {
                    return Err(truncated());
                }
                let mut counts = Vec::with_capacity(polygons);
                for i in 0..polygons {
                    counts.push(u32_at(32 + i * 4)? as usize);
                }
                if counts.iter().try_fold(0usize, |a, &c| a.checked_add(c)) != Some(total) {
                    return Err(malformed(format!("{name} point counts do not add up")));
                }
                let all = p.points(record, 32 + polygons * 4, total, wide, truncated)?;
                let figures = figures(&all, &counts);
                if kind == 8 || kind == 91 {
                    p.poly_polygon(&figures)?;
                } else {
                    p.poly_polyline(&figures)?;
                }
            }
            9 => p.dc.window_ext = point(8)?,
            10 => p.dc.window_org = point(8)?,
            11 => p.dc.viewport_ext = point(8)?,
            12 => p.dc.viewport_org = point(8)?,
            31 | 32 => {
                let (xn, xd, yn, yd) = rect(8)?;
                if xd != 0.0 && yd != 0.0 {
                    let ext = if kind == 31 {
                        &mut p.dc.viewport_ext
                    } else {
                        &mut p.dc.window_ext
                    };
                    *ext = (ext.0 * xn / xd, ext.1 * yn / yd);
                }
            }
            17 => p.dc.map_mode = u32_at(8)?,
            19 => p.dc.even_odd = u32_at(8)? != 2,
            20 => p.mix_mode(u32_at(8)?, name),
            27 => p.move_to(point(8)?),
            54 => {
                let to = point(8)?;
                p.continue_to(&[to], false)?;
            }
            29 => p.clip_rect(rect(8)?, true)?,
            30 => p.clip_rect(rect(8)?, false)?,
            33 => p.save()?,
            34 => p.restore(i32_at(8)? as i32),
            35 | 36 => {
                let mut values = [0.0; 6];
                for (i, value) in values.iter_mut().enumerate() {
                    *value = f64::from(record.f32(8 + i * 4).ok_or_else(truncated)?);
                }
                let xform = Affine(values);
                if !xform.is_finite() {
                    return Err(malformed(format!("{name} has a non-finite transform")));
                }
                let world = &mut p.dc.world;
                *world = match if kind == 35 { 4 } else { u32_at(32)? } {
                    1 => Affine::IDENTITY,
                    2 => xform.then(world),
                    3 => world.then(&xform),
                    _ => xform,
                };
            }
            37 => {
                let handle = u32_at(8)?;
                let object = if handle & 0x8000_0000 != 0 {
                    stock(handle & 0x7FFF_FFFF)
                } else {
                    self.objects.get(&handle).cloned()
                };
                if let Some(object) = object {
                    self.player.select(object);
                }
            }
            38 => {
                let handle = u32_at(8)?;
                let style = u32_at(12)?;
                let width = i32_at(16)?.abs();
                let color = color_ref(u32_at(24)?);
                let pen = Pen::from_style(style, (width > 0.0).then_some(width), color);
                self.create(handle, Object::Pen(pen))?;
            }
            39 => {
                let handle = u32_at(8)?;
                let brush = p.brush(name, u32_at(12)?, u32_at(16)?);
                self.create(handle, Object::Brush(brush))?;
            }
            95 => {
                let handle = u32_at(8)?;
                let style = u32_at(28)?;
                let width = f64::from(u32_at(32)?);
                let geometric = style & 0x000F_0000 == 0x0001_0000;
                let width = (geometric && width > 0.0).then_some(width);
                let brush = p.brush(name, u32_at(36)?, u32_at(40)?);
                let mut pen = Pen::from_style(style, width, brush.color.unwrap_or_default());
                if brush.color.is_none() {
                    pen.color = None;
                }
                // PS_USERSTYLE: dash and gap lengths in logical units (device pixels when cosmetic).
                if style & 0x0F == 7 {
                    let entries = (u32_at(48)? as usize).min(16);
                    let unit = width.unwrap_or(1.0);
                    pen.dash = (0..entries)
                        .map(|i| u32_at(52 + i * 4).map(|v| f64::from(v) / unit))
                        .collect::<Result<_, _>>()?;
                }
                self.create(handle, Object::Pen(pen))?;
            }
            93 | 94 => {
                let handle = u32_at(8)?;
                self.player.pattern_brush(name);
                self.create(handle, Object::Brush(Brush { color: None }))?;
            }
            // Fonts, palettes, and colour spaces occupy handles but draw nothing here.
            49 | 82 | 99 | 122 => {
                let handle = u32_at(8)?;
                self.create(handle, Object::Other)?;
            }
            40 => {
                let handle = u32_at(8)?;
                self.objects.remove(&handle);
            }
            42 => p.ellipse(rect(8)?)?,
            43 => p.rectangle(rect(8)?)?,
            44 => p.round_rect(rect(8)?, point(24)?)?,
            59 => p.begin_path(),
            60 => p.end_path(),
            61 => p.close_figure(),
            62 => p.draw_path(true, false)?,
            63 => p.draw_path(true, true)?,
            64 => p.draw_path(false, true)?,
            67 => p.clip_path(u32_at(8)?)?,
            68 => p.abort_path(),
            66 => p.note(
                Code::Unsupported,
                "Widened metafile paths are drawn unwidened (EMR_WIDENPATH)".into(),
            ),
            70 => {
                // An EMF+ header record inside the first comment marks an EMF+ picture.
                if record.get::<4>(12) == Some(*b"EMF+") && record.u16(16) == Some(0x4001) {
                    self.plus = Some(record.u16(18).unwrap_or(0) & 1 == 1);
                }
            }
            75 => {
                let size = u32_at(8)?;
                let mode = u32_at(12)?;
                if size == 0 {
                    if mode == 5 {
                        p.reset_clip();
                    }
                    return Ok(());
                }
                // RgnData: a 32-byte header (CountRects at 8), then rectangles in device units.
                let count = u32_at(24)? as usize;
                if count > record.0.len() / 16 {
                    return Err(truncated());
                }
                p.take_points(count * 4)?;
                let mut outline = Outline::new(Affine::IDENTITY);
                for i in 0..count {
                    outline.rect(rect(48 + i * 16)?);
                }
                if count == 0 {
                    // An empty region clips everything away.
                    outline.rect((0.0, 0.0, 0.0, 0.0));
                }
                p.combine_clip(mode, &outline.d, "nonzero")?;
            }
            76 | 77 | 114 => {
                let dest = rect(24)?;
                let rop = u32_at(40)?;
                let src_origin = point(44)?;
                let usage = u32_at(80)?;
                let (off_info, info_len) = (u32_at(84)?, u32_at(88)?);
                let (off_bits, bits_len) = (u32_at(92)?, u32_at(96)?);
                let src_size = if kind == 76 {
                    (dest.2, dest.3)
                } else {
                    point(100)?
                };
                let alpha = if kind == 114 {
                    let [_, _, constant, format] = rop.to_le_bytes();
                    dib::Alpha {
                        constant,
                        per_pixel: format & 1 == 1,
                    }
                } else {
                    dib::Alpha::OPAQUE
                };
                let rop = if kind == 114 { 0x00CC_0020 } else { rop };
                let bitmap = Bitmap {
                    usage,
                    info: (off_info, info_len),
                    bits: (off_bits, bits_len),
                    rect: (src_origin.0, src_origin.1, src_size.0, src_size.1),
                    dib_origin: false,
                    alpha,
                };
                bitmap.blit(p, name, record, rop, dest)?;
            }
            81 => {
                let bitmap = Bitmap {
                    usage: u32_at(64)?,
                    info: (u32_at(48)?, u32_at(52)?),
                    bits: (u32_at(56)?, u32_at(60)?),
                    rect: rect(32)?,
                    dib_origin: true,
                    alpha: dib::Alpha::OPAQUE,
                };
                let dest = (i32_at(24)?, i32_at(28)?, i32_at(72)?, i32_at(76)?);
                bitmap.blit(p, name, record, u32_at(68)?, dest)?;
            }
            83 | 84 | 96 | 97 | 108 => p.text_record(name),
            // State with no visible effect on the drawn subset.
            13
            | 16
            | 18
            | 21..=26
            | 28
            | 48
            | 50..=52
            | 57
            | 58
            | 65
            | 98
            | 100
            | 101
            | 104
            | 109..=113
            | 115
            | 119..=121 => {}
            _ => p.unsupported_record(name),
        }
        Ok(())
    }

    fn create(&mut self, handle: u32, object: Object) -> Result<(), String> {
        if self.objects.len() >= MAX_OBJECTS && !self.objects.contains_key(&handle) {
            return Err(too_many_objects());
        }
        self.objects.insert(handle, object);
        Ok(())
    }
}

/// The bitmap fields of a blit record: `DIB_*_COLORS` usage, offsets and sizes of the
/// header and bits, the source rectangle, and how it and the alpha channel are read.
struct Bitmap {
    usage: u32,
    info: (u32, u32),
    bits: (u32, u32),
    rect: (f64, f64, f64, f64),
    dib_origin: bool,
    alpha: dib::Alpha,
}
impl Bitmap {
    /// Blit onto `dest`; a record without a bitmap only fills (`PatBlt`-like operations),
    /// and an unusable bitmap is reported and skipped.
    fn blit(
        self,
        player: &mut Player,
        name: &str,
        record: Bytes<'_>,
        rop: u32,
        dest: (f64, f64, f64, f64),
    ) -> Result<(), String> {
        if self.info.1 == 0 {
            return player.blit(name, rop, dest, None);
        }
        let info = record
            .slice(self.info.0, self.info.1)
            .ok_or_else(|| malformed(format!("{name} bitmap header lies outside the record")))?;
        let bits = record
            .slice(self.bits.0, self.bits.1)
            .ok_or_else(|| malformed(format!("{name} bitmap data lies outside the record")))?;
        let image = if self.usage != 0 {
            Err(dib::Error::Unsupported("palette-index colours".into()))
        } else {
            dib::Dib::parse(info, bits)
        };
        match image {
            Ok(image) => {
                let source = Source {
                    image,
                    rect: self.rect,
                    dib_origin: self.dib_origin,
                    alpha: self.alpha,
                };
                player.blit(name, rop, dest, Some(source))
            }
            Err(reason) => {
                player.bitmap_failed(name, reason);
                Ok(())
            }
        }
    }
}

/// The MS-EMF name of record type `kind` (2.1.1), or its number when unknown.
fn name(kind: u32) -> Cow<'static, str> {
    const NAMES: [&str; 123] = [
        "",
        "EMR_HEADER",
        "EMR_POLYBEZIER",
        "EMR_POLYGON",
        "EMR_POLYLINE",
        "EMR_POLYBEZIERTO",
        "EMR_POLYLINETO",
        "EMR_POLYPOLYLINE",
        "EMR_POLYPOLYGON",
        "EMR_SETWINDOWEXTEX",
        "EMR_SETWINDOWORGEX",
        "EMR_SETVIEWPORTEXTEX",
        "EMR_SETVIEWPORTORGEX",
        "EMR_SETBRUSHORGEX",
        "EMR_EOF",
        "EMR_SETPIXELV",
        "EMR_SETMAPPERFLAGS",
        "EMR_SETMAPMODE",
        "EMR_SETBKMODE",
        "EMR_SETPOLYFILLMODE",
        "EMR_SETROP2",
        "EMR_SETSTRETCHBLTMODE",
        "EMR_SETTEXTALIGN",
        "EMR_SETCOLORADJUSTMENT",
        "EMR_SETTEXTCOLOR",
        "EMR_SETBKCOLOR",
        "EMR_OFFSETCLIPRGN",
        "EMR_MOVETOEX",
        "EMR_SETMETARGN",
        "EMR_EXCLUDECLIPRECT",
        "EMR_INTERSECTCLIPRECT",
        "EMR_SCALEVIEWPORTEXTEX",
        "EMR_SCALEWINDOWEXTEX",
        "EMR_SAVEDC",
        "EMR_RESTOREDC",
        "EMR_SETWORLDTRANSFORM",
        "EMR_MODIFYWORLDTRANSFORM",
        "EMR_SELECTOBJECT",
        "EMR_CREATEPEN",
        "EMR_CREATEBRUSHINDIRECT",
        "EMR_DELETEOBJECT",
        "EMR_ANGLEARC",
        "EMR_ELLIPSE",
        "EMR_RECTANGLE",
        "EMR_ROUNDRECT",
        "EMR_ARC",
        "EMR_CHORD",
        "EMR_PIE",
        "EMR_SELECTPALETTE",
        "EMR_CREATEPALETTE",
        "EMR_SETPALETTEENTRIES",
        "EMR_RESIZEPALETTE",
        "EMR_REALIZEPALETTE",
        "EMR_EXTFLOODFILL",
        "EMR_LINETO",
        "EMR_ARCTO",
        "EMR_POLYDRAW",
        "EMR_SETARCDIRECTION",
        "EMR_SETMITERLIMIT",
        "EMR_BEGINPATH",
        "EMR_ENDPATH",
        "EMR_CLOSEFIGURE",
        "EMR_FILLPATH",
        "EMR_STROKEANDFILLPATH",
        "EMR_STROKEPATH",
        "EMR_FLATTENPATH",
        "EMR_WIDENPATH",
        "EMR_SELECTCLIPPATH",
        "EMR_ABORTPATH",
        "",
        "EMR_COMMENT",
        "EMR_FILLRGN",
        "EMR_FRAMERGN",
        "EMR_INVERTRGN",
        "EMR_PAINTRGN",
        "EMR_EXTSELECTCLIPRGN",
        "EMR_BITBLT",
        "EMR_STRETCHBLT",
        "EMR_MASKBLT",
        "EMR_PLGBLT",
        "EMR_SETDIBITSTODEVICE",
        "EMR_STRETCHDIBITS",
        "EMR_EXTCREATEFONTINDIRECTW",
        "EMR_EXTTEXTOUTA",
        "EMR_EXTTEXTOUTW",
        "EMR_POLYBEZIER16",
        "EMR_POLYGON16",
        "EMR_POLYLINE16",
        "EMR_POLYBEZIERTO16",
        "EMR_POLYLINETO16",
        "EMR_POLYPOLYLINE16",
        "EMR_POLYPOLYGON16",
        "EMR_POLYDRAW16",
        "EMR_CREATEMONOBRUSH",
        "EMR_CREATEDIBPATTERNBRUSHPT",
        "EMR_EXTCREATEPEN",
        "EMR_POLYTEXTOUTA",
        "EMR_POLYTEXTOUTW",
        "EMR_SETICMMODE",
        "EMR_CREATECOLORSPACE",
        "EMR_SETCOLORSPACE",
        "EMR_DELETECOLORSPACE",
        "EMR_GLSRECORD",
        "EMR_GLSBOUNDEDRECORD",
        "EMR_PIXELFORMAT",
        "EMR_DRAWESCAPE",
        "EMR_EXTESCAPE",
        "",
        "EMR_SMALLTEXTOUT",
        "EMR_FORCEUFIMAPPING",
        "EMR_NAMEDESCAPE",
        "EMR_COLORCORRECTPALETTE",
        "EMR_SETICMPROFILEA",
        "EMR_SETICMPROFILEW",
        "EMR_ALPHABLEND",
        "EMR_SETLAYOUT",
        "EMR_TRANSPARENTBLT",
        "",
        "EMR_GRADIENTFILL",
        "EMR_SETLINKEDUFIS",
        "EMR_SETTEXTJUSTIFICATION",
        "EMR_COLORMATCHTOTARGETW",
        "EMR_CREATECOLORSPACEW",
    ];
    match NAMES.get(kind as usize) {
        Some(name) if !name.is_empty() => Cow::Borrowed(name),
        _ => Cow::Owned(format!("EMF record type {kind}")),
    }
}
