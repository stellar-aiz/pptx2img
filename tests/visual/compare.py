"""LibreOffice を比較対象にした描画テスト。差分は正解・不正解を断定しない。"""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

from PIL import Image, ImageChops, ImageStat
import pypdfium2 as pdfium


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare_images(actual: Image.Image, reference: Image.Image, tolerance: int) -> tuple[dict, Image.Image]:
    if actual.size != reference.size:
        raise ValueError(f"画像寸法が一致しません: {actual.size} / {reference.size}")
    actual, reference = actual.convert("RGB"), reference.convert("RGB")
    difference = ImageChops.difference(actual, reference)
    channels = difference.split()
    maximum = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2])
    changed = maximum.point(lambda value: 255 if value > tolerance else 0)
    pixels = actual.width * actual.height
    metrics = {
        "mae": sum(ImageStat.Stat(difference).mean) / (3 * 255),
        "changed_ratio": changed.histogram()[255] / pixels,
        "pixel_tolerance": tolerance,
        "width": actual.width,
        "height": actual.height,
    }
    # 同じ画素は黒、違う画素は差の強さに応じて赤。弱い差も視認しやすくする。
    heatmap = Image.merge("RGB", (maximum.point(lambda x: min(255, x * 4)), Image.new("L", actual.size), Image.new("L", actual.size)))
    return metrics, heatmap


def passes(metrics: dict, max_mae: float, max_changed_ratio: float) -> bool:
    return metrics["mae"] <= max_mae and metrics["changed_ratio"] <= max_changed_ratio


def execute(command: list[str], directory: Path, name: str, env: dict) -> str:
    (directory / f"{name}.command.json").write_text(json.dumps(command, ensure_ascii=False, indent=2))
    # タイムアウト時も失敗時も、途中までのログを失わない。
    with (directory / f"{name}.stdout.log").open("w") as stdout, (directory / f"{name}.stderr.log").open("w") as stderr:
        result = subprocess.run(command, stdout=stdout, stderr=stderr, env=env, timeout=120, check=False)
    if result.returncode != 0:
        raise RuntimeError(f"{name} が失敗しました（終了コード {result.returncode}）。ログ: {directory}")
    return (directory / f"{name}.stdout.log").read_text(errors="replace").strip()


def find_soffice(value: str | None) -> str:
    candidate = value or os.environ.get("SOFFICE") or shutil.which("soffice")
    if candidate is None and sys.platform == "darwin":
        candidate = "/Applications/LibreOffice.app/Contents/MacOS/soffice"
    if not candidate or not Path(candidate).is_file():
        raise RuntimeError("LibreOffice がありません。--soffice または SOFFICE で実行ファイルを指定してください")
    return str(Path(candidate).resolve())


def font_inventory() -> list[dict]:
    roots = {
        "Darwin": [Path("/System/Library/Fonts"), Path("/Library/Fonts"), Path.home() / "Library/Fonts"],
        "Windows": [Path(os.environ.get("WINDIR", "C:/Windows")) / "Fonts", Path(os.environ.get("LOCALAPPDATA", "")) / "Microsoft/Windows/Fonts"],
    }.get(platform.system(), [Path("/usr/share/fonts"), Path("/usr/local/share/fonts"), Path.home() / ".local/share/fonts", Path.home() / ".fonts"])
    records = []
    for root in roots:
        if root.is_dir():
            for path in sorted(root.rglob("*")):
                if path.suffix.lower() in {".ttf", ".otf", ".ttc"} and path.is_file():
                    records.append({"path": str(path), "sha256": digest(path)})
    return records


