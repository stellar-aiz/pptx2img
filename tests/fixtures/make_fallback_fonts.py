"""代替順位検証用。字形・メトリクスを本プロジェクトで生成する。"""
from pathlib import Path
from zipfile import ZipFile
from xml.etree import ElementTree
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

# 日本語サンプルの構造・色・画像の検証を OS のフォントから独立させる。
# 輪郭は他のテスト書体と同じ矩形であり、日本語字形の忠実度を検証する用途ではない。
sample_chars = set(' ')
for filename in ['basic.pptx', 'theme.pptx']:
    with ZipFile(Path(__file__).resolve().parents[2] / 'examples' / filename) as archive:
        for name in archive.namelist():
            if name.endswith('.xml'):
                root = ElementTree.fromstring(archive.read(name))
                for text in root.iter('{http://schemas.openxmlformats.org/drawingml/2006/main}t'):
                    sample_chars.update(text.text or '')

for family, chars, weight, italic, filename in [
    ('Pptx Samples', ''.join(sorted(sample_chars)), 400, False, 'samples'),
    ('Pptx Latin', 'A ', 400, False, 'latin'),
    ('Pptx CJK', 'A あ', 400, False, 'cjk'),
    ('Pptx CJK', 'A あ', 700, False, 'cjk-bold'),
    ('Pptx CJK', 'A あ', 400, True, 'cjk-italic'),
    ('Pptx Other', 'A あ', 400, False, 'other'),
    ('Pptx Bullets', 'Aあ •.()0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ', 400, False, 'bullets'),
]:
    chars = list(dict.fromkeys(chars))
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
