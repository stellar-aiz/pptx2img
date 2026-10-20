//! Device-independent bitmaps embedded in metafile records (MS-WMF 2.2.2.9), read row by
//! row so the size policy in `raster` decides before any pixel buffer is allocated.
use super::Bytes;

/// Why a bitmap is not drawn; the rest of the picture still is.
#[derive(Debug, PartialEq)]
pub(super) enum Error {
    Unsupported(String),
    Malformed(String),
}

/// How the fourth byte of a 32-bit pixel is used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Alpha {
    /// Constant opacity applied to every pixel (`AlphaBlend` `SourceConstantAlpha`).
    pub constant: u8,
    /// Whether 32-bit pixels carry premultiplied alpha (`AlphaBlend` with `AC_SRC_ALPHA`).
    pub per_pixel: bool,
}
impl Alpha {
    pub const OPAQUE: Alpha = Alpha {
        constant: 255,
        per_pixel: false,
    };
}

pub(super) struct Dib<'a> {
    pub width: usize,
    pub height: usize,
    pub top_down: bool,
    bpp: u16,
    /// RGB colours of palette entries (bit depths up to 8).
    palette: Vec<[u8; 3]>,
    /// Red, green, blue, and alpha masks of 16- and 32-bit pixels.
    masks: Option<[u32; 4]>,
    bits: &'a [u8],
    stride: usize,
}

