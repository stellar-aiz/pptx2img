//! Gate for embedded SVG pictures (`asvg:svgBlip`) before they reach the SVG renderer.
//!
//! usvg parses the nested document again and silently draws nothing when it cannot, so
//! every rejection here returns a reason the caller records. Vector data is bounded by
//! bytes and XML node count; the PNG/JPEG pixel policy in `raster` does not apply.
//! SVG translated from EMF/WMF pictures passes the same gate (see `check_generated`).
use roxmltree::{Document, ParsingOptions};

const SVG: &str = "http://www.w3.org/2000/svg";
/// Largest SVG part accepted, in bytes.
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Largest XML node count accepted (elements, text, and other nodes; attributes excluded).
const MAX_NODES: u32 = 100_000;

/// Largest translated metafile SVG accepted, in bytes. It may carry PNG bitmaps that
/// already passed the PNG/JPEG size policy, so it is bounded like one package part.
const MAX_GENERATED_BYTES: usize = 32 * 1024 * 1024;

/// Accept a self-contained, bounded SVG document, or return why it is not rendered.
/// `Ok(true)` means the document contains `<text>`, which may not render (see the caller).
pub(super) fn check(data: &[u8]) -> Result<bool, String> {
    gate(data, MAX_BYTES, false)
}
/// Gate for SVG that pptx2img translated from a metafile: the same node limit, reference
/// rules, and render check, except that `<image>` may carry the PNG data URIs the
/// translator encoded under the PNG/JPEG size policy.
pub(super) fn check_generated(data: &[u8]) -> Result<bool, String> {
    gate(data, MAX_GENERATED_BYTES, true)
}
fn gate(data: &[u8], max_bytes: usize, png_images: bool) -> Result<bool, String> {
    if data.len() > max_bytes {
        return Err(format!(
            "SVG picture exceeds {max_bytes} bytes ({} bytes)",
            data.len()
        ));
    }
    let text = std::str::from_utf8(data)
        .map_err(|_| "Cannot parse SVG picture (not UTF-8 XML)".to_string())?;
    let options = ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_NODES,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(text, options)
        .map_err(|error| format!("Cannot parse SVG picture ({error})"))?;
    if !doc.root_element().has_tag_name((SVG, "svg")) {
        return Err("SVG picture part is not an SVG document".into());
    }
    let mut has_text = false;
    for node in doc.descendants() {
        has_text |= node.has_tag_name((SVG, "text"));
        let foreign = node.attributes().any(|attr| {
            let embedded_png = png_images
                && node.has_tag_name((SVG, "image"))
                && attr.value().starts_with("data:image/png;base64,");
            (attr.name() == "href" && !attr.value().trim_start().starts_with('#') && !embedded_png)
                || references_outside(attr.value())
        }) || node.text().is_some_and(references_outside);
        if foreign {
            return Err("SVG picture references external or embedded resources".into());
        }
    }
    // usvg drops a nested SVG it cannot load without a diagnostic; load it here first.
    resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default())
        .map_err(|error| format!("Cannot render SVG picture ({error})"))?;
    Ok(has_text)
}

/// True when CSS text imports a stylesheet or uses `url(...)` other than a local `#id`.
fn references_outside(value: &str) -> bool {
    if !value.contains(['(', '@']) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    lower.contains("@import")
        || lower.match_indices("url(").any(|(index, _)| {
            !lower[index + 4..]
                .trim_start()
                .trim_start_matches(['"', '\''])
                .starts_with('#')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn svg(body: &str) -> Vec<u8> {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10">{body}</svg>"#).into_bytes()
    }
    #[test]
    fn local_references_are_accepted() {
        let data = svg(
            r##"<defs><linearGradient id="g"/></defs><rect width="10" height="10" fill="url(#g)"/><use href="#g"/><use xlink:href="#g"/><style>.a{fill:url( '#g')}</style>"##,
        );
        assert!(!check(&data).unwrap());
        assert!(check(&svg("<text>a</text>")).unwrap());
    }
    #[test]
    fn outside_references_are_rejected() {
        for body in [
            r#"<image href="https://example.com/a.png"/>"#,
            r#"<image xlink:href="data:image/png;base64,AAAA"/>"#,
            r#"<use href="other.svg#a"/>"#,
            r#"<rect fill="url(https://example.com/#a)"/>"#,
            r#"<rect style="fill: URL(file.svg#a)"/>"#,
            r#"<style>@import "a.css";</style>"#,
        ] {
            assert_eq!(
                check(&svg(body)).err().as_deref(),
                Some("SVG picture references external or embedded resources"),
                "{body}"
            );
        }
    }
    #[test]
    fn non_svg_and_oversized_data_are_rejected() {
        assert!(check(b"\x89PNG\r\n\x1a\n").is_err());
        assert!(check(b"<html/>").is_err());
        let big = svg(&"<g/>".repeat(MAX_NODES as usize));
        assert!(
            check(&big)
                .unwrap_err()
                .starts_with("Cannot parse SVG picture")
        );
        let mut huge = svg("");
        huge.resize(MAX_BYTES + 1, b' ');
        assert!(check(&huge).unwrap_err().starts_with("SVG picture exceeds"));
    }
}
