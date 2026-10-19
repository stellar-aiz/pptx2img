//! Size policy for embedded PNG/JPEG pictures, and a bounded-memory box-filter downscale.
//!
//! One oversized picture must not abort the deck. Pictures within the embed limit are
//! passed through unchanged; larger ones up to a hard ceiling are downscaled before they
//! reach the SVG renderer; anything above the ceiling is omitted by the caller.
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{cell::RefCell, collections::HashMap, io::Cursor, rc::Rc};
use zune_jpeg::{
    JpegDecoder,
    zune_core::{colorspace::ColorSpace, options::DecoderOptions},
};

/// Largest side and pixel count embedded as-is (unchanged from the original limit).
const EMBED_SIDE: usize = 8192;
const EMBED_PIXELS: usize = 16_777_216;
/// Hard ceiling for downscaling: larger pictures are omitted.
const CEILING_SIDE: usize = 32_768;
const CEILING_PIXELS: usize = 67_108_864;
/// Longest side after downscaling. A fixed, resource-bounded target: the final output
/// scale (group transforms, crop, `--width`) is unknown where pictures are decoded.
// stellar:debt(perf) the target ignores the displayed size. upgrade: derive it from the output scale and crop.
const TARGET_SIDE: usize = 2048;

enum Fit {
    Embed,
    Downscale,
    Omit,
}
fn fit(width: usize, height: usize) -> Fit {
    let pixels = width.saturating_mul(height);
    if width <= EMBED_SIDE && height <= EMBED_SIDE && pixels <= EMBED_PIXELS {
        Fit::Embed
    } else if width <= CEILING_SIDE && height <= CEILING_SIDE && pixels <= CEILING_PIXELS {
        Fit::Downscale
    } else {
        Fit::Omit
    }
}

/// How a PNG/JPEG picture reaches the SVG: unchanged, or as a downscaled PNG data URI.
#[derive(Clone)]
pub(super) enum Prepared {
    Embed,
    Downscaled {
        href: Rc<str>,
        width: usize,
        height: usize,
    },
}
/// Outcome for one media part; `Err` holds the omission reason (without the part name).
type Outcome = Result<Prepared, String>;

thread_local! {
    /// Layout and master pictures repeat on every slide; decode each media part once.
    static CACHE: RefCell<HashMap<String, Outcome>> = RefCell::new(HashMap::new());
}
/// Scope the cache to one conversion: cleared when created and when dropped.
pub(super) struct CacheScope;
impl CacheScope {
    pub fn new() -> Self {
        CACHE.with(|c| c.borrow_mut().clear());
        Self
    }
}
impl Drop for CacheScope {
    fn drop(&mut self) {
        CACHE.with(|c| c.borrow_mut().clear());
    }
}

/// Decide how to embed a PNG/JPEG part of `width`x`height` px.
pub(super) fn prepare(part: &str, data: &[u8], mime: &str, width: usize, height: usize) -> Outcome {
    match fit(width, height) {
        Fit::Embed => Ok(Prepared::Embed),
        Fit::Omit => Err(format!(
            "Image exceeds the downscale ceiling of {CEILING_PIXELS} pixels or {CEILING_SIDE} px per side ({width}x{height} px)"
        )),
        Fit::Downscale => {
            if let Some(outcome) = CACHE.with(|c| c.borrow().get(part).cloned()) {
                return outcome;
            }
            let outcome = downscale(data, mime == "image/png", (width, height))
                .map(|(png, width, height)| Prepared::Downscaled {
                    href: format!("data:image/png;base64,{}", STANDARD.encode(png)).into(),
                    width,
                    height,
                })
                .map_err(|error| {
                    format!("Cannot downscale oversized image ({width}x{height} px, {error})")
                });
            CACHE.with(|c| c.borrow_mut().insert(part.into(), outcome.clone()));
            outcome
        }
    }
}

/// Decode a PNG/JPEG and return a PNG whose longest side is at most `TARGET_SIDE`.
/// `probed` is the size the ceiling was checked against; the decoder must agree with it
/// before any pixel buffer is allocated, so a header parsed differently cannot bypass the cap.
fn downscale(data: &[u8], png: bool, probed: (usize, usize)) -> Result<(Vec<u8>, usize, usize)> {
    let shrink = if png {
        downscale_png(data, probed)?
    } else {
        downscale_jpeg(data, probed)?
    };
    let mut encoded = Vec::new();
    let mut encoder = png::Encoder::new(&mut encoded, shrink.tw as u32, shrink.th as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&shrink.out)?;
    writer.finish()?;
    Ok((encoded, shrink.tw, shrink.th))
}

fn agree(decoded: (usize, usize), probed: (usize, usize)) -> Result<()> {
    if decoded != probed {
        bail!(
            "decoder reports {}x{} px, header probe {}x{} px",
            decoded.0,
            decoded.1,
            probed.0,
            probed.1
        );
    }
    Ok(())
}

fn downscale_png(data: &[u8], probed: (usize, usize)) -> Result<Shrink> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().context("Cannot decode PNG")?;
    if reader.info().interlaced {
        // Adam7 rows arrive out of order, which a row-streaming box filter cannot accumulate.
        // stellar:debt(scope) oversized interlaced PNG is omitted. upgrade: deinterlace into a bounded accumulator.
        bail!("interlaced PNG is not downscaled");
    }
    let (width, height) = (reader.info().width as usize, reader.info().height as usize);
    agree((width, height), probed)?;
    let channels = reader.output_color_type().0.samples();
    let mut shrink = Shrink::new(width, height);
    let mut y = 0;
    while let Some(row) = reader.next_row().context("Cannot decode PNG")? {
        shrink.push(y, row.data(), channels);
        y += 1;
    }
    shrink.finish(y)
}

