//! Placeable WMF records (MS-WMF 2.3) played into the shared `Player`.
//!
//! The placeable header's bounding box is the picture; the window (`SetWindowOrg`,
//! `SetWindowExt`) maps onto it, starting as the box itself. Viewport records only
//! describe the playback device, so they do not move the picture.
use super::*;

/// The placeable-header key `0x9AC6CDD7`.
pub(super) fn sniff(data: &[u8]) -> bool {
    Bytes(data).u32(0) == Some(0x9AC6_CDD7)
}

fn malformed(what: impl std::fmt::Display) -> String {
    format!("Malformed WMF picture ({what})")
}

pub(super) fn translate(data: &[u8]) -> Result<Translated, String> {
    let file = Bytes(data);
    let short = || malformed("truncated header");
    let coordinate = |at| file.i16(at).map(f64::from).ok_or_else(short);
    let (l, t, r, b) = (
        coordinate(6)?,
        coordinate(8)?,
        coordinate(10)?,
        coordinate(12)?,
    );
    let (l, t, r, b) = normalized(l, t, r, b);
    if r <= l || b <= t {
        return Err(malformed("empty bounding box"));
    }
    let inch = match file.u16(14).ok_or_else(short)? {
        0 => 1440.0,
        n => f64::from(n),
    };
    let header_words = usize::from(file.u16(24).ok_or_else(short)?);
    if !matches!(file.u16(22), Some(1 | 2)) || header_words < 9 {
        return Err(malformed("invalid META_HEADER"));
    }
    let view = (l, t, r - l, b - t);
    let size = ((r - l) / inch * 96.0, (b - t) / inch * 96.0);
    let mut player = Player::new(Page::Wmf { bbox: view }, inch / 96.0, view);
    player.dc.window_org = (l, t);
    player.dc.window_ext = (r - l, b - t);
    let mut state = Wmf {
        player,
        objects: Vec::new(),
    };
    let mut offset = 22 + header_words * 2;
    let mut records = 0;
    loop {
        let ended = || malformed("ends without META_EOF");
        let words = file.u32(offset).ok_or_else(ended)? as usize;
        let function = file.u16(offset + 4).ok_or_else(ended)?;
        let length = words.checked_mul(2).filter(|&n| n >= 6);
        let Some(length) = length.filter(|&n| n <= data.len() - offset) else {
            return Err(malformed(format!(
                "record at byte {offset} has an invalid size"
            )));
        };
        records += 1;
        if records > MAX_RECORDS {
            return Err(format!(
                "Metafile picture exceeds {MAX_RECORDS} records; not rendered"
            ));
        }
        if function == 0 {
            break;
        }
        state.play(function, Bytes(&data[offset + 6..offset + length]))?;
        offset += length;
    }
    Ok(state.player.finish_document(size))
}

struct Wmf {
    player: Player,
    /// The object table: a new object takes the lowest free index (MS-WMF 3.1.4.1).
    objects: Vec<Option<Object>>,
}

