"""Independent inheritance, group, table, and crop fixtures. python-pptx is needed only for generation."""
from io import BytesIO
from pathlib import Path
import sys
from PIL import Image, ImageDraw
from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.oxml.xmlchemy import OxmlElement
from pptx.util import Inches, Pt


def rect(shapes, x, y, w, h, color):
    shape = shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(x), Inches(y), Inches(w), Inches(h))
    shape.fill.solid()
    shape.fill.fore_color.rgb = RGBColor.from_string(color)
    shape.line.fill.background()
    style = shape._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}style')
    if style is not None:
        shape._element.remove(style)
    # Empty shape-text paragraphs are unnecessary for comparison.
    body = shape._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}txBody')
    if body is not None:
        shape._element.remove(body)
    return shape


def main(out):
    prs = Presentation()
    prs.slide_width, prs.slide_height = Inches(10), Inches(5.625)
    # 1: Group with scaled child coordinates, flipping, and rotation.
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    group = slide.shapes.add_group_shape()
    rect(group.shapes, 1, 1, 1, 1, '245A51')
    rect(group.shapes, 2.5, 1.5, 1.5, 1, 'C37B49')
    group.left, group.top = Inches(1.5), Inches(1)
    group.width, group.height = Inches(6), Inches(3)
    group.rotation = 15
    group._element.grpSpPr.xfrm.set('flipH', '1')
    # 2: Table with explicit backgrounds, borders, and text instead of theme styles.
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    table = slide.shapes.add_table(2, 2, Inches(1), Inches(1), Inches(8), Inches(3)).table
    for child in list(table._tbl.tblPr):
        table._tbl.tblPr.remove(child)
    table._tbl.tblPr.attrib.clear()
    table.cell(0, 0).merge(table.cell(0, 1))
    for r, c, text, color in [(0,0,'結合した見出し','245A51'),(1,0,'企画','C37B49'),(1,1,'制作','245A51')]:
        cell = table.cell(r,c)
        cell.fill.solid(); cell.fill.fore_color.rgb = RGBColor.from_string(color)
        cell.text = text
        p = cell.text_frame.paragraphs[0]
        p.font.name = 'Hiragino Sans'; p.font.size = Pt(24)
        p.font.color.rgb = RGBColor(255,255,255)
        for edge in ['lnL','lnR','lnT','lnB']:
            line = OxmlElement('a:'+edge); line.set('w','12700')
            fill = OxmlElement('a:solidFill'); rgb = OxmlElement('a:srgbClr'); rgb.set('val','FFFFFF')
            fill.append(rgb); line.append(fill); cell._tc.get_or_add_tcPr().append(line)
    # 3: Crop the right side of an image with different colors on each half.
    slide = prs.slides.add_slide(prs.slide_layouts[6])
    image = Image.new('RGB',(400,200),'#C37B49')
    draw = ImageDraw.Draw(image); draw.rectangle((200,0,399,199), fill='#245A51')
    data = BytesIO(); image.save(data,format='PNG'); data.seek(0)
    picture = slide.shapes.add_picture(data,Inches(3),Inches(1),width=Inches(4),height=Inches(3))
    picture.crop_left = .5
    picture.crop_right = picture.crop_top = picture.crop_bottom = 0
    # 4: Master holds shared shapes, layout sets title position, slide holds only text.
    slide = prs.slides.add_slide(prs.slide_layouts[0])
    for shape in list(slide.shapes):
        if shape != slide.shapes.title:
            slide.shapes._spTree.remove(shape._element)
    title = slide.shapes.title
    title.text = '継承したタイトル'
    for run in title.text_frame.paragraphs[0].runs:
        run.font.name = 'Hiragino Sans'; run.font.size = Pt(28)
        run.font.color.rgb = RGBColor.from_string('245A51')
        ea = OxmlElement('a:ea'); ea.set('typeface', 'Hiragino Sans'); run._r.get_or_add_rPr().append(ea)
    # Create a shared band by moving our shape XML into the master's shape tree.
    band = rect(slide.shapes,0,0,10,.25,'245A51')
    band._element.nvSpPr.cNvPr.set('id', '1000')
    prs.slide_master._element.cSld.spTree.append(band._element)
    prs.save(out)


if __name__ == '__main__':
    main(Path(sys.argv[1] if len(sys.argv)>1 else 'examples/composite-shapes.pptx'))
