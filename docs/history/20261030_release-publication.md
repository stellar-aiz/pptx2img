---
date: 2026-10-30
kind: decision
principles: [lightweightness, transparency, observability]
---

# Publish the development bundles as GitHub pre-releases

**Date**: 2026-10-30
**Status**: Decided (#116); implemented in `.github/workflows/release.yml`.

## Decision

- A pushed tag `vX.Y.Z` publishes a GitHub Release marked **pre-release** with one ZIP per target, produced unmodified by `crates/xlsx/tools/distribution-bundle.py`, and a `SHA256SUMS` over those ZIP files. This replaces one clause of the [2026-10-20 distribution record](20261020_xlsx-distribution.md): "publishing to GitHub Releases was explicitly excluded". Everything else in that record stays in force.
- The bundle's `manifest.json` keeps `status: development`; the tool and its tests are unchanged. Leaving pre-release is a later decision tied to the implementation state (the XLSX renderer converting workbooks), not to the version number.
- Targets: `aarch64-apple-darwin` (macos-15), `x86_64-pc-windows-msvc` (windows-2025), `x86_64-unknown-linux-gnu` (ubuntu-24.04). The tool builds only for the host, and no Intel macOS runner is used, so `x86_64-apple-darwin` is not published.
- Binaries are unsigned. `SHA256SUMS` proves a download is intact, not who published it; README says so and gives the macOS quarantine workaround.
- A guard job compares the tag with the `rhodopsin` version in `cargo metadata` before anything is built; the tool's own check compares the binary with Cargo metadata, not with the tag. Each target also converts a deck with the binary extracted from its ZIP, with the font setup of `tests/cli.rs`, because the tool only checks start-up and packaging.
- Every action in the release workflow is pinned by commit SHA, and only the publishing job has `contents: write`.

## Why

- A downstream consumer (doggies) needs prebuilt, hash-pinnable binaries for macOS arm64 and Windows x64 without a Rust toolchain; a release page is the plain way to provide them.
- Reusing the bundle tool keeps licence notices, SBOM, and the commit-to-binary link in one place instead of a second packaging path in YAML.
- The 2026-10-20 record kept bundles off GitHub Releases because the product could not convert a workbook; that is still true, so the artifact keeps saying "development" and only the publication channel changes. A pre-release label states the same thing on the release page.
- Signing needs certificates and secrets and was already out of scope; bundling inside doggies is unaffected by quarantine.
- The publishing job can write releases, unlike the read-only test workflow, so its actions are pinned more strictly than `ci.yml`.

Canonical code: `.github/workflows/release.yml`, `crates/xlsx/tools/distribution-bundle.py`.
