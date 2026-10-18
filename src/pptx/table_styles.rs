//! 表スタイルをセルの明示書式へ重ねる。通常の文字配置処理は変更しない。
use super::*;
#[derive(Clone)]
struct Element {
    name: String,
    namespace: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<Element>,
}
impl Element {
    fn from(n: Node<'_, '_>) -> Self {
        Self {
            name: n.tag_name().name().into(),
            namespace: n.tag_name().namespace().unwrap_or(A).into(),
            attrs: n
                .attributes()
                .map(|a| (a.name().into(), a.value().into()))
                .collect(),
            text: n
                .children()
                .filter(|c| c.is_text())
                .filter_map(|c| c.text())
                .collect(),
            children: n
                .children()
                .filter(|c| c.is_element())
                .map(Self::from)
                .collect(),
        }
    }
    fn empty(name: &str) -> Self {
        Self {
            name: name.into(),
            namespace: A.into(),
            attrs: vec![],
            text: String::new(),
            children: vec![],
        }
    }
    fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|c| c.name == name)
    }
    fn child_mut(&mut self, name: &str) -> &mut Self {
        if !self.children.iter().any(|c| c.name == name) {
            self.children.push(Self::empty(name));
        }
        self.children.iter_mut().find(|c| c.name == name).unwrap()
    }
    fn xml(&self) -> String {
        let attrs = self
            .attrs
            .iter()
            .map(|(k, v)| format!(" {k}=\"{}\"", escape(v)))
            .collect::<String>();
        format!(
            "<{} xmlns=\"{}\"{attrs}>{}{}</{}>",
            self.name,
            escape(&self.namespace),
            escape(&self.text),
            self.children.iter().map(Self::xml).collect::<String>(),
            self.name
        )
    }
    /// 上位の属性と子要素が下位の同名項目を置き換える。
    fn overlay(&mut self, other: &Self) {
        for (k, v) in &other.attrs {
            self.attrs.retain(|(key, _)| key != k);
            self.attrs.push((k.clone(), v.clone()));
        }
        for c in &other.children {
            self.children.retain(|old| old.name != c.name);
            self.children.push(c.clone());
        }
    }
}

pub(super) fn load(
    parts: &Parts,
    table: Node<'_, '_>,
    warnings: &mut Warnings,
) -> Result<Option<String>> {
    let Some(props) = child(table, A, "tblPr") else {
        return Ok(None);
    };
    for attr in props.attributes() {
        if !matches!(
            attr.name(),
            "firstRow" | "lastRow" | "firstCol" | "lastCol" | "bandRow" | "bandCol"
        ) {
            warn(warnings, format!("未対応の表全体属性: {}", attr.name()));
        }
    }
    for node in props.children().filter(|n| n.is_element()) {
        if !node.has_tag_name((A, "tableStyleId")) {
            warn(
                warnings,
                format!("未対応の表全体書式: {}", node.tag_name().name()),
            );
        }
    }
    let requested = child(props, A, "tableStyleId").and_then(|n| n.text());
    if requested.is_none() {
        return Ok(None);
    }
    let roots = relationships(parts, "")?;
    let Some(presentation) = roots
        .values()
        .find(|r| r.kind.ends_with("/officeDocument") && !r.external)
    else {
        return Ok(None);
    };
    let rels = relationships(parts, &presentation.target)?;
    let Some(part) = rels
        .values()
        .find(|r| r.kind.ends_with("/tableStyles") && !r.external)
    else {
        warn(
            warnings,
            "表スタイル部品がないためセルの明示書式で代替しました",
        );
        return Ok(None);
    };
    let doc = xml(parts, &part.target)?;
    let id = requested.unwrap();
    let style = doc
        .descendants()
        .find(|n| n.has_tag_name((A, "tblStyle")) && n.attribute("styleId") == Some(id));
    if let Some(style) = style {
        Ok(Some(Element::from(style).xml()))
    } else {
        warn(
            warnings,
            format!("表スタイルID {id} を解決できないためセルの明示書式で代替しました"),
        );
        Ok(None)
    }
}

fn regions<'a>(
    style: Node<'a, 'a>,
    props: Option<Node<'_, '_>>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> Vec<Node<'a, 'a>> {
    let enabled = |key| props.and_then(|p| theme::bool_attr(p, key)) == Some(true);
    let mut names = vec!["wholeTbl"];
    let first_row = enabled("firstRow");
    let last_row = enabled("lastRow");
    let first_col = enabled("firstCol");
    let last_col = enabled("lastCol");
    if enabled("bandRow") && !(first_row && row == 0 || last_row && row + 1 == rows) {
        names.push(if (row - usize::from(first_row)) % 2 == 0 {
            "band1H"
        } else {
            "band2H"
        });
    }
    if enabled("bandCol") && !(first_col && col == 0 || last_col && col + 1 == cols) {
        names.push(if (col - usize::from(first_col)) % 2 == 0 {
            "band1V"
        } else {
            "band2V"
        });
    }
    if first_col && col == 0 {
        names.push("firstCol");
    }
    if last_col && col + 1 == cols {
        names.push("lastCol");
    }
    if first_row && row == 0 {
        names.push("firstRow");
    }
    if last_row && row + 1 == rows {
        names.push("lastRow");
    }
    if first_row && row == 0 && first_col && col == 0 {
        names.push("nwCell");
    }
    if first_row && row == 0 && last_col && col + 1 == cols {
        names.push("neCell");
    }
    if last_row && row + 1 == rows && first_col && col == 0 {
        names.push("swCell");
    }
    if last_row && row + 1 == rows && last_col && col + 1 == cols {
        names.push("seCell");
    }
    names
        .into_iter()
        .filter_map(|name| child(style, A, name))
        .collect()
}