impl<'a> Dib<'a> {
    /// Read a `BITMAPINFO` (`info`) and its pixel data (`bits`).
    pub fn parse(info: &'a [u8], bits: &'a [u8]) -> Result<Dib<'a>, Error> {
        Self::read(info, Some(bits))
    }
    /// Read a packed DIB: the `BITMAPINFO` directly followed by its pixel data (WMF).
    pub fn packed(data: &'a [u8]) -> Result<Dib<'a>, Error> {
        Self::read(data, None)
    }
    fn read(info: &'a [u8], bits: Option<&'a [u8]>) -> Result<Dib<'a>, Error> {
        let malformed = |what: &str| Error::Malformed(what.into());
        let header = Bytes(info);
        let size = header.u32(0).ok_or_else(|| malformed("missing header"))? as usize;
        let short = || malformed("truncated header");
        let (width, height, bpp, compression, colors, entry) = if size == 12 {
            // BITMAPCOREHEADER: unsigned 16-bit sizes, RGBTRIPLE palette.
            let width = i64::from(header.u16(4).ok_or_else(short)?);
            let height = i64::from(header.u16(6).ok_or_else(short)?);
            (width, height, header.u16(10).ok_or_else(short)?, 0, 0, 3)
        } else if size >= 40 {
            let width = i64::from(header.i32(4).ok_or_else(short)?);
            let height = i64::from(header.i32(8).ok_or_else(short)?);
            let bpp = header.u16(14).ok_or_else(short)?;
            let compression = header.u32(16).ok_or_else(short)?;
            (
                width,
                height,
                bpp,
                compression,
                header.u32(32).ok_or_else(short)?,
                4,
            )
        } else {
            return Err(malformed("unknown header size"));
        };
        if width <= 0 || height == 0 {
            return Err(malformed("empty bitmap"));
        }
        let (width, top_down) = (width as usize, height < 0);
        let height = height.unsigned_abs() as usize;
        let mut offset = size;
        let masks = match compression {
            0 => match bpp {
                16 => Some([0x7C00, 0x03E0, 0x001F, 0]),
                32 => Some([0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000]),
                1 | 4 | 8 | 24 => None,
                _ => return Err(Error::Unsupported(format!("{bpp}-bit pixels"))),
            },
            // BI_BITFIELDS and BI_ALPHABITFIELDS: masks in a V4/V5 header or after it.
            3 | 6 if bpp == 16 || bpp == 32 => {
                let count = if compression == 6 { 4 } else { 3 };
                let at = if size >= 52 { 40 } else { size };
                let mut masks = [0; 4];
                for (i, mask) in masks.iter_mut().enumerate().take(count) {
                    *mask = header.u32(at + i * 4).ok_or_else(short)?;
                }
                if size >= 56 {
                    masks[3] = header.u32(52).ok_or_else(short)?;
                }
                if size < 52 {
                    offset += count * 4;
                }
                Some(masks)
            }
            1 | 2 => return Err(Error::Unsupported("RLE compression".into())),
            4 => return Err(Error::Unsupported("JPEG compression".into())),
            5 => return Err(Error::Unsupported("PNG compression".into())),
            other => return Err(Error::Unsupported(format!("compression {other}"))),
        };
        let mut palette = Vec::new();
        if bpp <= 8 {
            let full = 1usize << bpp;
            let count = match colors as usize {
                0 => full,
                n => n.min(full),
            };
            for i in 0..count {
                let at = offset + i * entry;
                let [b, g, r] = header.get::<3>(at).ok_or_else(short)?;
                palette.push([r, g, b]);
            }
        }
        let bits = match bits {
            Some(bits) => bits,
            None => {
                // Deeper bitmaps may still carry an optimisation palette of `colors` entries.
                let entries = if bpp <= 8 {
                    palette.len()
                } else {
                    colors as usize
                };
                entries
                    .checked_mul(entry)
                    .and_then(|n| info.get(offset.checked_add(n)?..))
                    .ok_or_else(|| malformed("pixel data is missing"))?
            }
        };
        let stride = width
            .checked_mul(usize::from(bpp))
            .and_then(|b| b.checked_add(31))
            .map(|b| b / 32 * 4)
            .ok_or_else(|| malformed("row size overflows"))?;
        // The last row needs only its pixels, not its padding.
        let last = (width * usize::from(bpp)).div_ceil(8);
        let needed = stride
            .checked_mul(height - 1)
            .and_then(|n| n.checked_add(last))
            .ok_or_else(|| malformed("bitmap size overflows"))?;
        if bits.len() < needed {
            return Err(malformed("pixel data is shorter than the header states"));
        }
        Ok(Dib {
            width,
            height,
            top_down,
            bpp,
            palette,
            masks,
            bits,
            stride,
        })
    }

    /// Write `out.len() / 4` RGBA pixels of top-down row `y`, starting at column `x0`.
    pub fn row(&self, y: usize, x0: usize, out: &mut [u8], alpha: Alpha) -> anyhow::Result<()> {
        let index = if self.top_down {
            y
        } else {
            self.height - 1 - y
        };
        let row = &self.bits[index * self.stride..];
        for (i, pixel) in out.chunks_exact_mut(4).enumerate() {
            let x = x0 + i;
            let [r, g, b, a] = self.pixel(row, x, alpha);
            pixel.copy_from_slice(&[r, g, b, a]);
        }
        Ok(())
    }

    fn pixel(&self, row: &[u8], x: usize, alpha: Alpha) -> [u8; 4] {
        let byte = |at: usize| row.get(at).copied().unwrap_or(0);
        let indexed = |index: usize| {
            let [r, g, b] = self.palette.get(index).copied().unwrap_or([0, 0, 0]);
            [r, g, b, alpha.constant]
        };
        match self.bpp {
            1 => indexed(usize::from(byte(x / 8) >> (7 - x % 8) & 1)),
            4 => indexed(usize::from(
                byte(x / 2) >> (if x.is_multiple_of(2) { 4 } else { 0 }) & 0x0F,
            )),
            8 => indexed(usize::from(byte(x))),
            24 => [
                byte(x * 3 + 2),
                byte(x * 3 + 1),
                byte(x * 3),
                alpha.constant,
            ],
            _ => {
                let value = if self.bpp == 16 {
                    u32::from(u16::from_le_bytes([byte(x * 2), byte(x * 2 + 1)]))
                } else {
                    u32::from_le_bytes([
                        byte(x * 4),
                        byte(x * 4 + 1),
                        byte(x * 4 + 2),
                        byte(x * 4 + 3),
                    ])
                };
                let [rm, gm, bm, am] = self.masks.unwrap_or_default();
                let (r, g, b) = (channel(value, rm), channel(value, gm), channel(value, bm));
                if !(alpha.per_pixel && self.bpp == 32) {
                    return [r, g, b, alpha.constant];
                }
                let a = if am == 0 { 255 } else { channel(value, am) };
                // Premultiplied source: undo the premultiplication for a straight-alpha PNG.
                let straight = |c: u8| {
                    (u32::from(c) * 255)
                        .checked_div(u32::from(a))
                        .unwrap_or(0)
                        .min(255) as u8
                };
                let a = (u32::from(a) * u32::from(alpha.constant) / 255) as u8;
                [straight(r), straight(g), straight(b), a]
            }
        }
    }
}

