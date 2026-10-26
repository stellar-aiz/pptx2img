//! Focused PDF 1.4 backend using the existing scene and font libraries.
//! Reject unsupported nodes instead of silently changing their appearance.
use anyhow::{Context, Result, bail, ensure};
use resvg::{
    tiny_skia::{PathSegment, Transform},
    usvg,
};
use skrifa::{
    FontRef, MetadataProvider,
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
    raw::TableProvider,
};
use std::{collections::HashMap, fmt::Write};

#[derive(Default)]
struct Pen {
    data: String,
    x: f32,
    y: f32,
    start: (f32, f32),
}
impl Pen {
    fn from_path(path: &resvg::tiny_skia::Path) -> Self {
        let mut pen = Self::default();
        for seg in path.segments() {
            match seg {
                PathSegment::MoveTo(p) => pen.move_to(p.x, p.y),
                PathSegment::LineTo(p) => pen.line_to(p.x, p.y),
                PathSegment::QuadTo(c, p) => pen.quad_to(c.x, c.y, p.x, p.y),
                PathSegment::CubicTo(a, b, p) => pen.curve_to(a.x, a.y, b.x, b.y, p.x, p.y),
                PathSegment::Close => pen.close(),
            }
        }
        pen
    }
}
impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        writeln!(self.data, "{x} {y} m").unwrap();
        self.start = (x, y);
        self.x = x;
        self.y = y;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        writeln!(self.data, "{x} {y} l").unwrap();
        self.x = x;
        self.y = y;
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.curve_to(
            self.x + (cx - self.x) * 2.0 / 3.0,
            self.y + (cy - self.y) * 2.0 / 3.0,
            x + (cx - x) * 2.0 / 3.0,
            y + (cy - y) * 2.0 / 3.0,
            x,
            y,
        );
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        writeln!(self.data, "{a} {b} {c} {d} {x} {y} c").unwrap();
        self.x = x;
        self.y = y;
    }
    fn close(&mut self) {
        self.data.push_str("h\n");
        (self.x, self.y) = self.start;
    }
}
fn matrix(t: Transform) -> String {
    format!("{} {} {} {} {} {}", t.sx, t.ky, t.kx, t.sy, t.tx, t.ty)
}
fn color(paint: &usvg::Paint, stroke: bool) -> Result<String> {
    let usvg::Paint::Color(c) = paint else {
        bail!("Gradient/pattern paint is not implemented")
    };
    Ok(format!(
        "{} {} {} {}\n",
        c.red as f32 / 255.0,
        c.green as f32 / 255.0,
        c.blue as f32 / 255.0,
        if stroke { "RG" } else { "rg" }
    ))
}
pub struct Pdf {
    objects: Vec<Vec<u8>>,
    pages: Vec<usize>,
    fonts: HashMap<String, (usize, f32)>,
    object_bytes: usize,
}
impl Pdf {
    pub fn new() -> Self {
        Self {
            objects: vec![vec![], vec![]],
            pages: vec![],
            fonts: HashMap::new(),
            object_bytes: 0,
        }
    }
    fn object(&mut self, data: impl Into<Vec<u8>>) -> usize {
        let data = data.into();
        self.object_bytes += data.len();
        self.objects.push(data);
        self.objects.len()
    }
    fn stream(&mut self, extra: &str, data: &[u8]) -> usize {
        let mut out = format!("<< {extra} /Length {} >>\nstream\n", data.len()).into_bytes();
        out.extend_from_slice(data);
        out.extend_from_slice(b"\nendstream");
        self.object(out)
    }
    fn budget(&self, content: usize) -> Result<()> {
        ensure!(
            self.object_bytes.saturating_add(content) <= 128 * 1024 * 1024,
            "PDF content exceeds 128 MiB limit"
        );
        ensure!(
            self.objects.len() <= 100_000,
            "PDF object count exceeds limit"
        );
        Ok(())
    }
    fn font(
        &mut self,
        tree: &usvg::Tree,
        face: usvg::fontdb::ID,
        gid: usvg::GlyphId,
        text: &str,
    ) -> Result<(usize, f32)> {
        ensure!(gid.0 != 0, "Missing PDF glyph for {text:?}");
        let key = format!("{}:{}:{}", face, gid.0, text);
        if let Some(value) = self.fonts.get(&key) {
            return Ok(*value);
        }
        ensure!(
            !text.is_empty(),
            "Empty glyph Unicode mapping (cluster needs dedicated handling)"
        );
        // stellar:debt(perf) one resource per glyph/text pair. upgrade: pack up to 255 codes per Type 3 font.
        let (path, upem, width, bounds) = tree
            .fontdb()
            .with_face_data(face, |data, index| -> Result<_> {
                let font = FontRef::from_index(data, index)?;
                ensure!(
                    ![b"COLR", b"CBDT", b"sbix", b"SVG "]
                        .iter()
                        .any(|tag| font.table_data(skrifa::Tag::new(tag)).is_some()),
                    "Color/bitmap font is unsupported for PDF text"
                );
                let flags = font
                    .os2()
                    .context("Cannot check embedding flags")?
                    .fs_type();
                // Conservative probe policy: also reject no-subsetting and bitmap-only fonts.
                ensure!(
                    flags & 0x0302 == 0,
                    "Font embedding flags unsupported: 0x{flags:04x}"
                );
                ensure!(
                    font.axes().is_empty(),
                    "Variable fonts require explicit instance handling"
                );
                let metrics = font.metrics(Size::unscaled(), LocationRef::default());
                let width = font
                    .glyph_metrics(Size::unscaled(), LocationRef::default())
                    .advance_width(skrifa::GlyphId::new(gid.0))
                    .context("No advance width")?;
                let mut pen = Pen::default();
                font.outline_glyphs()
                    .get(skrifa::GlyphId::new(gid.0))
                    .context("No outline glyph")?
                    .draw(
                        DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                        &mut pen,
                    )?;
                Ok((pen.data, metrics.units_per_em as f32, width, metrics.bounds))
            })
            .context("Font data unavailable")??;
        let bounds = bounds.context("No font bounding box")?;
        let proc = self.stream("", format!("{width} 0 d0\n{path}f\n").as_bytes());
        let unicode = text
            .encode_utf16()
            .map(|u| format!("{u:04X}"))
            .collect::<String>();
        let cmap=self.stream("",format!("/CIDInit /ProcSet findresource begin\n12 dict begin begincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /ProbeUnicode def /CMapType 2 def\n1 begincodespacerange <00> <FF> endcodespacerange\n1 beginbfchar <01> <{unicode}> endbfchar\nendcmap CMapName currentdict /CMap defineresource pop end end").as_bytes());
        let id=self.object(format!("<< /Type /Font /Subtype /Type3 /FontBBox [{} {} {} {}] /FontMatrix [{} 0 0 {} 0 0] /CharProcs << /g {proc} 0 R >> /Encoding << /Type /Encoding /Differences [1 /g] >> /FirstChar 1 /LastChar 1 /Widths [{width}] /Resources << >> /ToUnicode {cmap} 0 R >>",bounds.x_min,bounds.y_min,bounds.x_max,bounds.y_max,1.0/upem,1.0/upem));
        self.fonts.insert(key, (id, upem));
        Ok((id, upem))
    }
    fn path(&mut self, path: &usvg::Path, out: &mut String) -> Result<()> {
        if !path.is_visible() {
            return Ok(());
        }
        let pen = Pen::from_path(path.data());
        writeln!(out, "q {} cm", matrix(path.abs_transform()))?;
        let order = if path.paint_order() == usvg::PaintOrder::FillAndStroke {
            [false, true]
        } else {
            [true, false]
        };
        for stroke in order {
            if stroke {
                if let Some(s) = path.stroke() {
                    ensure!(
                        s.opacity().get() == 1.0,
                        "Stroke opacity is not implemented"
                    );
                    out.push_str(&color(s.paint(), true)?);
                    let cap = match s.linecap() {
                        usvg::LineCap::Butt => 0,
                        usvg::LineCap::Round => 1,
                        usvg::LineCap::Square => 2,
                    };
                    let join = match s.linejoin() {
                        usvg::LineJoin::Miter => 0,
                        usvg::LineJoin::Round => 1,
                        usvg::LineJoin::Bevel => 2,
                        _ => bail!("Miter-clip is not implemented"),
                    };
                    let dash = s
                        .dasharray()
                        .unwrap_or(&[])
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    writeln!(
                        out,
                        "{} w {cap} J {join} j {} M [{dash}] {} d\n{}S",
                        s.width().get(),
                        s.miterlimit().get(),
                        s.dashoffset(),
                        pen.data
                    )?;
                }
            } else if let Some(f) = path.fill() {
                ensure!(f.opacity().get() == 1.0, "Fill opacity is not implemented");
                out.push_str(&color(f.paint(), false)?);
                out.push_str(&pen.data);
                out.push_str(if f.rule() == usvg::FillRule::EvenOdd {
                    "f*\n"
                } else {
                    "f\n"
                });
            }
        }
        out.push_str("Q\n");
        Ok(())
    }
    fn group(
        &mut self,
        group: &usvg::Group,
        tree: &usvg::Tree,
        out: &mut String,
        resources: &mut String,
        location: &str,
    ) -> Result<()> {
        ensure!(
            group.opacity().get() == 1.0
                && group.blend_mode() == usvg::BlendMode::Normal
                && !group.isolate(),
            "{location}: group compositing is not implemented"
        );
        ensure!(
            group.mask().is_none() && group.filters().is_empty(),
            "{location}: masks or filters are not implemented"
        );
        out.push_str("q\n");
        if let Some(clip) = group.clip_path() {
            ensure!(
                clip.clip_path().is_none() && clip.root().children().len() == 1,
                "Nested/multiple clip paths are not implemented"
            );
            let usvg::Node::Path(p) = &clip.root().children()[0] else {
                bail!("Non-path clip is not implemented")
            };
            let transform = group
                .abs_transform()
                .pre_concat(clip.transform())
                .pre_concat(p.abs_transform());
            let path = p
                .data()
                .clone()
                .transform(transform)
                .context("Invalid clip transform")?;
            ensure!(
                p.is_visible() && p.fill().is_some(),
                "Invisible clip path is unsupported"
            );
            let pen = Pen::from_path(&path);
            out.push_str(&pen.data);
            out.push_str(
                if p.fill()
                    .is_some_and(|f| f.rule() == usvg::FillRule::EvenOdd)
                {
                    "W* n\n"
                } else {
                    "W n\n"
                },
            );
        }
        for (index, node) in group.children().iter().enumerate() {
            self.budget(out.len())?;
            let loc = format!("{location}/{index}");
            let result: Result<()> = (|| {
                match node {
                    usvg::Node::Group(g) => self.group(g, tree, out, resources, &loc)?,
                    usvg::Node::Path(p) => self.path(p, out)?,
                    usvg::Node::Text(t) => {
                        let actual = t
                            .chunks()
                            .iter()
                            .flat_map(|c| {
                                c.spans()
                                    .iter()
                                    .filter(|s| s.is_visible() && s.fill().is_some())
                                    .map(|s| &c.text()[s.start()..s.end()])
                            })
                            .flat_map(str::encode_utf16)
                            .map(|u| format!("{u:04X}"))
                            .collect::<String>();
                        writeln!(
                            out,
                            "q {} cm /Span << /ActualText <FEFF{actual}> >> BDC",
                            matrix(t.abs_transform())
                        )?;
                        for span in t.layouted() {
                            if !span.visible {
                                continue;
                            }
                            ensure!(span.stroke.is_none(), "Text strokes are not implemented");
                            ensure!(
                                span.underline.is_none()
                                    && span.overline.is_none()
                                    && span.line_through.is_none(),
                                "Text decorations are not implemented"
                            );
                            let Some(fill) = span.fill.as_ref() else {
                                continue;
                            };
                            ensure!(
                                fill.opacity().get() == 1.0,
                                "Text opacity is not implemented"
                            );
                            out.push_str(&color(fill.paint(), false)?);
                            for glyph in &span.positioned_glyphs {
                                self.budget(out.len())?;
                                let (id, upem) =
                                    self.font(tree, glyph.font, glyph.id, &glyph.text)?;
                                let transform = glyph.outline_transform().pre_scale(upem, upem);
                                writeln!(
                                    out,
                                    "BT /F{id} 1 Tf {} Tm <01> Tj ET",
                                    matrix(transform)
                                )?;
                            }
                        }
                        out.push_str("EMC Q\n");
                    }
                    usvg::Node::Image(image) => {
                        if !image.is_visible() {
                            return Ok(());
                        }
                        let (width, height, rgb) = match image.kind() {
                            usvg::ImageKind::PNG(data) => {
                                let size = imagesize::blob_size(data)?;
                                ensure!(
                                    (size.width as u64) * (size.height as u64) <= 16_777_216,
                                    "PDF image exceeds pixel limit"
                                );
                                let pix = resvg::tiny_skia::Pixmap::decode_png(data)?;
                                ensure!(
                                    pix.pixels().iter().all(|p| p.alpha() == 255),
                                    "Image alpha is unsupported for PDF"
                                );
                                (
                                    pix.width() as usize,
                                    pix.height() as usize,
                                    pix.pixels()
                                        .iter()
                                        .flat_map(|p| [p.red(), p.green(), p.blue()])
                                        .collect::<Vec<_>>(),
                                )
                            }
                            usvg::ImageKind::JPEG(data) => {
                                use zune_jpeg::{
                                    JpegDecoder,
                                    zune_core::{colorspace::ColorSpace, options::DecoderOptions},
                                };
                                let options = DecoderOptions::default()
                                    .jpeg_set_out_colorspace(ColorSpace::RGB);
                                let mut decoder = JpegDecoder::new_with_options(
                                    std::io::Cursor::new(data.as_slice()),
                                    options,
                                );
                                decoder
                                    .decode_headers()
                                    .map_err(|e| anyhow::anyhow!("Cannot decode JPEG: {e:?}"))?;
                                let info = decoder.info().context("JPEG info unavailable")?;
                                let (w, h) = (info.width as usize, info.height as usize);
                                ensure!(
                                    (w as u64) * (h as u64) <= 16_777_216,
                                    "PDF image exceeds pixel limit"
                                );
                                let rgb = decoder
                                    .decode()
                                    .map_err(|e| anyhow::anyhow!("Cannot decode JPEG: {e:?}"))?;
                                ensure!(rgb.len() == w * h * 3, "Unexpected JPEG pixel buffer");
                                (w, h, rgb)
                            }
                            _ => bail!("PDF supports PNG/JPEG images only"),
                        };
                        // stellar:debt(perf) raw RGB streams. upgrade: reuse existing compression facilities without new dependencies.
                        let id=self.stream(&format!("/Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8"),&rgb);
                        write!(resources, "/I{id} {id} 0 R ")?;
                        writeln!(
                            out,
                            "q {} cm {} 0 0 {} 0 {} cm /I{id} Do Q",
                            matrix(image.abs_transform()),
                            image.size().width(),
                            -image.size().height(),
                            image.size().height()
                        )?;
                    }
                }
                Ok(())
            })();
            result.with_context(|| format!("{loc} (SVG id {:?})", node.id()))?;
        }
        out.push_str("Q\n");
        Ok(())
    }
    pub fn page(&mut self, tree: &usvg::Tree) -> Result<()> {
        ensure!(
            tree.size().width() * 0.75 <= 14_400.0 && tree.size().height() * 0.75 <= 14_400.0,
            "PDF page exceeds 200 inch limit"
        );
        let mut content = format!("q 0.75 0 0 -0.75 0 {} cm\n", tree.size().height() * 0.75);
        let mut images = String::new();
        self.group(tree.root(), tree, &mut content, &mut images, "root")?;
        content.push_str("Q\n");
        self.budget(content.len())?;
        let stream = self.stream("", content.as_bytes());
        let mut fonts = self.fonts.values().map(|(id, _)| *id).collect::<Vec<_>>();
        fonts.sort_unstable();
        let fonts = fonts
            .into_iter()
            .map(|id| format!("/F{id} {id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        let page=self.object(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << {fonts} >> /XObject << {images} >> >> /Contents {stream} 0 R >>",tree.size().width()*0.75,tree.size().height()*0.75));
        self.pages.push(page);
        Ok(())
    }
    pub fn finish(&mut self) -> Result<Vec<u8>> {
        self.budget(0)?;
        self.objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
        self.objects[1] = format!(
            "<< /Type /Pages /Count {} /Kids [{}] >>",
            self.pages.len(),
            self.pages
                .iter()
                .map(|p| format!("{p} 0 R"))
                .collect::<Vec<_>>()
                .join(" ")
        )
        .into_bytes();
        let mut pdf = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = vec![0];
        for (i, obj) in self.objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            pdf.extend_from_slice(obj);
            pdf.extend_from_slice(b"\nendobj\n");
        }
        let xref = pdf.len();
        pdf.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes(),
        );
        for offset in &offsets[1..] {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes())
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        Ok(pdf)
    }
}
