"""Rendering comparison against LibreOffice. Differences do not establish which renderer is correct."""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import importlib.metadata
import json
import math
from collections import Counter
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import zipfile
from xml.etree import ElementTree
from xml.sax.saxutils import escape

from PIL import Image, ImageChops, ImageStat
import pypdfium2 as pdfium
import pypdfium2.raw as pdfium_c


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare_images(actual: Image.Image, reference: Image.Image, tolerance: int) -> tuple[dict, Image.Image]:
    if actual.size != reference.size:
        raise ValueError(f"Image dimensions differ: {actual.size} / {reference.size}")
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
    # Matching pixels are black; differing pixels are red by magnitude, enhancing faint differences.
    heatmap = Image.merge("RGB", (maximum.point(lambda x: min(255, x * 4)), Image.new("L", actual.size), Image.new("L", actual.size)))
    return metrics, heatmap


def passes(metrics: dict, max_mae: float, max_changed_ratio: float) -> bool:
    return metrics["mae"] <= max_mae and metrics["changed_ratio"] <= max_changed_ratio


def load_regions(path: Path | None) -> list[dict]:
    """Rectangles are normalized x, y, width, height (0-1). Reject out-of-bounds values instead of correcting them."""
    if path is None:
        return []
    regions = json.loads(path.read_text(encoding="utf-8"))["regions"]
    if not isinstance(regions, list) or not regions:
        raise ValueError("regions must be a nonempty array")
    identities = set()
    for region in regions:
        slide, name, rect = region["slide"], region["name"], region["rect"]
        if type(slide) is not int or slide < 1 or not isinstance(name, str) or not name:
            raise ValueError("Regions require a positive page number and a nonempty name")
        if not isinstance(rect, list) or len(rect) != 4 or any(
            type(v) not in (int, float) or not math.isfinite(v) for v in rect
        ):
            raise ValueError("rect must contain finite [x, y, width, height] values")
        x, y, width, height = rect
        if x < 0 or y < 0 or width <= 0 or height <= 0 or x + width > 1 or y + height > 1:
            raise ValueError("Region lies outside the image")
        if (slide, name) in identities:
            raise ValueError("Region names must be unique within a page")
        identities.add((slide, name))
    return regions


def compare_regions(actual: Image.Image, reference: Image.Image, regions: list[dict],
                    tolerance: int, max_mae: float, max_changed_ratio: float) -> list[dict]:
    results = []
    for region in regions:
        x, y, width, height = region["rect"]
        box = (math.floor(x * actual.width), math.floor(y * actual.height),
               math.ceil((x + width) * actual.width), math.ceil((y + height) * actual.height))
        metrics, _ = compare_images(actual.crop(box), reference.crop(box), tolerance)
        results.append({**region, "pixel_rect": list(box), **metrics,
                        "passed": passes(metrics, max_mae, max_changed_ratio)})
    return results


def diagnostic_summary(diagnostics: list[dict] | None) -> dict:
    # Distinguish missing diagnostics in old manifests from empty diagnostics in new ones.
    if diagnostics is None:
        return {"available": False, "by_code": {}, "has_omissions": None}
    counts = dict(sorted(Counter(record["code"] for record in diagnostics).items()))
    return {"available": True, "by_code": counts, "has_omissions": counts.get("omitted", 0) > 0}


def visible_slides(manifest: dict) -> list[dict]:
    """Slides that have a LibreOffice PDF page. PDF export skips hidden slides, so drop hidden entries
    (schema 3 `hidden`) before pairing by page order. Manifests without the field are used as-is."""
    return [slide for slide in manifest["slides"] if not slide.get("hidden", False)]


def execute(command: list[str], directory: Path, name: str, env: dict) -> str:
    (directory / f"{name}.command.json").write_text(json.dumps(command, ensure_ascii=False, indent=2), encoding="utf-8")
    # Preserve partial logs on timeouts and failures.
    with (directory / f"{name}.stdout.log").open("w") as stdout, (directory / f"{name}.stderr.log").open("w") as stderr:
        result = subprocess.run(command, stdout=stdout, stderr=stderr, env=env, timeout=120, check=False)
    if result.returncode != 0:
        raise RuntimeError(f"{name} failed (exit code {result.returncode}). Logs: {directory}")
    return (directory / f"{name}.stdout.log").read_text(encoding="utf-8", errors="replace").strip()


def find_soffice(value: str | None) -> str:
    candidate = value or os.environ.get("SOFFICE") or shutil.which("soffice")
    if candidate is None and sys.platform == "darwin":
        candidate = "/Applications/LibreOffice.app/Contents/MacOS/soffice"
    if not candidate or not Path(candidate).is_file():
        raise RuntimeError("LibreOffice not found. Specify its executable with --soffice or SOFFICE")
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


