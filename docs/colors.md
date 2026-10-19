# Color transforms and theme fills

Apply lumMod/lumOff to HSL luminance in document order. Do not round intermediate
values to 8 bits; convert the final color back to RGB. Preserve hue/saturation
and clamp luminance to 0–1.

Shape fillRef resolves against theme fmtScheme fillStyleLst (indices starting at 1)
and bgFillStyleLst (starting at 1001), passing the reference color as phClr.
Explicit noFill/solidFill wins. Unknown transforms, non-solid theme fills, and
unresolved references produce warnings. This color-fill implementation does not
cover line, effect, or text theme references; line support is documented separately.

Implemented independently from ECMA-376 Part 1 DrawingML and the
[Open XML SDK FillReference description](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.fillreference).
No ibis-office code, tests, or assets were reused.

Fixture: `python tests/visual/make_colors.py /tmp/colors.pptx`.
Historical comparison at 1600px with LibreOffice 26.2.5.2: MAE 0.000589,
changed-pixel ratio 0.422%, within unchanged thresholds.

Local inspection of source pages 6/14 found the pale blue region changed from
RGB (25,163,252) to (210,237,254), closer to reference (209,237,254).
The gray panel fill was reproduced; at that stage, surrounding shadow differences
came from unsupported effectLst. See [later shadow support](shadows.md).
Text and bullet differences are separate issues. Source images were not published.
