"""境界幅の前後を0.01pt刻みで検証する自作資料。生成時のみ python-pptx が必要。"""
import argparse
import json
from pathlib import Path
from pptx import Presentation
from pptx.util import Pt, Inches
from pptx.enum.text import MSO_AUTO_SIZE
from pptx.oxml.xmlchemy import OxmlElement


def make(output, sizes):
    deck = Presentation()
    deck.slide_width, deck.slide_height = Inches(4), Inches(2)
    cases = []
    patterns = [(size, text, size * factor) for size in sizes
                for text, factor in [('あいうえおか', 6), ('あ、いう、え', 5)]]
    if 24 in sizes:
        patterns.append((24, 'あいうえおか', 120))
    for size, text, boundary in patterns:
        for delta in (-.1, -.05, -.02, -.01, 0, .01, .02, .05, .1):
            width = boundary + delta
            slide = deck.slides.add_slide(deck.slide_layouts[6])
            shape = slide.shapes.add_textbox(Pt(24), Pt(15), Pt(width), Pt(110))
            frame = shape.text_frame
            frame.word_wrap, frame.auto_size = True, MSO_AUTO_SIZE.NONE
            frame.margin_left = frame.margin_right = 0
            frame.margin_top = frame.margin_bottom = 0
            para = frame.paragraphs[0]
            para._p.get_or_add_pPr().set('hangingPunct', '0')
            run = para.add_run()
            run.text = text
            run.font.name, run.font.size = 'Hiragino Sans', Pt(size)
            props = run._r.get_or_add_rPr()
            props.set('lang', 'ja-JP')
            east = OxmlElement('a:ea')
            east.set('typeface', 'Hiragino Sans')
            props.append(east)
            cases.append({'slide': len(cases) + 1, 'font_pt': size,
                          'text': text, 'width_pt': width})
    deck.save(output)
    output.with_suffix('.json').write_text(json.dumps(cases, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--sizes', type=float, nargs='+', default=[18, 20, 21.5, 24, 32])
    args = parser.parse_args()
    make(args.output, args.sizes)
