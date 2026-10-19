"""Independent fixture for five presets; contains no assets from provided documents."""
from pathlib import Path
import sys
from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.util import Inches, Pt
from pptx.oxml.xmlchemy import OxmlElement


def main(out):
    prs = Presentation()
    prs.slide_width, prs.slide_height = Inches(10), Inches(5.625)
    kinds = [MSO_SHAPE.PENTAGON, MSO_SHAPE.UP_ARROW, MSO_SHAPE.RIGHT_ARROW,
             MSO_SHAPE.RIGHT_BRACE, MSO_SHAPE.ROUNDED_RECTANGULAR_CALLOUT]
    for adjusted in [False, True]:
        slide = prs.slides.add_slide(prs.slide_layouts[6])
        for i, kind in enumerate(kinds):
            shape = slide.shapes.add_shape(kind, Inches(.4+i*1.9), Inches(1.8), Inches(1.5), Inches(1.2))
            shape.fill.solid(); shape.fill.fore_color.rgb = RGBColor.from_string('245A51')
            shape.line.color.rgb = RGBColor.from_string('C37B49'); shape.line.width = Pt(2)
            style = shape._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}style')
            if style is not None: shape._element.remove(style)
            if adjusted:
                shape.rotation = 20
                shape._element.spPr.xfrm.set('flipH','1')
                if i < 4: shape.adjustments[0] = .3
                else:
                    shape.adjustments[0] = -.35
                    shape.adjustments[1] = .8
            if i == 0:
                shape.text = '工程'
                p = shape.text_frame.paragraphs[0]
                p.font.name = 'Hiragino Sans'; p.font.size = Pt(16)
                p.font.color.rgb = RGBColor(255,255,255)
                for run in p.runs:
                    ea = OxmlElement('a:ea'); ea.set('typeface','Hiragino Sans'); run._r.get_or_add_rPr().append(ea)
            else:
                shape._element.remove(shape._element.txBody)
    prs.save(out)

if __name__ == '__main__':
    main(Path(sys.argv[1]))
