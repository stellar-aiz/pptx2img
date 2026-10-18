"""代替順位検証用。字形・メトリクスを本プロジェクトで生成する。"""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

for family, chars, weight, italic, filename in [
    ('Pptx Latin', 'A ', 400, False, 'latin'),
    ('Pptx CJK', 'A あ', 400, False, 'cjk'),
    ('Pptx CJK', 'A あ', 700, False, 'cjk-bold'),
    ('Pptx CJK', 'A あ', 400, True, 'cjk-italic'),
    ('Pptx Other', 'A あ', 400, False, 'other'),
]:
    builder = FontBuilder(1000, isTTF=True)
    names = ['.notdef'] + [f'u{ord(ch):04x}' for ch in chars]
    builder.setupGlyphOrder(names)
    builder.setupCharacterMap({ord(ch): name for ch, name in zip(chars, names[1:])})
    glyphs = {}
    for name in names:
        pen = TTGlyphPen(None)
        if name != 'u0020':
            pen.moveTo((50, 0)); pen.lineTo((50, 700)); pen.lineTo((450, 700)); pen.lineTo((450, 0)); pen.closePath()
        glyphs[name] = pen.glyph()
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics({n: (1000 if n == 'u3042' else 500, 0) for n in names})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    style = 'Italic' if italic else 'Bold' if weight == 700 else 'Regular'
    builder.setupNameTable({'familyName': family, 'styleName': style, 'uniqueFontIdentifier': filename,
                           'fullName': family + ' ' + style, 'psName': family.replace(' ', '') + '-' + style})
    builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200,
                     usWeightClass=weight, fsSelection=1 if italic else 32 if weight == 700 else 64)
    builder.setupPost(italicAngle=-12 if italic else 0)
    builder.setupMaxp()
    builder.font['head'].macStyle = 2 if italic else 1 if weight == 700 else 0
    builder.font['head'].created = builder.font['head'].modified = 2082844800
    builder.font.recalcTimestamp = False
    builder.save(Path(__file__).with_name(filename + '.ttf'))
