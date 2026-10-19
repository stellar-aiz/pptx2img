"""Independent fixture for text inside flipped shapes and groups (a:xfrm flipH/flipV).

python-pptx is needed only for generation. Each cell holds the same left-aligned,
top-anchored label so mirrored or rotated glyphs are easy to spot:

- top row: rectangles with no flip, flipH, flipV, and flipH+flipV;
- bottom row: a flipH rightArrow, a flipH group, a flipV group, and a flipH group
  whose child is also flipH (the two flips cancel).

Next to the deck, `<out>.json` lists one comparison region per cell for compare.py
`--regions`, so a mirrored label is caught even when the whole page passes.
"""
import json
from pathlib import Path
import sys

from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.enum.text import MSO_ANCHOR, PP_ALIGN
from pptx.util import Inches, Pt

W, H = Inches(2.0), Inches(1.4)
COLUMNS = [Inches(0.4), Inches(2.75), Inches(5.1), Inches(7.45)]
ROWS = [Inches(0.5), Inches(3.2)]


def flip(xfrm, horizontal=False, vertical=False):
    if horizontal:
        xfrm.set("flipH", "1")
    if vertical:
        xfrm.set("flipV", "1")


def plain_shape(shapes, kind, x, y, w, h):
    """Add a shape without python-pptx's theme style reference (its theme shadow is
    a separate, known unsupported feature and would blur the comparison)."""
    shape = shapes.add_shape(kind, x, y, w, h)
    style = shape._element.find("{http://schemas.openxmlformats.org/presentationml/2006/main}style")
    shape._element.remove(style)
    return shape


def labelled(shapes, kind, x, y, label, color):
    shape = plain_shape(shapes, kind, x, y, W, H)
    shape.fill.solid()
    shape.fill.fore_color.rgb = RGBColor.from_string(color)
    shape.line.color.rgb = RGBColor(0x20, 0x20, 0x20)
    shape.line.width = Pt(2)
    frame = shape.text_frame
    frame.vertical_anchor = MSO_ANCHOR.TOP
    frame.word_wrap = True
    paragraph = frame.paragraphs[0]
    paragraph.alignment = PP_ALIGN.LEFT
    run = paragraph.add_run()
    run.text = label
    run.font.name = "Arial"
    run.font.size = Pt(20)
    run.font.color.rgb = RGBColor(0, 0, 0)
    return shape


def marker(shapes, x, y):
    """A small dark square at the top-left of the group's child space."""
    square = plain_shape(shapes, MSO_SHAPE.RECTANGLE, x, y, Inches(0.3), Inches(0.3))
    square.fill.solid()
    square.fill.fore_color.rgb = RGBColor(0x20, 0x20, 0x20)
    square.line.fill.background()


def flipped_group(slide, x, y, label, horizontal, vertical, child_flip=False):
    group = slide.shapes.add_group_shape()
    child = labelled(group.shapes, MSO_SHAPE.RECTANGLE, x, y, label, "F4B183")
    if child_flip:
        flip(child._element.spPr.xfrm, horizontal=True)
    marker(group.shapes, x, y + H - Inches(0.3))
    flip(group._element.grpSpPr.xfrm, horizontal, vertical)


def main(out: Path):
    prs = Presentation()
    prs.slide_width, prs.slide_height = Inches(10), Inches(5.625)
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    for column, (label, horizontal, vertical) in zip(
        COLUMNS,
        [
            ("Rk 1 none", False, False),
            ("Rk 2 flipH", True, False),
            ("Rk 3 flipV", False, True),
            ("Rk 4 flipHV", True, True),
        ],
    ):
        shape = labelled(slide.shapes, MSO_SHAPE.RECTANGLE, column, ROWS[0], label, "9DC3E6")
        flip(shape._element.spPr.xfrm, horizontal, vertical)
    arrow = labelled(slide.shapes, MSO_SHAPE.RIGHT_ARROW, COLUMNS[0], ROWS[1], "Rk 5 arrow", "A9D18E")
    flip(arrow._element.spPr.xfrm, horizontal=True)
    flipped_group(slide, COLUMNS[1], ROWS[1], "Rk 6 grpH", True, False)
    flipped_group(slide, COLUMNS[2], ROWS[1], "Rk 7 grpV", False, True)
    flipped_group(slide, COLUMNS[3], ROWS[1], "Rk 8 grpH+H", True, False, child_flip=True)
    prs.save(out)
    names = ["none", "flipH", "flipV", "flipHV", "arrow-flipH", "group-flipH", "group-flipV", "group-flipH-child-flipH"]
    cells = [(column, row) for row in ROWS for column in COLUMNS]
    out.with_suffix(".json").write_text(json.dumps({"regions": [
        {"slide": 1, "name": name, "rect": [
            round(x / prs.slide_width, 4), round(y / prs.slide_height, 4),
            round(W / prs.slide_width, 4), round(H / prs.slide_height, 4)]}
        for name, (x, y) in zip(names, cells)
    ]}, indent=2) + "\n")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