fn downscale_jpeg(data: &[u8], probed: (usize, usize)) -> Result<Shrink> {
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_max_width(probed.0)
        .set_max_height(probed.1);
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(data), options);
    let error = |e| anyhow::anyhow!("Cannot decode JPEG: {e:?}");
    decoder.decode_headers().map_err(error)?;
    let info = decoder.info().context("Cannot decode JPEG")?;
    let (width, height) = (usize::from(info.width), usize::from(info.height));
    agree((width, height), probed)?;
    // stellar:debt(perf) JPEG is decoded at full size (no scaled decode in zune-jpeg). upgrade: DCT-domain scaling.
    let pixels = decoder.decode().map_err(error)?;
    if pixels.len() != width * height * 3 {
        bail!("Cannot decode JPEG: unexpected pixel buffer size");
    }
    let mut shrink = Shrink::new(width, height);
    for (y, row) in pixels.chunks_exact(width * 3).enumerate() {
        shrink.push(y, row, 3);
    }
    shrink.finish(height)
}

/// Area-average source rows into target rows; only one target row is accumulated at a time.
/// resvg's bilinear/bicubic filters would need the whole image decoded and alias at large ratios.
struct Shrink {
    sw: usize,
    sh: usize,
    tw: usize,
    th: usize,
    /// Premultiplied RGB sums, alpha sum, and sample count per target column.
    acc: Vec<[u64; 5]>,
    current: usize,
    out: Vec<u8>,
}
impl Shrink {
    fn new(sw: usize, sh: usize) -> Self {
        let scale = TARGET_SIDE as f64 / sw.max(sh) as f64;
        let tw = ((sw as f64 * scale).round() as usize).clamp(1, sw);
        let th = ((sh as f64 * scale).round() as usize).clamp(1, sh);
        Self {
            sw,
            sh,
            tw,
            th,
            acc: vec![[0; 5]; tw],
            current: 0,
            out: Vec::with_capacity(tw * th * 4),
        }
    }
    fn push(&mut self, y: usize, row: &[u8], channels: usize) {
        let ty = y * self.th / self.sh;
        while self.current < ty {
            self.flush();
        }
        for (x, px) in row.chunks_exact(channels).take(self.sw).enumerate() {
            let (r, g, b, a) = match px {
                [v] => (*v, *v, *v, 255),
                [v, a] => (*v, *v, *v, *a),
                [r, g, b] => (*r, *g, *b, 255),
                [r, g, b, a, ..] => (*r, *g, *b, *a),
                _ => unreachable!("channel count is 1 to 4"),
            };
            let a = u64::from(a);
            let cell = &mut self.acc[x * self.tw / self.sw];
            cell[0] += u64::from(r) * a;
            cell[1] += u64::from(g) * a;
            cell[2] += u64::from(b) * a;
            cell[3] += a;
            cell[4] += 1;
        }
    }
    fn flush(&mut self) {
        for cell in &mut self.acc {
            let [r, g, b, a, n] = *cell;
            let color = |c: u64| (c + a / 2).checked_div(a).unwrap_or(0) as u8;
            let alpha = (a + n / 2).checked_div(n).unwrap_or(0) as u8;
            self.out.extend([color(r), color(g), color(b), alpha]);
            *cell = [0; 5];
        }
        self.current += 1;
    }
    fn finish(mut self, rows: usize) -> Result<Self> {
        if rows != self.sh {
            bail!("image data ended early");
        }
        while self.current < self.th {
            self.flush();
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn box_filter_averages_premultiplied_and_keeps_aspect() {
        let mut shrink = Shrink::new(4096, 2048);
        assert_eq!((shrink.tw, shrink.th), (2048, 1024));
        // Opaque red next to transparent green must stay red, at half alpha.
        let row: Vec<u8> = (0..4096)
            .flat_map(|x| {
                if x % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 0]
                }
            })
            .collect();
        for y in 0..2048 {
            shrink.push(y, &row, 4);
        }
        let shrink = shrink.finish(2048).unwrap();
        assert_eq!(shrink.out.len(), 2048 * 1024 * 4);
        assert_eq!(&shrink.out[..4], &[255, 0, 0, 128]);
    }
    #[test]
    fn decoder_size_must_match_the_probed_size() {
        let mut data = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut data, 9000, 2);
            encoder.set_color(png::ColorType::Grayscale);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0; 18000]).unwrap();
        }
        assert!(downscale(&data, true, (9000, 2)).is_ok());
        let error = downscale(&data, true, (9000, 1)).unwrap_err().to_string();
        assert_eq!(error, "decoder reports 9000x2 px, header probe 9000x1 px");
        let jpeg = include_bytes!("../../tests/fixtures/oversized.jpg");
        assert!(downscale(jpeg, false, (8400, 64)).is_ok());
        assert!(downscale(jpeg, false, (8400, 63)).is_err());
    }
    #[test]
    fn size_policy_boundaries() {
        assert!(matches!(fit(8192, 2048), Fit::Embed));
        assert!(matches!(fit(4097, 4096), Fit::Downscale));
        assert!(matches!(fit(8193, 1), Fit::Downscale));
        assert!(matches!(fit(8192, 8192), Fit::Downscale));
        assert!(matches!(fit(32_768, 2048), Fit::Downscale));
        assert!(matches!(fit(32_769, 1), Fit::Omit));
        assert!(matches!(fit(8193, 8193), Fit::Omit));
    }
}
