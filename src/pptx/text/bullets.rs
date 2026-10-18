//! 箇条書きの排他的な属性継承と、段落レベルごとの番号列。
use super::*;

pub(super) fn choice<'a>(nodes: &[Node<'a, '_>], tags: &[&str]) -> Option<Node<'a, 'a>> {
    nodes.iter().find_map(|node| {
        node.children()
            .find(|n| n.tag_name().namespace() == Some(A) && tags.contains(&n.tag_name().name()))
    })
}
#[derive(Default)]
pub(super) struct Counters([Option<(String, u32)>; 9]);
impl Counters {
    pub fn label(
        &mut self,
        nodes: &[Node<'_, '_>],
        level: usize,
        warnings: &mut Warnings,
    ) -> Option<String> {
        let marker = choice(nodes, &["buNone", "buChar", "buAutoNum", "buBlip"]);
        if marker.is_none_or(|n| !n.has_tag_name((A, "buAutoNum"))) {
            self.0[level] = None;
        }
        self.0[level + 1..].fill(None);
        let marker = marker?;
        match marker.tag_name().name() {
            "buNone" => None,
            "buChar" => marker.attribute("char").map(str::to_owned),
            "buAutoNum" => {
                let kind = marker.attribute("type").unwrap_or("arabicPeriod");
                let next = self.0[level]
                    .as_ref()
                    .filter(|(previous, _)| previous == kind)
                    .map(|(_, number)| number + 1)
                    .unwrap_or(1);
                let restart = self.0[level]
                    .as_ref()
                    .is_none_or(|(previous, _)| previous != kind)
                    || marker.parent().is_some_and(|n| n.has_tag_name((A, "pPr")));
                let number = if let Some(start) = marker.attribute("startAt").filter(|_| restart) {
                    match start
                        .parse::<u32>()
                        .ok()
                        .filter(|n| (1..=32767).contains(n))
                    {
                        Some(n) => n,
                        None => {
                            warn(warnings, "不正な開始番号を1で近似します");
                            1
                        }
                    }
                } else {
                    next
                };
                self.0[level] = Some((kind.into(), number));
                Some(number_label(kind, number, warnings))
            }
            _ => {
                warn(warnings, "画像の箇条書き記号は未対応です");
                None
            }
        }
    }
}
fn number_label(kind: &str, number: u32, warnings: &mut Warnings) -> String {
    let (value, suffix) = if let Some(suffix) = kind.strip_prefix("arabic") {
        (number.to_string(), suffix)
    } else if let Some(suffix) = kind.strip_prefix("alphaLc") {
        (alphabetic(number), suffix)
    } else if let Some(suffix) = kind.strip_prefix("alphaUc") {
        (alphabetic(number).to_uppercase(), suffix)
    } else if let Some(suffix) = kind.strip_prefix("romanLc") {
        (roman(number, warnings).to_lowercase(), suffix)
    } else if let Some(suffix) = kind.strip_prefix("romanUc") {
        (roman(number, warnings), suffix)
    } else {
        warn(
            warnings,
            format!("未対応の番号形式 {kind} を算用数字で近似します"),
        );
        return format!("{number}.");
    };
    match suffix {
        "Period" => format!("{value}."),
        "ParenR" => format!("{value})"),
        "ParenBoth" => format!("({value})"),
        "Plain" => value,
        _ => {
            warn(
                warnings,
                format!("未対応の番号形式 {kind} を算用数字で近似します"),
            );
            format!("{number}.")
        }
    }
}
fn alphabetic(mut number: u32) -> String {
    let mut chars = Vec::new();
    while number > 0 {
        number -= 1;
        chars.push((b'a' + (number % 26) as u8) as char);
        number /= 26;
    }
    chars.into_iter().rev().collect()
}
fn roman(mut number: u32, warnings: &mut Warnings) -> String {
    if number > 3999 {
        warn(warnings, "3999を超えるローマ数字を算用数字で近似します");
        return number.to_string();
    }
    let mut result = String::new();
    for (value, symbol) in [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while number >= value {
            result.push_str(symbol);
            number -= value;
        }
    }
    result
}
pub(super) fn style(
    nodes: &[Node<'_, '_>],
    mut base: Style,
    scale: f64,
    theme: &Theme,
    warnings: &mut Warnings,
) -> Style {
    if let Some(font) = choice(nodes, &["buFontTx", "buFont"])
        && font.has_tag_name((A, "buFont"))
        && let Some(face) = font.attribute("typeface")
    {
        base.face = theme.font(face, warnings);
    }
    if let Some(size) = choice(nodes, &["buSzTx", "buSzPct", "buSzPts"]) {
        match size.tag_name().name() {
            "buSzPct" => {
                let value = number(size, "val", 100000.0);
                if !(25000.0..=400000.0).contains(&value) {
                    warn(warnings, "箇条書き記号の百分率サイズを25〜400%へ制限します");
                }
                base.size *= value.clamp(25000.0, 400000.0) / 100000.0;
            }
            "buSzPts" => {
                let value = number(size, "val", base.size * 75.0 / scale);
                if !(100.0..=400000.0).contains(&value) {
                    warn(warnings, "箇条書き記号のサイズを1〜4000ptへ制限します");
                }
                base.size = value.clamp(100.0, 400000.0) / 75.0 * scale;
            }
            _ => {}
        }
    }
    if let Some(fill) = choice(nodes, &["buClrTx", "buClr"])
        && fill.has_tag_name((A, "buClr"))
    {
        base.color = color(fill, &base.color, theme, warnings);
    }
    base.spacing = 0.0;
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbering_formats_start_and_restart_are_explicit() {
        let mut warnings = Warnings::default();
        for (kind, n, label) in [
            ("arabicPeriod", 3, "3."),
            ("arabicParenR", 4, "4)"),
            ("alphaLcParenBoth", 27, "(aa)"),
            ("romanUcPeriod", 14, "XIV."),
        ] {
            assert_eq!(number_label(kind, n, &mut warnings), label);
        }
        assert!(warnings.is_empty());
        let xml = format!(
            r#"<a:root xmlns:a="{A}"><a:pPr><a:buAutoNum type="arabicPeriod" startAt="3"/></a:pPr><a:pPr><a:buAutoNum type="arabicPeriod"/></a:pPr><a:pPr><a:buNone/></a:pPr><a:lvl1pPr><a:buAutoNum type="arabicPeriod" startAt="5"/></a:lvl1pPr></a:root>"#
        );
        let doc = Document::parse(&xml).unwrap();
        let nodes: Vec<_> = doc
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .collect();
        let mut counters = Counters::default();
        assert_eq!(
            counters.label(&[nodes[0]], 0, &mut warnings).as_deref(),
            Some("3.")
        );
        assert_eq!(
            counters.label(&[nodes[1]], 0, &mut warnings).as_deref(),
            Some("4.")
        );
        assert_eq!(
            counters.label(&[nodes[2], nodes[1]], 0, &mut warnings),
            None
        );
        assert_eq!(
            counters.label(&[nodes[3]], 0, &mut warnings).as_deref(),
            Some("5.")
        );
        assert_eq!(
            counters.label(&[nodes[3]], 0, &mut warnings).as_deref(),
            Some("6.")
        );
        let changed = format!(
            r#"<a:lvl1pPr xmlns:a="{A}"><a:buAutoNum type="alphaUcParenR" startAt="3"/></a:lvl1pPr>"#
        );
        let changed = Document::parse(&changed).unwrap();
        assert_eq!(
            counters
                .label(&[changed.root_element()], 0, &mut warnings)
                .as_deref(),
            Some("C)")
        );
    }
}
