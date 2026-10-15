//! LibreOffice 比較は明示実行時のみ。通常の cargo test は Office を要求しない。
use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
#[ignore = "LibreOffice と tests/visual/requirements.txt の Python 依存が必要"]
fn geometry_matches_libreoffice() {
    compare_fixture("geometry");
}

#[test]
#[ignore = "LibreOffice と日本語フォントが必要"]
fn theme_matches_libreoffice() {
    compare_fixture("theme");
}

#[test]
#[ignore = "LibreOffice と日本語フォントが必要"]
fn text_spacing_and_punctuation_match_libreoffice() {
    compare_fixture("text-theme");
}

#[test]
#[ignore = "LibreOffice と日本語フォントが必要"]
fn explicit_character_spacing_matches_libreoffice() {
    compare_fixture("character-spacing");
}

#[test]
#[ignore = "LibreOffice と日本語フォントが必要"]
fn automatic_punctuation_compression_matches_both_modes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let python = std::env::var_os("PPTX2IMG_TEST_PYTHON").unwrap_or_else(|| "python3".into());
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = root
        .join("target/visual-tests")
        .join(format!("auto-punctuation-{}-{run_id}", std::process::id()));
    let status = Command::new(python)
        .arg(root.join("tests/visual/compare_auto_punctuation.py"))
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_pptx2img"))
        .arg("--fixture")
        .arg(root.join("examples/auto-punctuation.pptx"))
        .arg("--out")
        .arg(&out)
        .status()
        .expect("比較テストを起動できません");
    assert!(
        status.success(),
        "自動圧縮の比較に失敗しました: {}",
        out.display()
    );
}

#[test]
#[ignore = "LibreOffice と日本語フォントが必要"]
fn boundary_widths_match_both_modes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let python = std::env::var_os("PPTX2IMG_TEST_PYTHON").unwrap_or_else(|| "python3".into());
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = root
        .join("target/visual-tests")
        .join(format!("boundary-widths-{}-{run_id}", std::process::id()));
    let status = Command::new(python)
        .arg(root.join("tests/visual/compare_boundary_widths.py"))
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_pptx2img"))
        .arg("--fixture")
        .arg(root.join("examples/boundary-widths.pptx"))
        .arg("--out")
        .arg(&out)
        .status()
        .expect("比較テストを起動できません");
    assert!(
        status.success(),
        "境界幅の比較に失敗しました: {}",
        out.display()
    );
}

#[test]
#[ignore = "LibreOffice と日本語フォントが必要"]
fn punctuation_positions_match_both_modes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let python = std::env::var_os("PPTX2IMG_TEST_PYTHON").unwrap_or_else(|| "python3".into());
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = root.join("target/visual-tests").join(format!(
        "punctuation-positions-{}-{run_id}",
        std::process::id()
    ));
    let status = Command::new(python)
        .arg(root.join("tests/visual/compare_punctuation_positions.py"))
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_pptx2img"))
        .arg("--fixture")
        .arg(root.join("examples/punctuation-positions.pptx"))
        .arg("--out")
        .arg(&out)
        .status()
        .expect("行内位置の比較テストを起動できません");
    assert!(
        status.success(),
        "行内位置の比較に失敗しました: {}",
        out.display()
    );
}

fn compare_fixture(fixture: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let python = std::env::var_os("PPTX2IMG_TEST_PYTHON").unwrap_or_else(|| "python3".into());
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = root
        .join("target/visual-tests")
        .join(format!("{fixture}-{}-{run_id}", std::process::id()));
    let status = Command::new(python)
        .arg(root.join("tests/visual/compare.py"))
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_pptx2img"))
        .arg("--fixture")
        .arg(root.join(format!("examples/{fixture}.pptx")))
        .arg("--out")
        .arg(&out)
        .status()
        .expect("比較テストの Python を起動できません");
    assert!(
        status.success(),
        "LibreOffice 比較に失敗しました。画像・差分・ログ: {}",
        out.display()
    );
}