/// The 8-bit value of the bits of `value` that `mask` selects.
fn channel(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = u64::from(mask >> shift);
    let v = u64::from((value & mask) >> shift);
    (v * 255 / max) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info(width: i32, height: i32, bpp: u16, compression: u32, palette: &[[u8; 4]]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend(40u32.to_le_bytes());
        data.extend(width.to_le_bytes());
        data.extend(height.to_le_bytes());
        data.extend(1u16.to_le_bytes());
        data.extend(bpp.to_le_bytes());
        data.extend(compression.to_le_bytes());
        data.extend([0; 12]);
        data.extend((palette.len() as u32).to_le_bytes());
        data.extend(0u32.to_le_bytes());
        for entry in palette {
            data.extend(entry);
        }
        data
    }
    fn rows(dib: &Dib<'_>) -> Vec<Vec<u8>> {
        (0..dib.height)
            .map(|y| {
                let mut out = vec![0; dib.width * 4];
                dib.row(y, 0, &mut out, Alpha::OPAQUE).unwrap();
                out
            })
            .collect()
    }
    #[test]
    fn bottom_up_24_bit_rows_are_returned_top_down() {
        let header = info(1, 2, 24, 0, &[]);
        // Bottom row blue, top row red; rows are padded to four bytes.
        let bits = [255, 0, 0, 0, 0, 0, 255, 0];
        let dib = Dib::parse(&header, &bits).unwrap();
        assert_eq!(rows(&dib), [vec![255, 0, 0, 255], vec![0, 0, 255, 255]]);
    }
    #[test]
    fn palette_bitmaps_index_their_colours() {
        let header = info(9, -1, 1, 0, &[[0, 0, 0, 0], [255, 255, 255, 0]]);
        let dib = Dib::parse(&header, &[0b1000_0000, 0b1000_0000]).unwrap();
        let row = &rows(&dib)[0];
        assert_eq!(&row[0..4], &[255, 255, 255, 255]);
        assert_eq!(&row[4..8], &[0, 0, 0, 255]);
        assert_eq!(&row[32..36], &[255, 255, 255, 255]);
        let header = info(2, 1, 4, 0, &[[0, 0, 255, 0], [0, 255, 0, 0]]);
        let dib = Dib::parse(&header, &[0x10]).unwrap();
        assert_eq!(rows(&dib)[0], [0, 255, 0, 255, 255, 0, 0, 255]);
    }
    #[test]
    fn premultiplied_alpha_is_made_straight() {
        let header = info(1, 1, 32, 0, &[]);
        let dib = Dib::parse(&header, &[0, 0, 64, 128]).unwrap();
        let mut out = [0; 4];
        let blend = Alpha {
            constant: 255,
            per_pixel: true,
        };
        dib.row(0, 0, &mut out, blend).unwrap();
        assert_eq!(out, [127, 0, 0, 128]);
        dib.row(0, 0, &mut out, Alpha::OPAQUE).unwrap();
        assert_eq!(out, [64, 0, 0, 255]);
    }
    #[test]
    fn short_or_compressed_bitmaps_are_rejected() {
        let header = info(4, 4, 24, 0, &[]);
        assert!(matches!(
            Dib::parse(&header, &[0; 20]),
            Err(Error::Malformed(_))
        ));
        assert!(matches!(
            Dib::parse(&info(4, 4, 8, 1, &[]), &[0; 64]),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            Dib::parse(&info(0, 4, 24, 0, &[]), &[]),
            Err(Error::Malformed(_))
        ));
        assert!(matches!(
            Dib::parse(&info(i32::MAX, 1, 32, 0, &[]), &[]),
            Err(Error::Malformed(_))
        ));
    }
}
