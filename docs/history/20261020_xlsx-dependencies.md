---
date: 2026-10-20
kind: decision
principles: [lightweightness, transparency]
status: obsolete
reason: Written for the standalone xlsx2pdf repository. In the rhodopsin workspace `crates/xlsx/tools/dependency-artifacts.py` reads every workspace package and stops on the root `rhodopsin` crate, which declares no license; the tool must be rewired (select the `rhodopsin-xlsx` package, or declare the root license) before its output is current again.
---

# Dependency licence policy, SBOM, and NOTICE generation for the spreadsheet crate

**Date**: 2026-10-20
**Status**: Accepted policy; the generator does not run in the workspace (see `reason`).
**Canonical code**: `crates/xlsx/tools/dependency-artifacts.py`, `crates/xlsx/tools/test_dependency_artifacts.py` (CI runs these tests).

## Decision

- Generation is a development-time step, never called from the shipped CLI. It uses `cargo metadata --locked --offline` and no network, and refuses an existing output directory.
- Output: CycloneDX 1.6 `sbom.cdx.json`, `NOTICE.txt`, and `licenses/` with the fetched crates' licence texts and author material, untranslated, with each file's SHA-256 recorded in the NOTICE. No dates or machine paths; the same metadata and material give the same output.
- Licence expressions (`AND`, `OR`, `WITH`, parentheses) are parsed. An `OR` with a permissive option passes and records the choice even if another option is copyleft; `AND` needs every term allowed. Unknown identifiers, bad syntax, or missing licence files stop the run. Changing the allow list gets the same review as changing a dependency.
- The list covers everything Cargo resolves for all target OSes and dev-dependencies, not only what a given binary links. Narrowing per distribution target was left for later.
- Automatic collection does not prove redistribution terms are met; per-dependency notice requirements and third-party assets outside the collection are checked before distribution. Fonts are not in this list and need a separate embedding-rights check.

## Why

- The requirement is permissive licences only, with an SBOM and NOTICE covering transitive dependencies. Doing it from `cargo metadata` keeps the check reproducible and offline, and tying allow-list changes to dependency review stops a copyleft dependency from arriving unnoticed.
- The 2026-10-20 output was validated against the official CycloneDX 1.6 JSON schema. The direct dependencies of the crate (image, miniz_oxide, pdf-writer, roxmltree, rustybuzz, serde, serde_json, subsetter, tempfile, unicode-linebreak, unicode-bidi, unicode-script, unicode-bidi-mirroring, zip, rustix on unix) are MIT, Apache-2.0, or dual-licensed; `unicode-linebreak` is Apache-2.0 only.
