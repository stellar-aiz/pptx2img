---
date: 2026-10-20
kind: report
---

# Visual CI baseline per OS and macOS font hashes

Measured by `workflow_dispatch` run 39368259323 on branch `ci/fleet-ci-71` (issue #71): LibreOffice 26.2.5.2, width 1600, default thresholds (MAE 0.02, changed ratio 0.05, pixel tolerance 16). The table covers the comparisons whose `report.json` sits directly under `target/visual-tests/<run>/`; "(over)" marks a slide above the thresholds. A later comparison that is worse than this table is the signal to investigate. The Linux and Windows numbers are not a statement that those OSes render acceptably: the reference pins Noto Sans CJK JP while rhodopsin resolves system fonts per OS, so line widths differ.

| Fixture | Slide | macOS MAE / changed | Linux MAE / changed | Windows MAE / changed |
|---|---|---|---|---|
| `character-spacing` | 1 | 0.0184 / 0.0273 | 0.0331 / 0.0414 (over) | 0.0277 / 0.0354 (over) |
| `character-spacing` | 2 | 0.0172 / 0.0255 | 0.0306 / 0.0387 (over) | 0.0258 / 0.0331 (over) |
| `character-spacing` | 3 | 0.0184 / 0.0274 | 0.0331 / 0.0414 (over) | 0.0277 / 0.0354 (over) |
| `character-spacing` | 4 | 0.0188 / 0.0277 | 0.0337 / 0.0422 (over) | 0.0277 / 0.0355 (over) |
| `character-spacing` | 5 | 0.0124 / 0.0188 | 0.0226 / 0.0286 (over) | 0.0186 / 0.0241 |
| `composite-shapes` | 1 | 0.0003 / 0.0018 | 0.0003 / 0.0018 | 0.0003 / 0.0018 |
| `composite-shapes` | 2 | 0.0030 / 0.0097 | 0.0048 / 0.0123 | 0.0055 / 0.0133 |
| `composite-shapes` | 3 | 0.0009 / 0.0028 | 0.0009 / 0.0028 | 0.0009 / 0.0028 |
| `composite-shapes` | 4 | 0.0012 / 0.0041 | 0.0021 / 0.0051 | 0.0026 / 0.0058 |
| `geometry` | 1 | 0.0017 / 0.0107 | 0.0018 / 0.0115 | 0.0017 / 0.0107 |
| `text-theme` | 1 | 0.0035 / 0.0112 | 0.0055 / 0.0136 | 0.0055 / 0.0136 |
| `text-theme` | 2 | 0.0126 / 0.0306 | 0.0181 / 0.0359 | 0.0181 / 0.0359 |
| `theme` | 1 | 0.0035 / 0.0112 | 0.0055 / 0.0136 | 0.0055 / 0.0136 |

Outcome: macOS 0 slides over; Linux 5 over (all of `character-spacing`); Windows 4 over (`character-spacing` slides 1-4).

## macOS font hashes

`tests/visual/check_environment.py` pins the SHA-256 of the two Hiragino Sans files of the `macos-15` runner image. The image changed once before 2026-10-20; the earlier hashes are kept as a comment in that file.

| File | Earlier image | Image checked 2026-10-20 |
|---|---|---|
| `ヒラギノ角ゴシック W3.ttc` | `ff92033a300f55aff2f45f57479a560af2c5467973b425d3f724f870af07f83e` | `833776a6fd68e2c71c0c52fd8041195bd3d0a336cdb278170b7ad71c7e1b3475` |
| `ヒラギノ角ゴシック W6.ttc` | `495faf728470c0111c15265fd5844293fb0bef4240371b79a888371a4ae38c3b` | `790fe535987adc662e83489e162dc54c1e50be8de15169300082f6cc7a082e4e` |
