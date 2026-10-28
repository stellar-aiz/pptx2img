# docs

**The design canon is the charter in [`constitution.md`](constitution.md).** Every other document is an application of it, not a canon of its own; when a document and the code disagree, the code is right.

| Place | Content | Frontmatter |
|---|---|---|
| `docs/constitution.md` | The charter. Changed only when a principle changes | none |
| `docs/*.md` | Guides that must be correct for the code as it is: [usage](usage.md), [development](development.md), [rendering](rendering.md), [preset shapes](preset-shapes.md), [colors](colors.md), [shadows](shadows.md), [table styles](table-styles.md), [OLE previews](ole-preview.md), [visual comparison](visual-comparison.md), [XLSX](xlsx.md), [XLSX requirements](xlsx-requirements.md), [XLSX validation](xlsx-validation.md). Fix them when they go stale | `kind: guide` |
| `docs/history/` | Dated records: decisions (what and why) and reports (measurements, surveys). Never updated; may be deleted | `date`, `kind: decision \| report`, `principles`, optional `status`, `reason` |

To add a record, create `docs/history/YYYYMMDD_slug.md` with frontmatter. A decision has two sections, "Decision" and "Why", within about 3 KB, plus pointers to the canonical code. Correct a guide by rewriting it; delete a record whose subject is gone. When a record turns out to disagree with the current code, leave its body alone and add `status: obsolete` (or `unimplemented` for an agreed but unbuilt decision) with a `reason`. Measurement tables and comparison results are not kept in guides; git history holds the numbers.
