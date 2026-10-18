//! グループの座標系と、明示書式の表を描画する。
use super::*;

pub(super) struct Shape<'a> {
    pub chain: Vec<Node<'a, 'a>>,
    pub master_style: Option<Node<'a, 'a>>,
}
pub(super) struct Renderer<'a, 'd> {
    pub parts: &'a Parts,
    pub rels: &'a Relationships,
    pub theme: &'a Theme,
    pub options: &'a RenderOptions<'a>,
    pub svg: &'a mut String,
    pub warnings: &'a mut Warnings,
    pub part: &'a str,
    pub ancestors: &'a [Node<'d, 'd>],
    pub master_style: Option<Node<'d, 'd>>,
}
impl<'d> Renderer<'_, 'd> {
    pub fn node(&mut self, node: Node<'d, 'd>, location: &str, depth: usize) -> Result<()> {
        if depth > 64 {
            bail!("グループの入れ子は 64 段までです");
        }
        let previous = self.warnings.target.clone();
        let identity = node.children().find_map(|n| child(n, P, "cNvPr"));
        self.warnings.target = Target {
            part: self.part.into(),
            location: location.into(),
            object_id: identity.and_then(|n| n.attribute("id")).map(str::to_owned),
            object_name: identity
                .and_then(|n| n.attribute("name"))
                .map(str::to_owned),
        };
        if node.tag_name().namespace() != Some(P) {
            self.warnings.push(
                Code::Omitted,
                format!("未対応の要素を省略しました: {}", node.tag_name().name()),
            );
        } else {
            match node.tag_name().name() {
                "nvGrpSpPr" | "grpSpPr" => {}
                "sp" | "pic" | "cxnSp" => render_shape(
                    Shape {
                        chain: layout::inherited(node, self.ancestors),
                        master_style: self.master_style,
                    },
                    self.parts,
                    self.rels,
                    self.theme,
                    self.options,
                    self.svg,
                    self.warnings,
                )?,
                "grpSp" => self.group(node, location, depth)?,
                "graphicFrame" => {
                    let ole = child(node, A, "graphic")
                        .and_then(|n| child(n, A, "graphicData"))
                        .and_then(|data| {
                            child(data, P, "oleObj").or_else(|| {
                                const MC: &str =
                                    "http://schemas.openxmlformats.org/markup-compatibility/2006";
                                child(data, MC, "AlternateContent")
                                    .and_then(|n| child(n, MC, "Fallback"))
                                    .and_then(|n| child(n, P, "oleObj"))
                            })
                        });
                    if let Some(ole) = ole {
                        self.ole(node, ole)?;
                    } else {
                        self.table(node, location)?;
                    }
                }
                other => self.warnings.push(
                    Code::Omitted,
                    format!("未対応の要素を省略しました: {other}"),
                ),
            }
        }
        self.warnings.target = previous;
        Ok(())
    }
    fn group(&mut self, node: Node<'d, 'd>, location: &str, depth: usize) -> Result<()> {
        let props = child(node, P, "grpSpPr").context("グループ属性がありません")?;
        let t = child(props, A, "xfrm").context("グループ座標がありません")?;
        let (w, h) = transform(t, self.svg)?;
        let off = child(t, A, "chOff").context("グループの子座標原点がありません")?;
        let ext = child(t, A, "chExt").context("グループの子座標寸法がありません")?;
        let cw = number(ext, "cx", 0.0) / EMU;
        let ch = number(ext, "cy", 0.0) / EMU;
        if cw <= 0.0 || ch <= 0.0 {
            bail!("グループの子座標寸法は正数が必要です");
        }
        for prop in props.children().filter(|n| n.is_element()) {
            if !prop.has_tag_name((A, "xfrm")) {
                warn(self.warnings, "グループの塗り・効果の継承は未対応です");
            }
        }
        self.svg.push_str(&format!(
            "<g transform=\"scale({} {}) translate({} {})\">",
            w / cw,
            h / ch,
            -number(off, "x", 0.0) / EMU,
            -number(off, "y", 0.0) / EMU
        ));
        for (i, n) in node.children().filter(|n| n.is_element()).enumerate() {
            self.node(n, &format!("{location}/{i}"), depth + 1)?;
        }
        self.svg.push_str("</g></g>");
        Ok(())
    }
    /// OLE 本体にはアクセスせず、同一パッケージの静止画だけを表示する。
    fn ole(&mut self, frame: Node<'d, 'd>, ole: Node<'d, 'd>) -> Result<()> {
        let kind = ole.attribute("progId").unwrap_or("種別不明");
        let preview = child(ole, P, "pic");
        let hidden = [frame, preview.unwrap_or(ole)].iter().any(|node| {
            node.children()
                .find_map(|n| child(n, P, "cNvPr"))
                .and_then(|n| n.attribute("hidden"))
                .is_some_and(|v| v == "1" || v == "true")
        });
        if hidden {
            self.warnings.push(
                Code::Omitted,
                format!("OLE ({kind}) は hidden 指定のため非表示です。OLE は実行しません"),
            );
            return Ok(());
        }
        let reason = (|| {
            let pic = preview.ok_or("プレビュー画像がありません")?;
            let blip = pic
                .descendants()
                .find(|n| n.has_tag_name((A, "blip")))
                .ok_or("プレビューの画像参照がありません")?;
            let id = blip
                .attribute((R, "embed"))
                .ok_or("外部参照のプレビューは取得しません")?;
            let rel = self.rels.get(id).ok_or("プレビューの関連がありません")?;
            if rel.external {
                return Err("外部参照のプレビューは取得しません");
            }
            if !rel.kind.ends_with("/image") {
                return Err("プレビューの関連が画像ではありません");
            }
            let data = self
                .parts
                .get(&rel.target)
                .ok_or("プレビュー画像の部品がありません")?;
            if !data.starts_with(b"\x89PNG\r\n\x1a\n") && !data.starts_with(b"\xff\xd8\xff") {
                return Err("プレビュー形式は PNG/JPEG 以外のため未対応です");
            }
            if imagesize::blob_size(data).is_err() {
                return Err("プレビュー画像が不正です");
            }
            Ok(())
        })();
        if let Err(reason) = reason {
            self.warnings.push(
                Code::Omitted,
                format!("OLE ({kind}) を省略しました: {reason}。OLE は実行しません"),
            );
            return Ok(());
        }
        let Some(t) = child(frame, P, "xfrm") else {
            self.warnings.push(
                Code::Omitted,
                format!("OLE ({kind}) の表示座標がありません"),
            );
            return Ok(());
        };
        let (w, h) = transform(t, self.svg)?;
        // pic の内側座標を重ねて適用しない。graphicFrame が表示領域を持つ。
        render_image(
            preview.unwrap(),
            self.parts,
            self.rels,
            w,
            h,
            self.svg,
            self.warnings,
        )?;
        self.svg.push_str("</g>");
        self.warnings.push(
            Code::Approximation,
            format!("OLE ({kind}) は実行せず、内部 PNG/JPEG プレビューを静的描画しました"),
        );
        Ok(())
    }
    fn table(&mut self, node: Node<'d, 'd>, location: &str) -> Result<()> {
        let table = child(node, A, "graphic")
            .and_then(|n| child(n, A, "graphicData"))
            .and_then(|n| child(n, A, "tbl"));
        let Some(table) = table else {
            self.warnings.push(
                Code::Omitted,
                "未対応の要素を省略しました: graphicFrame（表以外）",
            );
            return Ok(());
        };
        let t = child(node, P, "xfrm").context("表の座標がありません")?;
        let (w, h) = transform(t, self.svg)?;
        let grid = child(table, A, "tblGrid").context("表の列幅がありません")?;
        let widths: Vec<_> = grid
            .children()
            .filter(|n| n.has_tag_name((A, "gridCol")))
            .map(|n| number(n, "w", 0.0) / EMU)
            .collect();
        let rows: Vec<_> = table
            .children()
            .filter(|n| n.has_tag_name((A, "tr")))
            .collect();
        let heights: Vec<_> = rows.iter().map(|n| number(*n, "h", 0.0) / EMU).collect();
        if widths.is_empty()
            || heights.is_empty()
            || widths.iter().chain(&heights).any(|v| *v <= 0.0)
        {
            bail!("表の行列寸法が不正です");
        }
        let style_source = table_styles::load(self.parts, table, self.warnings)?;
        let style_doc = style_source.as_deref().map(Document::parse).transpose()?;
        self.svg.push_str(&format!(
            "<g transform=\"scale({} {})\">",
            w / widths.iter().sum::<f64>(),
            h / heights.iter().sum::<f64>()
        ));
        let cell_count = widths
            .len()
            .checked_mul(rows.len())
            .context("表のセル数が上限を超えています")?;
        if cell_count > 100_000 {
            bail!("表は 100000 セルまでです");
        }
        let mut covered = vec![false; cell_count];
        let mut y = 0.0;
        for (row_index, row) in rows.iter().enumerate() {
            let cells: Vec<_> = row
                .children()
                .filter(|n| n.has_tag_name((A, "tc")))
                .collect();
            if cells.len() != widths.len() {
                bail!("表のセル数と列数が一致しません");
            }
            let mut x = 0.0;
            for (col, cell) in cells.iter().enumerate() {
                self.warnings.target.location =
                    format!("{location}/row/{}/cell/{}", row_index + 1, col + 1);
                let merged = theme::bool_attr(*cell, "hMerge") == Some(true)
                    || theme::bool_attr(*cell, "vMerge") == Some(true);
                if merged != covered[row_index * widths.len() + col] {
                    bail!("結合セルの範囲と継続セルが一致しません");
                }
                if theme::bool_attr(*cell, "hMerge") != Some(true)
                    && theme::bool_attr(*cell, "vMerge") != Some(true)
                {
                    let span = |key: &str| -> Result<usize> {
                        let value = cell.attribute(key).unwrap_or("1").parse::<usize>()?;
                        if value == 0 {
                            bail!("結合セルの範囲は正数が必要です");
                        }
                        Ok(value)
                    };
                    let cols = span("gridSpan")?;
                    let rs = span("rowSpan")?;
                    let ws = widths
                        .get(col..col.checked_add(cols).context("結合列数が不正です")?)
                        .context("結合セルが列の範囲を超えています")?;
                    let hs = heights
                        .get(row_index..row_index.checked_add(rs).context("結合行数が不正です")?)
                        .context("結合セルが行の範囲を超えています")?;
                    for r in row_index..row_index + rs {
                        for c in col..col + cols {
                            let index = r * widths.len() + c;
                            if covered[index] {
                                bail!("結合セルの範囲が重なっています");
                            }
                            if (r, c) != (row_index, col) {
                                covered[index] = true;
                            }
                        }
                    }
                    let cw: f64 = ws.iter().sum();
                    let ch: f64 = hs.iter().sum();
                    self.svg
                        .push_str(&format!("<g transform=\"translate({x} {y})\">"));
                    let styled = style_doc.as_ref().map(|doc| {
                        table_styles::cell(
                            *cell,
                            doc.root_element(),
                            child(table, A, "tblPr"),
                            (row_index, col, rows.len(), widths.len()),
                            self.warnings,
                        )
                    });
                    let styled_doc = styled.as_deref().map(Document::parse).transpose()?;
                    let cell = styled_doc
                        .as_ref()
                        .map(Document::root_element)
                        .unwrap_or(*cell);
                    let props = child(cell, A, "tcPr");
                    let background = props
                        .map(|n| fill(n, "none", self.theme, self.warnings))
                        .unwrap_or("none".into());
                    self.svg.push_str(&format!(
                        "<rect width=\"{cw}\" height=\"{ch}\" fill=\"{background}\"/>"
                    ));
                    if let Some(props) = props {
                        for (edge, x1, y1, x2, y2) in [
                            ("lnL", 0.0, 0.0, 0.0, ch),
                            ("lnR", cw, 0.0, cw, ch),
                            ("lnT", 0.0, 0.0, cw, 0.0),
                            ("lnB", 0.0, ch, cw, ch),
                        ] {
                            if let Some(line) = child(props, A, edge) {
                                let color = fill_line(line, self.theme, self.warnings);
                                let width = number(line, "w", 9525.0) / EMU;
                                let decorations = lines::decorations(
                                    line,
                                    &color,
                                    width,
                                    self.svg,
                                    self.warnings,
                                );
                                self.svg.push_str(&format!("<line {decorations} x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{color}\" stroke-width=\"{width}\"/>"));
                            }
                        }
                        for n in props.children().filter(|n| n.is_element()) {
                            if !matches!(
                                n.tag_name().name(),
                                "solidFill" | "noFill" | "lnL" | "lnR" | "lnT" | "lnB"
                            ) {
                                warn(
                                    self.warnings,
                                    format!("未対応のセル属性: {}", n.tag_name().name()),
                                );
                            }
                        }
                        if props.attribute("vert").is_some_and(|s| s != "horz") {
                            warn(self.warnings, "表の縦書きは未対応です");
                        }
                    }
                    if let Some(body) = child(cell, A, "txBody") {
                        text::render(
                            text::TextBody {
                                body,
                                inherited: vec![],
                                master_style: None,
                                cell_props: props,
                            },
                            cw,
                            ch,
                            self.theme,
                            self.options,
                            self.svg,
                            self.warnings,
                        );
                    }
                    self.svg.push_str("</g>");
                }
                x += widths[col];
            }
            y += heights[row_index];
        }
        self.svg.push_str("</g></g>");
        Ok(())
    }
}
fn transform(t: Node<'_, '_>, svg: &mut String) -> Result<(f64, f64)> {
    let off = child(t, A, "off").context("座標原点がありません")?;
    let ext = child(t, A, "ext").context("座標寸法がありません")?;
    let (x, y, w, h) = (
        number(off, "x", 0.0) / EMU,
        number(off, "y", 0.0) / EMU,
        number(ext, "cx", 0.0) / EMU,
        number(ext, "cy", 0.0) / EMU,
    );
    if w < 0.0 || h < 0.0 {
        bail!("座標寸法が負です");
    }
    let rotation = number(t, "rot", 0.0) / 60000.0;
    let fx = if theme::bool_attr(t, "flipH") == Some(true) {
        -1
    } else {
        1
    };
    let fy = if theme::bool_attr(t, "flipV") == Some(true) {
        -1
    } else {
        1
    };
    svg.push_str(&format!(
        "<g transform=\"translate({} {}) rotate({rotation}) scale({fx} {fy}) translate({} {})\">",
        x + w / 2.0,
        y + h / 2.0,
        -w / 2.0,
        -h / 2.0
    ));
    Ok((w, h))
}
