---
date: 2026-10-20
kind: decision
principles: [lightweightness, transparency, observability]
---

# Development distribution bundle: build it yourself, verify it, and never call it a release

**Date**: 2026-10-20
**Status**: Accepted policy; the tool does not run in the workspace (see `reason`).
**Canonical code**: `crates/xlsx/tools/distribution-bundle.py`, `crates/xlsx/tools/test_distribution_bundle.py` (CI runs these tests).

## Decision

- The ZIP is a development artifact. Its manifest `status` is fixed at `development`, and packaging success is never treated as product completion or conversion success.
- The tool runs `cargo build --locked --offline --release` itself and identifies the executable from Cargo's build output. There is no entry point for a user-supplied binary, so a bundle cannot be built from a different commit's executable.
- Requirements: a clean git worktree, a native toolchain for the target (macOS arm64/x86_64, Windows x86_64, Linux x86_64), dependencies fetched beforehand, Python 3 standard library only. Unrecorded changes are refused; the built executable's `--version` and `--help` are run as a start-up check, not an acceptance test.
- Contents under `rhodopsin-<version>-<target>/`: the executable (with Unix permissions), `LICENSE`, `NOTICE.txt` and `licenses/`, `sbom.cdx.json`, `manifest.json` (target, version, source commit, Rust version, `Cargo.lock` hash, every file's hash, size, and mode), and `SHA256SUMS`. Order, timestamps, permissions, and compression are fixed so the same inputs give the same ZIP; cross-OS or cross-compiler binary identity is not promised.
- The ZIP is inspected internally, written to a temporary file in the output directory, and published exclusively without overwriting; `--verify` re-checks the file set, hashes, sizes, permissions, SBOM version, and target name. Signatures and publisher authenticity are out of scope, and Office fonts are never included.

## Why

- A bundle that could be assembled from an arbitrary binary would break the link between the SBOM, the source commit, and the executable; building inside the tool is what makes the manifest trustworthy.
- Fixing `status: development` in the manifest keeps the artifact from being mistaken for a release while the renderer cannot yet convert a workbook; publishing to GitHub Releases was explicitly excluded.
- Real-workbook conversion, Excel comparison, sandbox behaviour, peak memory, and caller integration were listed as acceptance work outside this tool.
