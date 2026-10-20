"""Independent fixture for preset shapes and connectors; contains no assets from provided documents."""
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
    more_presets(prs)
    next_tier_presets(prs)
    prs.save(out)


# Presets evaluated from DrawingMLGeometries guides, with non-default adjustments.
PRESETS = [
    ('chevron', [('adj', 30000)]), ('triangle', [('adj', 20000)]),
    ('downArrow', [('adj1', 30000), ('adj2', 70000)]), ('leftArrow', [('adj1', 30000), ('adj2', 70000)]),
    ('leftRightArrow', [('adj1', 70000), ('adj2', 30000)]), ('upDownArrow', [('adj1', 70000), ('adj2', 30000)]),
    ('diamond', []), ('trapezoid', [('adj', 40000)]), ('pie', [('adj1', 0), ('adj2', 10800000)]),
    ('star5', [('adj', 30000), ('hf', 105146), ('vf', 110557)]), ('cloud', []),
    ('round2SameRect', [('adj1', 30000), ('adj2', 10000)]),
    ('wedgeEllipseCallout', [('adj1', 30000), ('adj2', 80000)]),
    ('flowChartAlternateProcess', []), ('flowChartMultidocument', []),
]
# Next tier (callouts, flowchart symbols, arcs, brackets, arrows, stars), with non-default
# adjustments for the adjusted pages. Kinds without adjustments are only rotated and flipped.
NEXT_TIER = [
    ('rtTriangle', []), ('foldedCorner', [('adj', 30000)]),
    ('borderCallout1', [('adj1', 30000), ('adj2', -20000), ('adj3', 120000), ('adj4', -50000)]),
    ('flowChartDocument', []), ('flowChartMagneticDisk', []),
    ('blockArc', [('adj1', 14400000), ('adj2', 3600000), ('adj3', 15000)]),
    ('flowChartTerminator', []), ('wedgeRectCallout', [('adj1', 30000), ('adj2', 80000)]),
    ('donut', [('adj', 40000)]), ('cloudCallout', [('adj1', 30000), ('adj2', 80000)]),
    ('arc', [('adj1', 3600000), ('adj2', 12600000)]),
    ('borderCallout2', [('adj1', 30000), ('adj2', -10000), ('adj3', 40000), ('adj4', -30000),
                        ('adj5', 120000), ('adj6', -50000)]),
    ('can', [('adj', 40000)]), ('flowChartExtract', []), ('snip1Rect', [('adj', 30000)]),
    ('mathPlus', [('adj1', 35000)]), ('leftBracket', [('adj', 20000)]),
    ('upArrowCallout', [('adj1', 35000), ('adj2', 20000), ('adj3', 30000), ('adj4', 55000)]),
    ('downArrowCallout', [('adj1', 35000), ('adj2', 20000), ('adj3', 30000), ('adj4', 55000)]),
    ('rightBracket', [('adj', 20000)]), ('leftBrace', [('adj1', 15000), ('adj2', 35000)]),
    ('bentUpArrow', [('adj1', 35000), ('adj2', 20000), ('adj3', 30000)]),
    ('irregularSeal2', []), ('plus', [('adj', 35000)]),
    ('curvedDownArrow', [('adj1', 40000), ('adj2', 35000), ('adj3', 30000)]),
    ('star7', [('adj', 25000), ('hf', 102572), ('vf', 105210)]),
    ('parallelogram', [('adj', 40000)]),
    ('rightArrowCallout', [('adj1', 35000), ('adj2', 20000), ('adj3', 30000), ('adj4', 55000)]),
    ('flowChartConnector', []), ('mathMultiply', [('adj1', 35000)]),
    ('circularArrow', [('adj1', 20000), ('adj2', 1500000), ('adj3', 19000000),
                       ('adj4', 9000000), ('adj5', 15000)]),
]
CONNECTORS = [
    ('bentConnector3', [('adj1', 30000)]), ('bentConnector4', [('adj1', 30000), ('adj2', 70000)]),
    ('curvedConnector2', []), ('curvedConnector3', [('adj1', 30000)]),
]


