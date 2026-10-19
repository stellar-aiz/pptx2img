"""Independent fixture for hidden elements (cNvPr hidden). python-pptx is needed only for generation."""
import copy
from pathlib import Path
import sys

from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.util import Inches


def fill(shape, color):
    shape.fill.solid()
    shape.fill.fore_color.rgb = RGBColor.from_string(color)
    shape.line.fill.background()


def hide(shape):
    shape._element.xpath(".//p:cNvPr")[0].set("hidden", "1")


def main(out: Path):
    prs = Presentation()
    prs.slide_width, prs.slide_height = Inches(10), Inches(5.625)
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    # Visible content.
    box = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(0.5), Inches(0.5), Inches(2), Inches(1.5))
    fill(box, "2E75B6")
    # Hidden rectangle covering the visible content area.
    cover = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(3), Inches(0.5), Inches(2), Inches(1.5))
    fill(cover, "C00000")
    hide(cover)
    # Hidden group with a child, plus a visible group with a hidden child.
    group = slide.shapes.add_group_shape()
    fill(group.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(5.5), Inches(0.5), Inches(2), Inches(1.5)), "C00000")
    hide(group)
    mixed = slide.shapes.add_group_shape()
    fill(mixed.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(0.5), Inches(3), Inches(2), Inches(1.5)), "548235")
    bad = mixed.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(3), Inches(3), Inches(2), Inches(1.5))
    fill(bad, "C00000")
    hide(bad)
    # Hidden shape on the slide layout.
    layout = slide.slide_layout
    template = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(5.5), Inches(3), Inches(2), Inches(1.5))
    fill(template, "C00000")
    hide(template)
    layout.shapes._spTree.append(copy.deepcopy(template._element))
    template._element.getparent().remove(template._element)
    prs.save(out)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
