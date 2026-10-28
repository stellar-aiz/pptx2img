"""Generate a minimal width-measurement font without reusing external fonts. Requires fontTools only for generation."""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

builder = FontBuilder(1000, isTTF=True)
names = ['.notdef', 'full', 'half', 'space', 'mark']
builder.setupGlyphOrder(names)
builder.setupCharacterMap({0x3042: 'full', 0x41: 'half', 0x20: 'space', 0x301: 'mark'})
glyphs = {}
for name in names:
    pen = TTGlyphPen(None)
    if name not in ('space', 'mark'):
        pen.moveTo((50, 0))
        pen.lineTo((50, 700))
        pen.lineTo((450, 700))
        pen.lineTo((450, 0))
        pen.closePath()
    glyphs[name] = pen.glyph()
builder.setupGlyf(glyphs)
builder.setupHorizontalMetrics(dict(zip(names, [(1000, 0), (1000, 0), (500, 0), (250, 0), (0, 0)])))
builder.setupHorizontalHeader(ascent=800, descent=-200)
builder.setupNameTable({'familyName': 'Rhodopsin Metrics Test', 'styleName': 'Regular',
                        'uniqueFontIdentifier': 'Rhodopsin Metrics Test',
                        'fullName': 'Rhodopsin Metrics Test', 'psName': 'RhodopsinMetricsTest'})
builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
builder.setupPost()
builder.setupMaxp()
builder.font['head'].created = builder.font['head'].modified = 2082844800
builder.font.recalcTimestamp = False
builder.save(Path(__file__).with_name('metrics.ttf'))
