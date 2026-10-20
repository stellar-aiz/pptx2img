"""Pin key fonts and the LibreOffice version for Japanese fixtures; fail on differences."""
import hashlib
import os
from pathlib import Path
import subprocess
import unicodedata

# Hiragino Sans files shipped on the macos-15 runner image (checked 2026-10-20, run 39366263037).
EXPECTED_FONTS = {
    "ヒラギノ角ゴシック W3.ttc": "833776a6fd68e2c71c0c52fd8041195bd3d0a336cdb278170b7ad71c7e1b3475",
    "ヒラギノ角ゴシック W6.ttc": "790fe535987adc662e83489e162dc54c1e50be8de15169300082f6cc7a082e4e",
}
# Earlier baseline from a previous runner image, kept for the record (no longer accepted):
#   W3 ff92033a300f55aff2f45f57479a560af2c5467973b425d3f724f870af07f83e
#   W6 495faf728470c0111c15265fd5844293fb0bef4240371b79a888371a4ae38c3b


def check():
    fonts = {unicodedata.normalize("NFC", p.name): p for p in Path("/System/Library/Fonts").glob("*.ttc")}
    differences = []
    for name, expected in EXPECTED_FONTS.items():
        path = fonts.get(name)
        actual = "missing" if path is None else hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != expected:
            differences.append(f"{name}: expected {expected}, found {actual}")
    if differences:
        raise RuntimeError("Comparison font differs from the baseline. Check the environment:\n" + "\n".join(differences))
    soffice = os.environ.get("SOFFICE", "/Applications/LibreOffice.app/Contents/MacOS/soffice")
    version = subprocess.check_output([soffice, "--version"], text=True).strip()
    if not version.startswith("LibreOffice 26.2.5.2 "):
        raise RuntimeError(f"Comparison LibreOffice version differs: {version}")
    print("Verified comparison fonts and LibreOffice version")


if __name__ == "__main__":
    check()
