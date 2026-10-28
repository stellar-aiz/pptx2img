---
date: 2026-10-22
kind: decision
principles: [lightweightness, transparency]
status: unimplemented
reason: The import and the unified CLI are done (PRs #106 and #108). The convergence order below is agreed but not started.
---

# Import xlsx2pdf as a workspace crate and converge the two stacks: fonts first, then the PDF writer

**Date**: 2026-10-22 (import snapshot), decisions confirmed 2026-10-28
**Status**: Import and CLI integration done; convergence agreed, not started.
**Canonical code**: `Cargo.toml` (`[workspace] members = ["crates/xlsx"]`), `crates/xlsx/` (`rhodopsin-xlsx`), `src/cli_xlsx.rs` (routing, YAML result documents, option rejection), `tests/xlsx_cli.rs`.

## Decision

- The private `stellar-aiz/xlsx2pdf` repository was imported as a snapshot of commit `fa55270` without its git history, as the library crate `rhodopsin-xlsx` under `crates/xlsx`, with its binary removed. The root CLI accepts `.xlsx` input and converts it to PDF only; diagnostics are rendered as the same YAML result documents as PPTX input, with `INVALID_INPUT`, `UNSUPPORTED_FEATURE`, `FONT_NOT_FOUND`, `LAYOUT_FAILED`, `INTERNAL_ERROR` as codes, exit 1 for conversion failure and 2 for argument-class errors. XLSX input always behaves as `--strict`; no new flag was added.
- History, internal links, and investigation notes from the source repository were not imported, and its former consumer tool is out of scope. No confidential material is ever imported, in any form.
- Unfinished state was accepted: the crate's conversion path stops with `rendering_not_implemented` after validation.
- Convergence order: (1) per-element approximate rendering with warnings for XLSX input, so that default and `--strict` diverge as they do for PPTX; (2) shared font discovery and loading, with the font-resolution interface to be designed with the owner; (3) one PDF writer, moving the PPTX backend from hand-rolled Type 3 fonts to the CID-font subset embedding (pdf-writer + subsetter) used by the xlsx crate.
- Japanese test names and test cases are acceptable in the crate. The crate name `rhodopsin-xlsx` is provisional. A temporary increase in CI time was accepted.

## Why

- rhodopsin is public; the source repository was not. A snapshot without history is the only way to guarantee that excluded material never appears in the public history. The import was squashed into one commit after a first attempt was found to contain an excluded term.
- The two stacks duplicate font parsing (skrifa vs ttf-parser), shaping (rustybuzz twice, differently wired), and PDF writing (Type 3 vs CID subset). The xlsx approach is the standard one and produces smaller, more extractable PDFs, so the PPTX side converges on it rather than the reverse. Fonts come first because the PDF writer depends on a shared font representation.
- One CLI with one result-document format was the owner's expectation (`rhodopsin EXCEL_INPUT_FILE --format=pdf`); separate binaries or JSON output for one input type would have split the contract. "Always strict" needs no flag because there is no approximate path yet; introducing `--eager` or similar was rejected as speculative.
- Rejected alternatives: keeping xlsx2pdf as a separate binary (two contracts to maintain); importing with history and scrubbing (cannot be guaranteed on a public repository); keeping the xlsx crate's JSON diagnostics (two formats for one CLI).
