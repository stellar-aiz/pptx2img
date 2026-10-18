"""明度変換とテーマfillRefの自作資料。"""
from pathlib import Path
import sys
from pptx import Presentation
from pptx.enum.shapes import MSO_SHAPE
from pptx.dml.color import RGBColor
from pptx.oxml.xmlchemy import OxmlElement
from pptx.util import Inches

def main(out):
 p=Presentation();p.slide_width,p.slide_height=Inches(10),Inches(5.625)
 slide=p.slides.add_slide(p.slide_layouts[6])
 for row in range(2):
  for i in range(4):
   s=slide.shapes.add_shape(MSO_SHAPE.RECTANGLE,Inches(.6+i*2.4),Inches(.8+row*2),Inches(1.8),Inches(1.4))
   s.line.fill.background();s._element.remove(s._element.txBody)
   style=s._element.find('{http://schemas.openxmlformats.org/presentationml/2006/main}style')
   for e in list(style):style.remove(e)
   fill=OxmlElement('a:fillRef' if row else 'a:solidFill')
   if row:fill.set('idx','1')
   c=OxmlElement('a:schemeClr');c.set('val','accent1');fill.append(c)
   for tag,value in ([('lumMod','50000')],[('lumOff','25000')],[('lumMod','50000'),('lumOff','50000')],[])[i]:
    e=OxmlElement('a:'+tag);e.set('val',value);c.append(e)
   (style if row else s._element.spPr).append(fill)
   if row and i==3:s.fill.solid();s.fill.fore_color.rgb=RGBColor(255,0,0)
 p.save(out)
if __name__=='__main__':main(Path(sys.argv[1]))