impl Wmf {
    fn play(&mut self, function: u16, params: Bytes<'_>) -> Result<(), String> {
        let name = &name(function);
        let truncated = || malformed(format!("{name} is truncated"));
        let int = |at| params.i16(at).map(f64::from).ok_or_else(truncated);
        let word = |at| params.u16(at).ok_or_else(truncated);
        let dword = |at| params.u32(at).ok_or_else(truncated);
        // Rectangles are stored bottom, right, top, left; points y before x.
        let rect = |at: usize| -> Result<_, String> {
            Ok((int(at + 6)?, int(at + 4)?, int(at + 2)?, int(at)?))
        };
        let point = |at: usize| -> Result<_, String> { Ok((int(at + 2)?, int(at)?)) };
        let p = &mut self.player;
        match function {
            0x020B => p.dc.window_org = point(0)?,
            0x020C => p.dc.window_ext = point(0)?,
            0x020F => {
                let (dx, dy) = point(0)?;
                p.dc.window_org = (p.dc.window_org.0 + dx, p.dc.window_org.1 + dy);
            }
            0x0410 => {
                let (y_denom, y_num, x_denom, x_num) = (int(0)?, int(2)?, int(4)?, int(6)?);
                if x_denom != 0.0 && y_denom != 0.0 {
                    let ext = &mut p.dc.window_ext;
                    *ext = (ext.0 * x_num / x_denom, ext.1 * y_num / y_denom);
                }
            }
            0x0103 => p.dc.map_mode = u32::from(word(0)?),
            0x0106 => p.dc.even_odd = word(0)? != 2,
            0x0104 => {
                let rop = word(0)?;
                if rop != 13 {
                    p.note(
                        Code::Approximation,
                        format!("Metafile mix mode {rop} is drawn as R2_COPYPEN (META_SETROP2)"),
                    );
                }
            }
            0x02FA => {
                let style = u32::from(word(0)?);
                let width = int(2)?.abs();
                let color = color_ref(dword(6)?);
                let pen = Pen::from_style(style, (width > 0.0).then_some(width), color);
                self.create(Object::Pen(pen))?;
            }
            0x02FC => {
                let brush = p.brush(name, u32::from(word(0)?), dword(2)?);
                self.create(Object::Brush(brush))?;
            }
            0x0142 | 0x01F9 => {
                p.note(
                    Code::Unsupported,
                    format!("Pattern brushes in metafile pictures are not drawn ({name})"),
                );
                self.create(Object::Brush(Brush { color: None }))?;
            }
            0x02FB | 0x00F7 | 0x06FF => self.create(Object::Other)?,
            0x012D => {
                let index = usize::from(word(0)?);
                if let Some(Some(object)) = self.objects.get(index) {
                    let object = object.clone();
                    self.player.select(object);
                }
            }
            0x01F0 => {
                let index = usize::from(word(0)?);
                if let Some(slot) = self.objects.get_mut(index) {
                    *slot = None;
                }
            }
            0x041B => p.rectangle(rect(0)?)?,
            0x0418 => p.ellipse(rect(0)?)?,
            0x061C => {
                let corner = (int(2)?, int(0)?);
                p.round_rect(rect(4)?, corner)?;
            }
            0x0324 | 0x0325 => {
                let count = usize::from(word(0)?);
                let points = points(p, params, 2, count).ok_or_else(truncated)??;
                if function == 0x0324 {
                    p.polygon(&points)?;
                } else {
                    p.polyline(&points)?;
                }
            }
            0x0538 => {
                let polygons = usize::from(word(0)?);
                let mut counts = Vec::with_capacity(polygons);
                for i in 0..polygons {
                    counts.push(usize::from(word(2 + i * 2)?));
                }
                let total = counts.iter().sum();
                let all = points(p, params, 2 + polygons * 2, total).ok_or_else(truncated)??;
                let mut rest = all.as_slice();
                let mut figures = Vec::with_capacity(polygons);
                for count in counts {
                    let (figure, tail) = rest.split_at(count);
                    figures.push(figure.to_vec());
                    rest = tail;
                }
                p.poly_polygon(&figures)?;
            }
            0x0214 => p.move_to(point(0)?),
            0x0213 => {
                let to = point(0)?;
                p.continue_to(&[to], false)?;
            }
            0x0416 => p.clip_rect(rect(0)?, false)?,
            0x0415 => p.clip_rect(rect(0)?, true)?,
            0x001E => p.save()?,
            0x0127 => p.restore(i32::from(params.i16(0).ok_or_else(truncated)?)),
            0x061D => {
                let rop = dword(0)?;
                let (h, w, y, x) = (int(4)?, int(6)?, int(8)?, int(10)?);
                p.blit(name, rop, (x, y, w, h), None)?;
            }
            0x0F43 => {
                let rop = dword(0)?;
                let usage = word(4)?;
                let (sh, sw, sy, sx) = (int(6)?, int(8)?, int(10)?, int(12)?);
                let (dh, dw, dy, dx) = (int(14)?, int(16)?, int(18)?, int(20)?);
                let dib = params.0.get(22..).ok_or_else(truncated)?;
                if usage != 0 {
                    p.bitmap_failed(
                        name,
                        dib::Error::Unsupported("palette-index colours".into()),
                    );
                } else {
                    packed_blit(
                        p,
                        name,
                        rop,
                        (dx, dy, dw, dh),
                        Some(dib),
                        (sx, sy, sw, sh),
                        true,
                    )?;
                }
            }
            0x0B41 | 0x0940 => {
                // Without a bitmap, a reserved word follows the source position (MS-WMF 2.3.1.3).
                let stretch = function == 0x0B41;
                let fixed = if stretch { 20 } else { 16 };
                let has_bitmap = params.0.len() > fixed + 2;
                let rop = dword(0)?;
                let reserved = if stretch { 4 } else { 2 };
                let at = |i: usize| {
                    if !has_bitmap && i >= reserved {
                        i + 1
                    } else {
                        i
                    }
                };
                let mut values = [0.0; 8];
                let used = if stretch { 8 } else { 6 };
                for (i, value) in values.iter_mut().enumerate().take(used) {
                    *value = int(4 + at(i) * 2)?;
                }
                let (dest, src) = if stretch {
                    let [sh, sw, sy, sx, dh, dw, dy, dx] = values;
                    ((dx, dy, dw, dh), (sx, sy, sw, sh))
                } else {
                    let [sy, sx, h, w, dy, dx, ..] = values;
                    ((dx, dy, w, h), (sx, sy, w, h))
                };
                let dib = if has_bitmap {
                    params.0.get(fixed..)
                } else {
                    None
                };
                packed_blit(p, name, rop, dest, dib, src, false)?;
            }
            0x0521 | 0x0A32 => p.text_record(name),
            // State with no visible effect on the drawn subset, viewport records (see the
            // module comment), and escapes.
            0x0102 | 0x0201 | 0x0209 | 0x012E | 0x0107 | 0x0105 | 0x0108 | 0x020A | 0x0231
            | 0x0234 | 0x0035 | 0x0037 | 0x0139 | 0x0436 | 0x0149 | 0x0626 | 0x020D | 0x020E
            | 0x0211 | 0x0412 => {}
            _ => p.unsupported_record(name),
        }
        Ok(())
    }