def geometry(shape, kind, guides):
    geom = shape._element.spPr.find('{http://schemas.openxmlformats.org/drawingml/2006/main}prstGeom')
    geom.set('prst', kind)
    av = geom.find('{http://schemas.openxmlformats.org/drawingml/2006/main}avLst')
    for child in list(av):
        av.remove(child)
    for name, value in guides:
        gd = OxmlElement('a:gd'); gd.set('name', name); gd.set('fmla', f'val {value}'); av.append(gd)


def more_presets(prs):
    """Pages 3-4: new presets with defaults, then adjusted/rotated/flipped. Page 5: connectors
    (p:cxnSp). Page 6: the same connector geometries as ordinary shapes (p:sp), a control for
    LibreOffice, which reroutes p:cxnSp connectors instead of drawing their preset path."""
    for adjusted in [False, True]:
        slide = prs.slides.add_slide(prs.slide_layouts[6])
        for i, (kind, guides) in enumerate(PRESETS):
            shape = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(.3+(i % 5)*1.9), Inches(.25+(i // 5)*1.8), Inches(1.6), Inches(1.4))
            shape.fill.solid(); shape.fill.fore_color.rgb = RGBColor.from_string('245A51')
            shape.line.color.rgb = RGBColor.from_string('C37B49'); shape.line.width = Pt(2)
            style = shape._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}style')
            if style is not None: shape._element.remove(style)
            geometry(shape, kind, guides if adjusted else [])
            if adjusted:
                shape.rotation = 15
                shape._element.spPr.xfrm.set('flipH' if i % 2 else 'flipV', '1')
            shape._element.remove(shape._element.txBody)
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    for adjusted in [False, True]:
        for i, (kind, guides) in enumerate(CONNECTORS):
            line = slide.shapes.add_connector(1, Inches(.5+i*2.3), Inches(.5+adjusted*2.6), Inches(2.3+i*2.3), Inches(2.5+adjusted*2.6))
            line._element.spPr.find('{http://schemas.openxmlformats.org/drawingml/2006/main}prstGeom').set('prst', kind)
            geometry(line, kind, guides if adjusted else [])
            line.line.color.rgb = RGBColor.from_string('245A51'); line.line.width = Pt(2.5)
            ln = line.line._get_or_add_ln()
            tail = OxmlElement('a:tailEnd'); tail.set('type', 'triangle'); ln.append(tail)
            style = line._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}style')
            if style is not None: line._element.remove(style)
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    for adjusted in [False, True]:
        for i, (kind, guides) in enumerate(CONNECTORS):
            shape = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(.5+i*2.3), Inches(.5+adjusted*2.6), Inches(1.8), Inches(2))
            geometry(shape, kind, guides if adjusted else [])
            shape.fill.background(); shape.line.color.rgb = RGBColor.from_string('245A51'); shape.line.width = Pt(2.5)
            style = shape._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}style')
            if style is not None: shape._element.remove(style)
            shape._element.remove(shape._element.txBody)

def next_tier_presets(prs):
    """Pages 7-10: the next-tier presets at their defaults (7-8), then adjusted, rotated by
    15 degrees, and alternately flipped (9-10). Eighteen shapes per page."""
    for adjusted in [False, True]:
        for start in range(0, len(NEXT_TIER), 18):
            slide = prs.slides.add_slide(prs.slide_layouts[6])
            for i, (kind, guides) in enumerate(NEXT_TIER[start:start+18]):
                shape = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(.25+(i % 6)*1.6), Inches(.2+(i // 6)*1.8), Inches(1.45), Inches(1.3))
                shape.fill.solid(); shape.fill.fore_color.rgb = RGBColor.from_string('245A51')
                shape.line.color.rgb = RGBColor.from_string('C37B49'); shape.line.width = Pt(2)
                style = shape._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}style')
                if style is not None: shape._element.remove(style)
                geometry(shape, kind, guides if adjusted else [])
                if adjusted:
                    shape.rotation = 15
                    shape._element.spPr.xfrm.set('flipH' if i % 2 else 'flipV', '1')
                shape._element.remove(shape._element.txBody)


if __name__ == '__main__':
    main(Path(sys.argv[1]))