DEFAULT_SUBSTITUTION_MAP = Path(__file__).resolve().with_name("font-substitution.json")
SUBSTITUTION_MAP_ENV = "PPTX2IMG_FONT_SUBSTITUTION_MAP"


def default_substitution_map() -> Path:
    """The map used without --font-substitution-map: the environment override, else the macOS table.

    The environment variable lets runs that cannot pass the flag (cargo-driven comparisons and
    scripts that call run() directly) pin the per-OS table chosen by the CI job.
    """
    override = os.environ.get(SUBSTITUTION_MAP_ENV)
    return Path(override) if override else DEFAULT_SUBSTITUTION_MAP


def load_substitutions(path: Path) -> dict[str, str]:
    """Family -> substitute face. Reject malformed maps instead of silently ignoring entries."""
    families = json.loads(path.read_text(encoding="utf-8"))["families"]
    if not isinstance(families, dict) or not families:
        raise ValueError("families must be a nonempty object")
    for family, substitute in families.items():
        if not family or not isinstance(substitute, str) or not substitute:
            raise ValueError("Each substitution requires a nonempty family and substitute")
    return dict(sorted(families.items()))


DRAWINGML = "http://schemas.openxmlformats.org/drawingml/2006/main"
FONT_PARTS = ("ppt/slides/", "ppt/slideLayouts/", "ppt/slideMasters/", "ppt/notesSlides/",
              "ppt/notesMasters/", "ppt/handoutMasters/", "ppt/theme/", "ppt/charts/", "ppt/diagrams/")


def requested_families(deck: Path) -> list[str]:
    """Latin and East Asian typefaces the deck declares (`a:latin` / `a:ea`), sorted.

    Theme references such as `+mn-lt` are skipped because the theme part declares the faces they
    resolve to. Theme faces are listed even when no text uses them, so the list can be longer
    than the set of families actually drawn. Unparsable parts are skipped.
    """
    names: set[str] = set()
    with zipfile.ZipFile(deck) as archive:
        for part in archive.namelist():
            if not part.endswith(".xml") or not part.startswith(FONT_PARTS):
                continue
            try:
                root = ElementTree.fromstring(archive.read(part))
            except ElementTree.ParseError:
                continue
            for tag in ("latin", "ea"):
                for element in root.iter(f"{{{DRAWINGML}}}{tag}"):
                    typeface = element.get("typeface", "").strip()
                    if typeface and not typeface.startswith("+"):
                        names.add(typeface)
    return sorted(names)


def unpinned_families(requested: list[str], substitutions: dict[str, str]) -> list[str]:
    """Requested families the replacement table does not list. LibreOffice matches family names
    case-insensitively, so the comparison ignores case and nothing else."""
    pinned = {family.casefold() for family in substitutions}
    return [family for family in requested if family.casefold() not in pinned]