    fn create(&mut self, object: Object) -> Result<(), String> {
        if let Some(slot) = self.objects.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(object);
        } else if self.objects.len() < MAX_OBJECTS {
            self.objects.push(Some(object));
        } else {
            return Err(format!(
                "Metafile picture exceeds {MAX_OBJECTS} graphics objects; not rendered"
            ));
        }
        Ok(())
    }
}

/// A blit whose bitmap, if any, is a packed DIB.
fn packed_blit(
    player: &mut Player,
    name: &str,
    rop: u32,
    dest: (f64, f64, f64, f64),
    dib: Option<&[u8]>,
    rect: (f64, f64, f64, f64),
    dib_origin: bool,
) -> Result<(), String> {
    let source = match dib.map(dib::Dib::packed) {
        None => None,
        Some(Ok(image)) => Some(Source {
            image,
            rect,
            dib_origin,
            alpha: dib::Alpha::OPAQUE,
        }),
        Some(Err(reason)) => {
            player.bitmap_failed(name, reason);
            return Ok(());
        }
    };
    player.blit(name, rop, dest, source)
}

/// `count` 16-bit points at `at`; `None` when the record is shorter than the count states.
fn points(
    player: &mut Player,
    params: Bytes<'_>,
    at: usize,
    count: usize,
) -> Option<Result<Vec<(f64, f64)>, String>> {
    if count > params.0.len().saturating_sub(at) / 4 {
        return None;
    }
    if let Err(error) = player.take_points(count) {
        return Some(Err(error));
    }
    (0..count)
        .map(|i| {
            let at = at + i * 4;
            Some((f64::from(params.i16(at)?), f64::from(params.i16(at + 2)?)))
        })
        .collect::<Option<Vec<_>>>()
        .map(Ok)
}

