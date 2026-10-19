//! CLI の結果文書。文字列は JSON の引用規則（YAML 1.2 の部分集合）で保護する。
use pptx2img::{FontFallbackError, SlideReport, StrictModeError};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn quoted(value: &str) -> String {
    serde_json::to_string(value)
        .expect("文字列のシリアライズ")
        .replace('\u{0085}', "\\u0085")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[derive(Default)]
struct Problem {
    slides: BTreeSet<usize>,
    characters: BTreeSet<char>,
}

pub fn completed(folder: &Path, slides: &[SlideReport]) -> String {
    let mut result = format!(
        "status: completed\noutput:\n  created: true\n  folder: {}\n  slides: {}\n",
        quoted(&folder.to_string_lossy()),
        slides.len()
    );
    let mut problems = BTreeMap::<(String, String), Problem>::new();
    for slide in slides {
        for d in &slide.diagnostics {
            if let Some(font) = &d.font
                && font.reason == "glyph_fallback"
            {
                let problem = problems
                    .entry((font.requested_family.clone(), font.actual_family.clone()))
                    .or_default();
                problem.slides.insert(slide.slide);
                if let Some(ch) = d.missing_character {
                    problem.characters.insert(ch);
                }
            }
        }
    }
    if !problems.is_empty() {
        result.push_str("warnings:\n");
    }
    for ((family, substitute), problem) in problems {
        result.push_str(&format!(
            "  - code: FONT_GLYPH_MISSING\n    family: {}\n    substitute_family: {}\n    slides: {}\n    characters:\n",
            quoted(&family), quoted(&substitute), serde_json::to_string(&problem.slides).unwrap()
        ));
        for ch in problem.characters {
            result.push_str(&format!(
                "      - character: {}\n        codepoint: {}\n",
                quoted(&ch.to_string()),
                quoted(&format!("U+{:04X}", ch as u32))
            ));
        }
    }
    result
}

pub fn failed(error: &anyhow::Error, folder: Option<&Path>, created: bool) -> String {
    let mut result = format!("status: failed\noutput:\n  created: {created}\n");
    if created && let Some(folder) = folder {
        result.push_str(&format!(
            "  folder: {}\n",
            quoted(&folder.to_string_lossy())
        ));
    }
    result.push_str("errors:\n");
    if let Some(fonts) = error.downcast_ref::<FontFallbackError>() {
        let mut problems = BTreeMap::<(&str, String), Problem>::new();
        for slide in &fonts.slides {
            for d in &slide.diagnostics {
                let (code, family) = if let Some(font) = &d.font {
                    let code = match font.reason.as_str() {
                        "family_missing" => "FONT_NOT_FOUND",
                        "mapped_family_missing" => "FONT_MAPPING_TARGET_NOT_FOUND",
                        "glyph_fallback" => "FONT_GLYPH_MISSING",
                        "style_approximation" => "FONT_STYLE_MISSING",
                        _ => "FONT_SUBSTITUTION_REQUIRED",
                    };
                    (code, font.requested_family.clone())
                } else if let Some(family) = &d.required_font {
                    (
                        if d.missing_character.is_some() {
                            "FONT_GLYPH_MISSING"
                        } else {
                            "FONT_NOT_FOUND"
                        },
                        family.clone(),
                    )
                } else {
                    continue;
                };
                let problem = problems.entry((code, family)).or_default();
                problem.slides.insert(slide.slide);
                if let Some(ch) = d.missing_character {
                    problem.characters.insert(ch);
                }
            }
        }
        for ((code, family), problem) in problems {
            result.push_str(&format!(
                "  - code: {code}\n    family: {}\n    slides: {}\n",
                quoted(&family),
                serde_json::to_string(&problem.slides).unwrap()
            ));
            if !problem.characters.is_empty() {
                result.push_str("    characters:\n");
                for ch in problem.characters {
                    result.push_str(&format!(
                        "      - character: {}\n        codepoint: {}\n",
                        quoted(&ch.to_string()),
                        quoted(&format!("U+{:04X}", ch as u32))
                    ));
                }
            }
            let suggestion = match code {
                "FONT_GLYPH_MISSING" => {
                    "列挙した文字を収録したフォントを --font-file <path> で追加してください。入手できない場合は依頼者に提供を求めてください。"
                }
                "FONT_STYLE_MISSING" => {
                    "必要な太さ・斜体を含む同書体のフォントを --font-file <path> で追加してください。"
                }
                "FONT_MAPPING_TARGET_NOT_FOUND" => {
                    "--font-map の指定先を確認し、指定先のフォントを --font-file <path> で追加してください。"
                }
                "FONT_SUBSTITUTION_REQUIRED" => {
                    "元の書体を利用する場合は --font-map を外し、必要なフォントを --font-file <path> で追加してください。"
                }
                _ => {
                    "必要なフォントを --font-file <path> で追加してください。入手できない場合は依頼者に提供を求めてください。"
                }
            };
            result.push_str(&format!("    suggestion: {}\n", quoted(suggestion)));
        }
    } else if let Some(strict) = error.downcast_ref::<StrictModeError>() {
        result.push_str("  - code: STRICT_MODE_REJECTED\n    suggestion: 検出された未対応・近似描画を確認してください。\n    diagnostics:\n");
        for slide in &strict.slides {
            for d in &slide.diagnostics {
                result.push_str(&format!(
                    "      - slide: {}\n        message: {}\n",
                    slide.slide,
                    quoted(&d.message)
                ));
            }
        }
    } else {
        result.push_str(&format!("  - code: CONVERSION_FAILED\n    message: {}\n    suggestion: 入力ファイル・出力先・実行条件を確認してください。\n", quoted(&format!("{error:#}"))));
    }
    result
}

pub fn invalid_arguments(message: &str) -> String {
    format!(
        "status: failed\noutput:\n  created: false\nerrors:\n  - code: INVALID_ARGUMENTS\n    message: {}\n    suggestion: --help で引数の形式を確認してください。\n",
        quoted(message)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes_paths_and_multiline_errors_without_yaml_injection() {
        let path = Path::new("日本語: # 出力\nerrors: fake");
        assert_eq!(
            completed(path, &[]),
            "status: completed\noutput:\n  created: true\n  folder: \"日本語: # 出力\\nerrors: fake\"\n  slides: 0\n"
        );
        let result = failed(&anyhow::anyhow!("失敗\n- code: forged"), Some(path), false);
        assert!(result.contains("message: \"失敗\\n- code: forged\""));
        assert!(!result.contains("folder:"));
        assert!(
            failed(&anyhow::anyhow!("I/O"), Some(path), true).contains("created: true\n  folder:")
        );
    }
}
