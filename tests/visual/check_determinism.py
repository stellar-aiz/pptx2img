"""Run the comparison twice with one pinned font substitution map and require identical references.

Prints a Markdown summary (the pinned table, the PDF fonts actually used, and the reference
PNG hashes of both runs) so a CI job can append it to its job summary. Exit status 1 means
the two runs produced different reference images; comparison thresholds are reported but
do not decide the exit status here (the cargo comparison tests own the thresholds).
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

COMPARE = Path(__file__).resolve().with_name("compare.py")


def png_hashes(directory: Path) -> dict[str, str]:
    return {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(directory.glob("slide-*.png"))}


def run_once(args: argparse.Namespace, index: int) -> tuple[Path, int]:
    out = args.out / f"run-{index}"
    command = [sys.executable, str(COMPARE), "--binary", str(args.binary), "--fixture", str(args.fixture),
               "--out", str(out), "--width", str(args.width), "--font-substitution-map", str(args.font_substitution_map)]
    if args.soffice:
        command += ["--soffice", args.soffice]
    status = subprocess.run(command).returncode
    if status == 2:
        raise RuntimeError(f"Comparison run {index} failed to execute")
    return out, status


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--font-substitution-map", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--soffice")
    parser.add_argument("--width", type=int, default=1600)
    args = parser.parse_args()
    first, first_status = run_once(args, 1)
    second, second_status = run_once(args, 2)
    hashes = [png_hashes(first / "reference"), png_hashes(second / "reference")]
    report = json.loads((first / "report.json").read_text(encoding="utf-8"))
    environment = report["environment"]
    identical = bool(hashes[0]) and hashes[0] == hashes[1]
    lines = [f"### Pinned font substitution: `{args.font_substitution_map.name}`", "",
             f"- Fixture: `{args.fixture.name}` at width {args.width}",
             f"- Map SHA-256: `{environment['font_substitution_sha256']}`",
             f"- Reference PNG hashes identical across two runs: **{'yes' if identical else 'NO'}**",
             f"- Comparison exit status (thresholds unchanged): run 1 = {first_status}, run 2 = {second_status}", "",
             "| Replaced family | Pinned substitute |", "|---|---|"]
    lines += [f"| {family} | {substitute} |" for family, substitute in environment["libreoffice_font_substitution"].items()]
    lines += ["", "| Reference PDF font | Embedded | Pages |", "|---|---|---|"]
    lines += [f"| {font['name']} | {font['embedded']} | {font['pages']} |" for font in report["reference_pdf_fonts"]]
    lines += ["", "| Reference page | Run 1 SHA-256 | Run 2 SHA-256 |", "|---|---|---|"]
    lines += [f"| {name} | `{digest}` | `{hashes[1].get(name)}` |" for name, digest in hashes[0].items()]
    print("\n".join(lines))
    return 0 if identical else 1


if __name__ == "__main__":
    sys.exit(main())