/// The MS-WMF name of a record function (2.1.1.1), or its number when unknown.
fn name(function: u16) -> String {
    let name = match function {
        0x0035 => "META_REALIZEPALETTE",
        0x0037 => "META_SETPALENTRIES",
        0x0102 => "META_SETBKMODE",
        0x0103 => "META_SETMAPMODE",
        0x0104 => "META_SETROP2",
        0x0105 => "META_SETRELABS",
        0x0106 => "META_SETPOLYFILLMODE",
        0x0107 => "META_SETSTRETCHBLTMODE",
        0x0108 => "META_SETTEXTCHAREXTRA",
        0x0127 => "META_RESTOREDC",
        0x012A => "META_INVERTREGION",
        0x012B => "META_PAINTREGION",
        0x012C => "META_SELECTCLIPREGION",
        0x012D => "META_SELECTOBJECT",
        0x012E => "META_SETTEXTALIGN",
        0x0139 => "META_RESIZEPALETTE",
        0x0142 => "META_DIBCREATEPATTERNBRUSH",
        0x0149 => "META_SETLAYOUT",
        0x01F0 => "META_DELETEOBJECT",
        0x01F9 => "META_CREATEPATTERNBRUSH",
        0x0201 => "META_SETBKCOLOR",
        0x0209 => "META_SETTEXTCOLOR",
        0x020A => "META_SETTEXTJUSTIFICATION",
        0x020B => "META_SETWINDOWORG",
        0x020C => "META_SETWINDOWEXT",
        0x020D => "META_SETVIEWPORTORG",
        0x020E => "META_SETVIEWPORTEXT",
        0x020F => "META_OFFSETWINDOWORG",
        0x0211 => "META_OFFSETVIEWPORTORG",
        0x0213 => "META_LINETO",
        0x0214 => "META_MOVETO",
        0x0220 => "META_OFFSETCLIPRGN",
        0x0228 => "META_FILLREGION",
        0x0231 => "META_SETMAPPERFLAGS",
        0x0234 => "META_SELECTPALETTE",
        0x02FA => "META_CREATEPENINDIRECT",
        0x02FB => "META_CREATEFONTINDIRECT",
        0x02FC => "META_CREATEBRUSHINDIRECT",
        0x0324 => "META_POLYGON",
        0x0325 => "META_POLYLINE",
        0x0410 => "META_SCALEWINDOWEXT",
        0x0412 => "META_SCALEVIEWPORTEXT",
        0x0415 => "META_EXCLUDECLIPRECT",
        0x0416 => "META_INTERSECTCLIPRECT",
        0x0418 => "META_ELLIPSE",
        0x0419 => "META_FLOODFILL",
        0x041B => "META_RECTANGLE",
        0x041F => "META_SETPIXEL",
        0x0429 => "META_FRAMEREGION",
        0x0436 => "META_ANIMATEPALETTE",
        0x0521 => "META_TEXTOUT",
        0x0538 => "META_POLYPOLYGON",
        0x0548 => "META_EXTFLOODFILL",
        0x061C => "META_ROUNDRECT",
        0x061D => "META_PATBLT",
        0x0626 => "META_ESCAPE",
        0x06FF => "META_CREATEREGION",
        0x0817 => "META_ARC",
        0x081A => "META_PIE",
        0x0830 => "META_CHORD",
        0x0922 => "META_BITBLT",
        0x0940 => "META_DIBBITBLT",
        0x0A32 => "META_EXTTEXTOUT",
        0x0B23 => "META_STRETCHBLT",
        0x0B41 => "META_DIBSTRETCHBLT",
        0x0D33 => "META_SETDIBTODEV",
        0x0F43 => "META_STRETCHDIB",
        0x001E => "META_SAVEDC",
        0x00F7 => "META_CREATEPALETTE",
        _ => return format!("WMF record 0x{function:04X}"),
    };
    name.into()
}