pub(super) fn cell(
    cell: Node<'_, '_>,
    style: Node<'_, '_>,
    props: Option<Node<'_, '_>>,
    position: (usize, usize, usize, usize),
    warnings: &mut Warnings,
) -> String {
    let (row, col, rows, cols) = position;
    let mut properties = Element::empty("tcPr");
    let mut text = Element::empty("rPr");
    for region in regions(style, props, row, col, rows, cols) {
        if let Some(tc) = child(region, A, "tcStyle") {
            if let Some(fill) = child(tc, A, "fill") {
                for n in fill.children().filter(|n| n.is_element()) {
                    properties.children.retain(|c| {
                        !matches!(
                            c.name.as_str(),
                            "solidFill" | "noFill" | "gradFill" | "pattFill"
                        )
                    });
                    properties.children.push(Element::from(n));
                }
            }
            if child(tc, A, "fillRef").is_some() {
                warn(warnings, "表スタイルのfillRefは未対応です");
            }
            if let Some(borders) = child(tc, A, "tcBdr") {
                for (edge, source) in [
                    ("lnL", if col == 0 { "left" } else { "insideV" }),
                    ("lnR", if col + 1 == cols { "right" } else { "insideV" }),
                    ("lnT", if row == 0 { "top" } else { "insideH" }),
                    ("lnB", if row + 1 == rows { "bottom" } else { "insideH" }),
                ] {
                    if let Some(border) = child(borders, A, source).or_else(|| {
                        child(
                            borders,
                            A,
                            match edge {
                                "lnL" => "left",
                                "lnR" => "right",
                                "lnT" => "top",
                                _ => "bottom",
                            },
                        )
                    }) {
                        if let Some(line) = child(border, A, "ln") {
                            let mut line = Element::from(line);
                            line.name = edge.into();
                            properties.children.retain(|c| c.name != edge);
                            properties.children.push(line);
                        } else {
                            warn(warnings, "表スタイルの線参照は未対応です");
                        }
                    }
                }
            }
        }
        if let Some(tx) = child(region, A, "tcTxStyle") {
            let mut inherited = Element::empty("rPr");
            for key in ["b", "i"] {
                if let Some(v) = tx.attribute(key) {
                    if v != "def" {
                        inherited
                            .attrs
                            .push((key.into(), if v == "on" { "1" } else { "0" }.into()));
                    }
                }
            }
            if tx
                .children()
                .any(|n| matches!(n.tag_name().name(), "srgbClr" | "schemeClr" | "sysClr"))
            {
                let mut fill = Element::empty("solidFill");
                fill.children = tx
                    .children()
                    .filter(|n| matches!(n.tag_name().name(), "srgbClr" | "schemeClr" | "sysClr"))
                    .map(Element::from)
                    .collect();
                inherited.children.push(fill);
            }
            if let Some(font) = child(tx, A, "fontRef") {
                let prefix = if font.attribute("idx") == Some("major") {
                    "+mj"
                } else {
                    "+mn"
                };
                for (tag, suffix) in [("latin", "lt"), ("ea", "ea"), ("cs", "cs")] {
                    let mut e = Element::empty(tag);
                    e.attrs
                        .push(("typeface".into(), format!("{prefix}-{suffix}")));
                    inherited.children.push(e);
                }
            }
            text.overlay(&inherited);
        }
    }
    let mut cell = Element::from(cell);
    if let Some(explicit) = cell.child("tcPr") {
        if explicit.children.iter().any(|c| {
            matches!(
                c.name.as_str(),
                "solidFill" | "noFill" | "gradFill" | "pattFill"
            )
        }) {
            properties.children.retain(|c| {
                !matches!(
                    c.name.as_str(),
                    "solidFill" | "noFill" | "gradFill" | "pattFill"
                )
            });
        }
        properties.overlay(explicit);
    }
    *cell.child_mut("tcPr") = properties;
    if let Some(body) = cell.children.iter_mut().find(|c| c.name == "txBody") {
        for paragraph in body.children.iter_mut().filter(|c| c.name == "p") {
            let mut defaults = text.clone();
            if let Some(existing) = paragraph.child("pPr").and_then(|p| p.child("defRPr")) {
                defaults.overlay(existing);
            }
            defaults.name = "defRPr".into();
            *paragraph.child_mut("pPr").child_mut("defRPr") = defaults;
        }
    }
    cell.xml()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn column_bands_and_edge_regions_follow_flags() {
        let source = format!(
            r#"<a:root xmlns:a="{A}"><a:tblPr firstCol="1" lastCol="1" firstRow="1" lastRow="1" bandCol="1"/><a:tblStyle><a:wholeTbl/><a:band1V/><a:band2V/><a:firstCol/><a:lastCol/><a:firstRow/><a:lastRow/><a:nwCell/><a:seCell/></a:tblStyle></a:root>"#
        );
        let doc = Document::parse(&source).unwrap();
        let root = doc.root_element();
        let style = child(root, A, "tblStyle").unwrap();
        let props = child(root, A, "tblPr");
        let names = |r, c| {
            regions(style, props, r, c, 4, 4)
                .iter()
                .map(|n| n.tag_name().name())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(0, 0),
            vec!["wholeTbl", "firstCol", "firstRow", "nwCell"]
        );
        assert_eq!(names(1, 1), vec!["wholeTbl", "band1V"]);
        assert_eq!(names(1, 2), vec!["wholeTbl", "band2V"]);
        assert_eq!(
            names(3, 3),
            vec!["wholeTbl", "lastCol", "lastRow", "seCell"]
        );
    }
}