def run(args: argparse.Namespace) -> bool:
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = {"status": "error", "slides": [], "thresholds": {"max_mae": args.max_mae, "max_changed_ratio": args.max_changed_ratio, "pixel_tolerance": args.pixel_tolerance}}
    try:
        binary, fixture = args.binary.resolve(), args.fixture.resolve()
        soffice = find_soffice(args.soffice)
        env = os.environ.copy()
        if sys.platform == "darwin":
            # svp は macOS のフォント解決を変えるため、通常の CoreText 経路を使う。
            env["SAL_USE_VCLPLUGIN"] = "osx"
        report["environment"] = {
            "platform": platform.platform(), "python": sys.version,
            "pillow": importlib.metadata.version("Pillow"),
            "pypdfium2": importlib.metadata.version("pypdfium2"),
            "pdfium": str(pdfium.version.PDFIUM_INFO),
            "vcl_plugin": env.get("SAL_USE_VCLPLUGIN"),
            "fonts": font_inventory(),
            "libreoffice": execute([soffice, "--version"], out, "libreoffice-version", env),
            "pptx2img": execute([str(binary), "--version"], out, "pptx2img-version", env),
            "binary_sha256": digest(binary), "fixture_sha256": digest(fixture),
        }
        # 読み込む入力を証跡として固定し、両者に同一ファイルを渡す。
        copied = out / "input.pptx"
        shutil.copyfile(fixture, copied)
        execute([str(binary), str(copied), "--out", str(out / "actual"), "--width", str(args.width)], out, "pptx2img", env)
        manifest = json.loads((out / "actual/manifest.json").read_text())
        reference_dir = out / "reference"
        reference_dir.mkdir()
        with tempfile.TemporaryDirectory(prefix="pptx2img-lo-profile-") as profile:
            execute([soffice, f"-env:UserInstallation={Path(profile).resolve().as_uri()}", "--headless", "--convert-to", "pdf:impress_pdf_Export", "--outdir", str(reference_dir), str(copied)], out, "libreoffice", env)
        pdf_path = reference_dir / "input.pdf"
        if not pdf_path.is_file() or pdf_path.stat().st_size == 0:
            raise RuntimeError("LibreOffice は正常終了しましたが PDF を生成していません")
        (out / "diff").mkdir()
        with pdfium.PdfDocument(pdf_path) as document:
            if len(document) != len(manifest["slides"]) or len(document) == 0:
                raise ValueError(f"ページ数が一致しません: PPTX={len(manifest['slides'])}, PDF={len(document)}")
            for index, slide in enumerate(manifest["slides"]):
                name = f"slide-{index + 1:03}.png"
                with Image.open(out / "actual" / slide["image"]) as loaded:
                    actual = loaded.convert("RGB")
                page = document[index]
                try:
                    # 画像を伸縮して縦横比の不一致を隠さない。丸め誤差の 1px のみ許す。
                    expected_height = actual.width * page.get_height() / page.get_width()
                    if abs(expected_height - actual.height) > 1.01:
                        raise ValueError(f"スライド {index + 1} の縦横比が一致しません")
                    bitmap = page.render(scale=actual.width / page.get_width())
                    try:
                        reference = bitmap.to_pil().convert("RGB")
                    finally:
                        bitmap.close()
                finally:
                    page.close()
                if abs(reference.width - actual.width) > 1 or abs(reference.height - actual.height) > 1:
                    raise ValueError("PDF の描画寸法が一致しません")
                reference = reference.resize(actual.size, Image.Resampling.LANCZOS)
                reference.save(reference_dir / name)
                metrics, heatmap = compare_images(actual, reference, args.pixel_tolerance)
                heatmap.save(out / "diff" / name)
                metrics.update({"slide": index + 1, "passed": passes(metrics, args.max_mae, args.max_changed_ratio), "renderer_warnings": slide["warnings"]})
                report["slides"].append(metrics)
        success = all(slide["passed"] for slide in report["slides"])
        report["status"] = "passed" if success else "difference_exceeded"
        return success
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        (out / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(f"比較結果: {out / 'report.json'}", flush=True)


def fraction(value: str) -> float:
    number = float(value)
    if not 0 <= number <= 1:
        raise argparse.ArgumentTypeError("0〜1 で指定してください")
    return number


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--soffice")
    parser.add_argument("--width", type=int, default=1200, choices=range(64, 4097), metavar="64..4096")
    parser.add_argument("--pixel-tolerance", type=int, default=16, choices=range(256), metavar="0..255")
    parser.add_argument("--max-mae", type=fraction, default=0.02)
    parser.add_argument("--max-changed-ratio", type=fraction, default=0.05)
    args = parser.parse_args()
    try:
        return 0 if run(args) else 1
    except Exception as error:
        print(f"比較テストの実行エラー: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