def compression_profile(enabled: bool, substitutions: dict[str, str] | None = None) -> str:
    # Pin comparison settings without modifying the user's regular profile.
    mode = 1 if enabled else 0
    pairs = ""
    if substitutions:
        # Replacement table with "Always": applies to PDF export, not only to the screen.
        pairs = '<item oor:path="/org.openoffice.Office.Common/Font/Substitution">\n' \
            '<prop oor:name="Replacement" oor:op="fuse"><value>true</value></prop>\n</item>\n'
        for index, (family, substitute) in enumerate(substitutions.items()):
            pairs += (
                f'<item oor:path="/org.openoffice.Office.Common/Font/Substitution/FontPairs">'
                f'<node oor:name="_{index}" oor:op="replace">'
                f'<prop oor:name="Always" oor:op="fuse"><value>true</value></prop>'
                f'<prop oor:name="OnScreenOnly" oor:op="fuse"><value>false</value></prop>'
                f'<prop oor:name="ReplaceFont" oor:op="fuse"><value>{escape(family)}</value></prop>'
                f'<prop oor:name="SubstituteFont" oor:op="fuse"><value>{escape(substitute)}</value></prop>'
                f'</node></item>\n'
            )
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<oor:items xmlns:oor="http://openoffice.org/2001/registry">
<item oor:path="/org.openoffice.Office.Common/AsianLayout">
<prop oor:name="CompressCharacterDistance" oor:op="fuse"><value>{mode}</value></prop>
<prop oor:name="IsKerningWesternTextOnly" oor:op="fuse"><value>true</value></prop>
</item>
{pairs}</oor:items>
'''


def pdf_fonts(document: pdfium.PdfDocument) -> list[dict]:
    """Base font names used by text objects in the reference PDF (subset prefix removed)."""
    found: dict[str, dict] = {}
    for index in range(len(document)):
        page = document[index]
        try:
            for item in page.get_objects():
                if item.type != pdfium_c.FPDF_PAGEOBJ_TEXT:
                    continue
                font = pdfium_c.FPDFTextObj_GetFont(item.raw)
                size = pdfium_c.FPDFFont_GetBaseFontName(font, None, 0)
                buffer = ctypes.create_string_buffer(size)
                pdfium_c.FPDFFont_GetBaseFontName(font, buffer, size)
                name = buffer.value.decode("utf-8", "replace")
                name = name.split("+", 1)[1] if len(name) > 7 and name[6] == "+" else name
                record = found.setdefault(name, {"name": name, "embedded": bool(pdfium_c.FPDFFont_GetIsEmbedded(font)), "pages": []})
                if index + 1 not in record["pages"]:
                    record["pages"].append(index + 1)
        finally:
            page.close()
    return [found[name] for name in sorted(found)]


def run(args: argparse.Namespace) -> bool:
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = {"status": "error", "slides": [], "thresholds": {"max_mae": args.max_mae, "max_changed_ratio": args.max_changed_ratio, "pixel_tolerance": args.pixel_tolerance}}
    try:
        binary, fixture = args.binary.resolve(), args.fixture.resolve()
        region_path = getattr(args, "regions", None)
        regions = load_regions(region_path)
        report["regions"] = regions
        report["reference_limitations"] = [
            "LibreOffice is a comparison reference; no PowerPoint ground-truth images are provided.",
            "The reference may contain rendering differences or overlaps. Passing pixel thresholds does not prove compatibility.",
        ]
        source = Path(__file__).resolve()
        shutil.copyfile(source, out / "comparator.py")
        if region_path is not None:
            shutil.copyfile(region_path, out / "regions.json")
        map_path = (getattr(args, "font_substitution_map", None) or default_substitution_map()).resolve()
        substitutions = load_substitutions(map_path)
        shutil.copyfile(map_path, out / "font-substitution.json")
        requested = requested_families(fixture)
        report["requested_families"] = requested
        report["unpinned_families"] = unpinned_families(requested, substitutions)
        report["font_pinning_strict"] = bool(getattr(args, "strict_font_pinning", False))
        soffice = find_soffice(args.soffice)
        env = os.environ.copy()
        if sys.platform == "darwin":
            # Use the regular CoreText path because svp changes font resolution on macOS.
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
            "punctuation_compression": args.punctuation_compression,
            "libreoffice_compress_character_distance": 1 if args.punctuation_compression else 0,
            "binary_sha256": digest(binary), "fixture_sha256": digest(fixture),
            "libreoffice_binary_sha256": digest(Path(soffice)),
            "comparator_sha256": digest(out / "comparator.py"),
            "regions_sha256": digest(out / "regions.json") if region_path else None,
            "width": args.width,
            "libreoffice_font_substitution": substitutions,
            "font_substitution_sha256": digest(out / "font-substitution.json"),
        }
        # Preserve the input as evidence and pass the same file to both renderers.
        copied = out / "input.pptx"
        shutil.copyfile(fixture, copied)
        command = [str(binary), str(copied), "--out", str(out / "actual"), "--width", str(args.width), "--font-fallback"]
        if args.punctuation_compression:
            command.append("--punctuation-compression")
        execute(command, out, "pptx2img", env)
        manifest = json.loads((out / "actual/manifest.json").read_text(encoding="utf-8"))
        pages = visible_slides(manifest)
        report["hidden_slides"] = {
            "rendered_and_excluded": [slide["slide"] for slide in manifest["slides"] if slide not in pages],
            "skipped_by_renderer": manifest.get("skipped_hidden_slides"),
        }
        report["renderer_fonts"] = manifest.get("fonts")
        if any(region["slide"] > len(pages) for region in regions):
            raise ValueError("Region page number exceeds the slide count")
        reference_dir = out / "reference"
        reference_dir.mkdir()
        profile_xml = compression_profile(args.punctuation_compression, substitutions)
        (out / "libreoffice-profile.xcu").write_text(profile_xml, encoding="utf-8")
        with tempfile.TemporaryDirectory(prefix="pptx2img-lo-profile-") as profile:
            user = Path(profile) / "user"
            user.mkdir()
            (user / "registrymodifications.xcu").write_text(profile_xml, encoding="utf-8")
            execute([soffice, f"-env:UserInstallation={Path(profile).resolve().as_uri()}", "--headless", "--convert-to", "pdf:impress_pdf_Export", "--outdir", str(reference_dir), str(copied)], out, "libreoffice", env)
        pdf_path = reference_dir / "input.pdf"
        if not pdf_path.is_file() or pdf_path.stat().st_size == 0:
            raise RuntimeError("LibreOffice exited successfully but did not produce a PDF")
        (out / "diff").mkdir()
        with pdfium.PdfDocument(pdf_path) as document:
            report["reference_pdf_fonts"] = pdf_fonts(document)
            if len(document) != len(pages) or len(document) == 0:
                raise ValueError(
                    f"Page counts differ: PPTX={len(pages)}, PDF={len(document)}. A renderer older than "
                    "manifest schema 3 draws hidden slides that LibreOffice skips; rebuild pptx2img")
            for index, slide in enumerate(pages):
                name = f"slide-{index + 1:03}.png"
                with Image.open(out / "actual" / slide["image"]) as loaded:
                    actual = loaded.convert("RGB")
                page = document[index]
                try:
                    # Do not hide aspect ratio differences by stretching; allow only 1px of rounding error.
                    expected_height = actual.width * page.get_height() / page.get_width()
                    if abs(expected_height - actual.height) > 1.01:
                        raise ValueError(f"Aspect ratio differs on slide {index + 1}")
                    bitmap = page.render(scale=actual.width / page.get_width())
                    try:
                        reference = bitmap.to_pil().convert("RGB")
                    finally:
                        bitmap.close()
                finally:
                    page.close()
                if abs(reference.width - actual.width) > 1 or abs(reference.height - actual.height) > 1:
                    raise ValueError("Rendered PDF dimensions differ")
                reference = reference.resize(actual.size, Image.Resampling.LANCZOS)
                reference.save(reference_dir / name)
                metrics, heatmap = compare_images(actual, reference, args.pixel_tolerance)
                heatmap.save(out / "diff" / name)
                local = compare_regions(actual, reference,
                    [region for region in regions if region["slide"] == index + 1],
                    args.pixel_tolerance, args.max_mae, args.max_changed_ratio)
                diagnostics = slide.get("diagnostics")
                metrics.update({"slide": index + 1, "deck_slide": slide.get("slide", index + 1), "passed": passes(metrics, args.max_mae, args.max_changed_ratio),
                    "renderer_warnings": slide["warnings"], "renderer_diagnostics": diagnostics,
                    "diagnostic_summary": diagnostic_summary(diagnostics),
                    "regions": local, "regions_passed": all(item["passed"] for item in local) if local else None})
                report["slides"].append(metrics)
        report["global_passed"] = all(slide["passed"] for slide in report["slides"])
        report["regions_passed"] = all(slide["regions_passed"] is not False for slide in report["slides"]) if regions else None
        diagnostics = [record for slide in report["slides"] for record in (slide["renderer_diagnostics"] or [])]
        report["diagnostic_summary"] = diagnostic_summary(diagnostics if all(
            slide["renderer_diagnostics"] is not None for slide in report["slides"]) else None)
        success = report["global_passed"] and report["regions_passed"] is not False
        report["status"] = "passed" if success else "difference_exceeded"
        if report["font_pinning_strict"] and report["unpinned_families"]:
            # Opt-in determinism gate: an unpinned family may fall back differently between runs.
            report["status"] = "unpinned_families"
            success = False
        return success
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        (out / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if report.get("unpinned_families"):
            kind = "error (--strict-font-pinning)" if report.get("font_pinning_strict") else "warning"
            print(f"Font pinning {kind}: {len(report['unpinned_families'])} requested "
                  f"famil{'y' if len(report['unpinned_families']) == 1 else 'ies'} not in the substitution table, "
                  f"so the LibreOffice reference may drift between runs: "
                  f"{', '.join(report['unpinned_families'])}", file=sys.stderr, flush=True)
        print(f"Comparison report: {out / 'report.json'}", flush=True)


def fraction(value: str) -> float:
    number = float(value)
    if not 0 <= number <= 1:
        raise argparse.ArgumentTypeError("Specify a value between 0 and 1")
    return number


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--soffice")
    parser.add_argument("--regions", type=Path, help="JSON with normalized rectangles; compare regions using the whole-page thresholds")
    parser.add_argument("--font-substitution-map", type=Path, help="JSON {\"families\": {family: substitute}} pinned in the LibreOffice profile (default: $PPTX2IMG_FONT_SUBSTITUTION_MAP, else tests/visual/font-substitution.json)")
    parser.add_argument("--strict-font-pinning", action="store_true", help="Fail (exit 1) when the deck requests a latin/ea family that the substitution table does not list; by default these are reported as unpinned_families only")
    parser.add_argument("--punctuation-compression", action="store_true", help="Enable automatic punctuation compression in both engines")
    parser.add_argument("--width", type=int, default=1200, choices=range(64, 4097), metavar="64..4096")
    parser.add_argument("--pixel-tolerance", type=int, default=16, choices=range(256), metavar="0..255")
    parser.add_argument("--max-mae", type=fraction, default=0.02)
    parser.add_argument("--max-changed-ratio", type=fraction, default=0.05)
    args = parser.parse_args()
    try:
        return 0 if run(args) else 1
    except Exception as error:
        print(f"Comparison execution error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
