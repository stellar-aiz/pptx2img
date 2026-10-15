//! スライド・レイアウト・マスターを保持し、プレースホルダーを解決する。
use super::*;

pub(super) fn placeholder<'a>(node: Node<'a, 'a>) -> Option<Node<'a, 'a>> {
    node.children()
        .filter(|n| n.is_element())
        .filter_map(|n| child(n, P, "nvPr"))
        .find_map(|n| child(n, P, "ph"))
}
pub(super) fn tree<'a>(root: Node<'a, 'a>) -> Option<Node<'a, 'a>> {
    child(root, P, "cSld").and_then(|n| child(n, P, "spTree"))
}
fn master_kind(kind: &str) -> &str {
    match kind {
        "ctrTitle" => "title",
        "obj" | "subTitle" => "body",
        other => other,
    }
}
pub(super) fn inherited<'a>(node: Node<'a, 'a>, ancestors: &[Node<'a, 'a>]) -> Vec<Node<'a, 'a>> {
    let mut chain = vec![node];
    let Some(mut ph) = placeholder(node) else {
        return chain;
    };
    for ancestor in ancestors {
        let master = ancestor.has_tag_name((P, "sldMaster"));
        let matched = tree(*ancestor)
            .into_iter()
            .flat_map(|n| n.children())
            .find(|n| {
                placeholder(*n).is_some_and(|candidate| {
                    if master {
                        master_kind(candidate.attribute("type").unwrap_or("obj"))
                            == master_kind(ph.attribute("type").unwrap_or("obj"))
                    } else {
                        candidate.attribute("idx").unwrap_or("0")
                            == ph.attribute("idx").unwrap_or("0")
                    }
                })
            });
        if let Some(found) = matched {
            ph = placeholder(found).unwrap();
            chain.push(found);
        }
    }
    chain
}
pub(super) fn chain(parts: &Parts, slide: &str) -> Result<Vec<String>> {
    let mut paths = vec![slide.to_owned()];
    for kind in ["/slideLayout", "/slideMaster"] {
        let rels = relationships(parts, paths.last().unwrap())?;
        if let Some(part) = rels
            .values()
            .find(|p| p.kind.ends_with(kind) && !p.external)
        {
            if paths.contains(&part.target) {
                bail!("レイアウトの参照が循環しています");
            }
            paths.push(part.target.clone());
        } else {
            break;
        }
    }
    Ok(paths)
}
pub(super) fn has_fill(node: Node<'_, '_>) -> bool {
    node.children().any(|n| {
        n.tag_name().namespace() == Some(A)
            && matches!(
                n.tag_name().name(),
                "noFill" | "solidFill" | "gradFill" | "pattFill" | "blipFill" | "grpFill"
            )
    })
}
