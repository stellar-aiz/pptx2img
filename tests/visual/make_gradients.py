"""Independent fixture for gradient fills (linear, scaled, multi-stop, alpha, path); no source-document assets."""
from pathlib import Path
import sys
from pptx import Presentation
from pptx.enum.shapes import MSO_SHAPE
from pptx.oxml import parse_xml
from pptx.util import Inches
from pptx.dml.color import RGBColor

A = 'http://schemas.openxmlformats.org/drawingml/2006/main'
P = 'http://schemas.openxmlformats.org/presentationml/2006/main'


def stops(*items):
    out = ''
    for pos, color, *alpha in items:
        mod = f'<a:alpha val="{alpha[0]}"/>' if alpha else ''
        out += f'<a:gs pos="{pos}"><a:srgbClr val="{color}">{mod}</a:srgbClr></a:gs>'
    return f'<a:gsLst>{out}</a:gsLst>'


TWO = stops((0, '245A51'), (100000, 'D2EDFE'))
FILLS = [
    TWO + '<a:lin ang="0" scaled="0"/>',
    TWO + '<a:lin ang="5400000" scaled="0"/>',
    TWO + '<a:lin ang="2700000" scaled="0"/>',
    TWO + '<a:lin ang="2700000" scaled="1"/>',
    stops((0, 'C00000'), (50000, 'FFC000'), (100000, '0070C0')) + '<a:lin ang="10800000" scaled="0"/>',
    stops((0, 'C37B49', 20000), (100000, 'C37B49')) + '<a:lin ang="0" scaled="0"/>',
    TWO + '<a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path>',
    TWO + '<a:path path="circle"><a:fillToRect l="100000" t="0" r="0" b="100000"/></a:path>',
]


def shape(slide, x, y, w, h, inner, kind=MSO_SHAPE.RECTANGLE, rotation=0):
    s = slide.shapes.add_shape(kind, Inches(x), Inches(y), Inches(w), Inches(h))
    s._element.remove(s._element.txBody)
    style = s._element.find(f'{{{P}}}style')
    if style is not None:
        s._element.remove(style)
    s._element.spPr.append(parse_xml(f'<a:gradFill xmlns:a="{A}" rotWithShape="1">{inner}</a:gradFill>'))
    s._element.spPr.append(parse_xml(f'<a:ln xmlns:a="{A}"><a:noFill/></a:ln>'))
    s.rotation = rotation


def main(out):
    prs = Presentation()
    prs.slide_width, prs.slide_height = Inches(10), Inches(5.625)
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    # Checkerboard behind the translucent stop.
    for i in range(8):
        r = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(7.4 + (i % 4) * .5), Inches(.3 + (i // 4) * .9), Inches(.25), Inches(1.8))
        r.fill.solid(); r.fill.fore_color.rgb = RGBColor(0, 0, 0); r.line.fill.background()
        r._element.remove(r._element.txBody); r._element.remove(r._element.find(f'{{{P}}}style'))
    for i, inner in enumerate(FILLS):
        shape(slide, .3 + (i % 4) * 2.4, .3 + (i // 4) * 2.5, 2.1, 1.8 if i % 4 != 3 else 1.0, inner)
    # Page 2: a gradient heading band, a rotated chevron, and a gradient slide background.
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    bg = slide._element.find(f'{{{P}}}cSld')
    bg.insert(0, parse_xml(f'<p:bg xmlns:p="{P}" xmlns:a="{A}"><p:bgPr><a:gradFill>{stops((0, "FFFFFF"), (100000, "D2EDFE"))}<a:lin ang="5400000" scaled="0"/></a:gradFill><a:effectLst/></p:bgPr></p:bg>'))
    shape(slide, 0, .4, 10, .8, stops((0, '245A51'), (100000, '4FA394')) + '<a:lin ang="0" scaled="1"/>')
    shape(slide, 1, 2, 3, 2, TWO + '<a:lin ang="0" scaled="0"/>', MSO_SHAPE.CHEVRON, 30)
    shape(slide, 5.5, 2, 3, 2.5, stops((0, 'FFFFFF'), (100000, '245A51')) + '<a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path>', MSO_SHAPE.OVAL)
    prs.save(out)


if __name__ == '__main__':
    main(Path(sys.argv[1]))
