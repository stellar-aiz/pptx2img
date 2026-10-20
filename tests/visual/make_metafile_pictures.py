"""Independent fixture: self-made EMF and placeable WMF metafiles.

Every byte is written here with the standard library only (``struct``), following
MS-EMF and MS-WMF; no third-party artwork or metafile is used. Each fixture isolates
one group of records so that a decoder's coverage can be listed record by record:

  emf-shapes      (also emf-shapes-header108: 108-byte header) pens, brushes, rectangle, ellipse, round rectangle, polygon,
                  polyline, MoveToEx/LineTo
  emf-path        BeginPath/EndPath, curves, CloseFigure, FillPath/StrokePath, and a
                  path used as the clip region (SelectClipPath)
  emf-transform   map mode, window/viewport, SetWorldTransform/ModifyWorldTransform,
                  SaveDC/RestoreDC
  emf-clip        IntersectClipRect and ExcludeClipRect
  emf-bitmap      StretchDIBits and BitBlt with an embedded 24-bit DIB
  emf-text        ExtCreateFontIndirectW and ExtTextOutW
  wmf-shapes      placeable WMF: pens, brushes, rectangle, ellipse, polygon, polyline
  wmf-clip-bitmap placeable WMF: IntersectClipRect, StretchDIB, TextOut

Usage:
  make_metafile_pictures.py OUT_DIR            writes the .emf / .wmf files
  make_metafile_pictures.py OUT_DIR --pptx     also writes metafile-pictures.pptx,
                                               one slide per fixture (python-pptx is
                                               needed only for this deck)

The files carry the fixed expectation in the docstrings of the builders below: a
decoder that supports a record draws the described shape inside the picture bounds.
"""
from pathlib import Path
import struct
import sys

# ---------------------------------------------------------------------------
# EMF writer
# ---------------------------------------------------------------------------

EMR_HEADER, EMR_POLYGON, EMR_POLYLINE = 1, 3, 4
EMR_EOF = 14
EMR_SETMAPMODE, EMR_SETBKMODE, EMR_SETPOLYFILLMODE = 17, 18, 19
EMR_SETTEXTALIGN, EMR_SETTEXTCOLOR = 22, 24
EMR_MOVETOEX, EMR_EXCLUDECLIPRECT, EMR_INTERSECTCLIPRECT = 27, 29, 30
EMR_SAVEDC, EMR_RESTOREDC = 33, 34
EMR_SETWORLDTRANSFORM, EMR_MODIFYWORLDTRANSFORM = 35, 36
EMR_SELECTOBJECT, EMR_CREATEPEN, EMR_CREATEBRUSHINDIRECT, EMR_DELETEOBJECT = 37, 38, 39, 40
EMR_ELLIPSE, EMR_RECTANGLE, EMR_ROUNDRECT = 42, 43, 44
EMR_LINETO = 54
EMR_BEGINPATH, EMR_ENDPATH, EMR_CLOSEFIGURE = 59, 60, 61
EMR_FILLPATH, EMR_STROKEANDFILLPATH, EMR_STROKEPATH = 62, 63, 64
EMR_SELECTCLIPPATH = 67
EMR_BITBLT, EMR_STRETCHDIBITS = 76, 81
EMR_EXTCREATEFONTINDIRECTW, EMR_EXTTEXTOUTW = 82, 84
EMR_POLYGON16, EMR_POLYLINE16, EMR_POLYBEZIERTO16, EMR_POLYLINETO16 = 86, 87, 88, 89

STOCK = 0x80000000
NULL_BRUSH, NULL_PEN = STOCK | 5, STOCK | 8
SRCCOPY = 0x00CC0020
MM_ANISOTROPIC = 8
MWT_IDENTITY, MWT_LEFTMULTIPLY, MWT_RIGHTMULTIPLY = 1, 2, 3
RGN_COPY = 5


def rgb(r, g, b):
    """COLORREF is 0x00BBGGRR."""
    return r | (g << 8) | (b << 16)


RED, GREEN, BLUE = rgb(220, 30, 30), rgb(20, 160, 60), rgb(30, 60, 220)
YELLOW, BLACK, GRAY = rgb(250, 210, 30), rgb(0, 0, 0), rgb(150, 150, 150)


def record(kind, payload=b''):
    assert len(payload) % 4 == 0, (kind, len(payload))
    return struct.pack('<II', kind, 8 + len(payload)) + payload


def emf_file(records, bounds, frame_mm100, header_size=108):
    """EMR_HEADER + records + EMR_EOF; `bounds` is device units, `frame` is 0.01 mm.

    `header_size` is 88 (original header), 100 (with the pixel-format fields) or 108
    (adds szlMicrometers); decoders must accept all three.
    """
    records = records + [record(EMR_EOF, struct.pack('<III', 0, 16, 20))]
    body = b''.join(records)
    header = struct.pack(
        '<II4i4iIIIIHHIIIiiii',
        EMR_HEADER, header_size,
        *bounds, *frame_mm100,
        0x464D4520, 0x10000, header_size + len(body), len(records) + 1,
        len(records), 0, 0, 0, 0,  # handle count (upper bound), reserved, no description/palette
        1920, 1080, 508, 286,  # reference device in pixels and millimetres
    )
    if header_size >= 100:
        header += struct.pack('<III', 0, 0, 0)  # cbPixelFormat, offPixelFormat, bOpenGL
    if header_size >= 108:
        header += struct.pack('<ii', 52920, 52920)  # szlMicrometers
    return header + body


# -- object and state records --------------------------------------------

def create_pen(handle, color, width=1, style=0):
    if width > 1:  # a wide pen must be geometric (PS_GEOMETRIC | style); cosmetic pens are 1 wide
        style |= 0x00010000
    return record(EMR_CREATEPEN, struct.pack('<IIiiI', handle, style, width, 0, color))


def create_brush(handle, color, style=0, hatch=0):
    return record(EMR_CREATEBRUSHINDIRECT, struct.pack('<IIII', handle, style, color, hatch))


def select(handle):
    return record(EMR_SELECTOBJECT, struct.pack('<I', handle))


def delete(handle):
    return record(EMR_DELETEOBJECT, struct.pack('<I', handle))


def state(kind, value):
    return record(kind, struct.pack('<i', value))


def rect_record(kind, l, t, r, b):
    return record(kind, struct.pack('<4i', l, t, r, b))


def points16(kind, pts):
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    bounds = struct.pack('<4i', min(xs), min(ys), max(xs), max(ys))
    flat = b''.join(struct.pack('<hh', x, y) for x, y in pts)
    return record(kind, bounds + struct.pack('<I', len(pts)) + flat)


def points32(kind, pts):
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    bounds = struct.pack('<4i', min(xs), min(ys), max(xs), max(ys))
    flat = b''.join(struct.pack('<ii', x, y) for x, y in pts)
    return record(kind, bounds + struct.pack('<I', len(pts)) + flat)


def move_to(x, y):
    return record(EMR_MOVETOEX, struct.pack('<ii', x, y))


def line_to(x, y):
    return record(EMR_LINETO, struct.pack('<ii', x, y))


def xform(m11, m12, m21, m22, dx, dy):
    return struct.pack('<6f', m11, m12, m21, m22, dx, dy)


def set_world_transform(*m):
    return record(EMR_SETWORLDTRANSFORM, xform(*m))


def modify_world_transform(mode, *m):
    return record(EMR_MODIFYWORLDTRANSFORM, xform(*(m or (1, 0, 0, 1, 0, 0))) + struct.pack('<I', mode))


def extent_record(kind, x, y):
    return record(kind, struct.pack('<ii', x, y))


def dib24(width, height, pixel):
    """A bottom-up 24-bit BITMAPINFOHEADER and its pixels; `pixel(x, y)` returns (r, g, b)."""
    stride = (width * 3 + 3) & ~3
    rows = []
    for y in range(height - 1, -1, -1):  # bottom-up
        row = b''.join(bytes(reversed(pixel(x, y))) for x in range(width))  # BGR
        rows.append(row + b'\0' * (stride - len(row)))
    bits = b''.join(rows)
    info = struct.pack('<IiiHHIIiiII', 40, width, height, 1, 24, 0, len(bits), 2835, 2835, 0, 0)
    return info, bits


def test_pattern(x, y):
    """Four quadrants (red, green, blue, yellow) with a black diagonal."""
    if x == y:
        return (0, 0, 0)
    return [[(220, 30, 30), (20, 160, 60)], [(30, 60, 220), (250, 210, 30)]][y // 16][x // 16]


def stretch_dibits(dest, size, info, bits):
    """EMR_STRETCHDIBITS drawing the whole 32x32 source into `dest` (x, y) of `size` (w, h)."""
    fixed = 80
    payload = struct.pack('<4i', dest[0], dest[1], dest[0] + size[0], dest[1] + size[1])
    payload += struct.pack('<6i', dest[0], dest[1], 0, 0, 32, 32)
    payload += struct.pack('<IIII', fixed, len(info), fixed + len(info), len(bits))
    payload += struct.pack('<II', 0, SRCCOPY)  # DIB_RGB_COLORS
    payload += struct.pack('<ii', *size)
    return record(EMR_STRETCHDIBITS, payload + info + bits)


def bit_blt(dest, size, info, bits):
    """EMR_BITBLT copying a 1:1 source (the DIB must be `size` pixels)."""
    fixed = 100
    payload = struct.pack('<4i', dest[0], dest[1], dest[0] + size[0], dest[1] + size[1])
    payload += struct.pack('<4i', dest[0], dest[1], *size)
    payload += struct.pack('<I', SRCCOPY)
    payload += struct.pack('<ii', 0, 0)
    payload += xform(1, 0, 0, 1, 0, 0)
    payload += struct.pack('<II', 0, 0)  # background colour, DIB_RGB_COLORS
    payload += struct.pack('<IIII', fixed, len(info), fixed + len(info), len(bits))
    return record(EMR_BITBLT, payload + info + bits)


def create_font(handle, face='Arial', height=-28, weight=400):
    logfont = struct.pack('<5i8B', height, 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 0, 0)
    logfont += face.encode('utf-16-le').ljust(64, b'\0')
    return record(EMR_EXTCREATEFONTINDIRECTW, struct.pack('<I', handle) + logfont)


def ext_text_out(x, y, text, advance=16):
    raw = text.encode('utf-16-le')
    raw += b'\0' * (-len(raw) % 4)
    fixed = 76
    dx = b''.join(struct.pack('<i', advance) for _ in text)
    emrtext = struct.pack('<iiIII4iI', x, y, len(text), fixed, 0, 0, 0, 0, 0, fixed + len(raw))
    payload = struct.pack('<4i', 0, 0, 0, 0) + struct.pack('<Iff', 1, 1.0, 1.0) + emrtext
    return record(EMR_EXTTEXTOUTW, payload + raw + dx)


# -- the fixtures ------------------------------------------------------------

BOUNDS = (0, 0, 199, 199)  # a 200 x 200 logical canvas
FRAME = (0, 0, 5292, 5292)  # about 52.9 mm square, 0.01 mm units


def emf_shapes(header_size=88):
    """Red rectangle, green ellipse, blue triangle (POLYGON16), black zig-zag (POLYLINE16),
    a yellow round rectangle with a dashed gray hairline outline, and a MoveToEx/LineTo cross."""
    r = [create_pen(1, BLACK, 3), create_brush(2, RED), select(1), select(2),
         rect_record(EMR_RECTANGLE, 10, 10, 90, 60),
         create_brush(3, GREEN), select(3), rect_record(EMR_ELLIPSE, 110, 10, 190, 60),
         create_brush(4, BLUE), select(4),
         points16(EMR_POLYGON16, [(50, 80), (90, 150), (10, 150)]),
         select(NULL_BRUSH), points16(EMR_POLYLINE16, [(110, 150), (130, 90), (150, 150), (170, 90), (190, 150)]),
         create_pen(5, GRAY, 1, 1), create_brush(6, YELLOW), select(5), select(6),
         record(EMR_ROUNDRECT, struct.pack('<6i', 10, 160, 90, 195, 16, 16)),
         create_pen(7, BLACK, 2), select(7), move_to(110, 160), line_to(190, 195),
         move_to(190, 160), line_to(110, 195),
         points32(EMR_POLYGON, [(100, 100), (104, 100), (104, 104)])]  # 32-bit variant
    return emf_file(r, BOUNDS, FRAME, header_size)


def emf_path():
    """A red-filled closed path (curves and a line), a blue stroked open path, and
    a green fill clipped to a circular clip path."""
    bez = [(60, 20), (10, 20), (10, 80)]
    r = [create_pen(1, BLACK, 3), create_brush(2, RED), select(1), select(2),
         record(EMR_BEGINPATH), move_to(60, 40), points16(EMR_POLYBEZIERTO16, bez),
         points16(EMR_POLYLINETO16, [(60, 110)]), points16(EMR_POLYBEZIERTO16, [(110, 80), (110, 20), (60, 40)]),
         record(EMR_CLOSEFIGURE), record(EMR_ENDPATH),
         record(EMR_STROKEANDFILLPATH, struct.pack('<4i', 0, 0, 199, 199)),
         create_pen(3, BLUE, 5), select(3),
         record(EMR_BEGINPATH), move_to(130, 20), line_to(190, 20), line_to(190, 80), line_to(150, 80),
         record(EMR_ENDPATH), record(EMR_STROKEPATH, struct.pack('<4i', 0, 0, 199, 199)),
         record(EMR_SAVEDC),
         record(EMR_BEGINPATH), rect_record(EMR_ELLIPSE, 120, 110, 190, 190), record(EMR_ENDPATH),
         record(EMR_SELECTCLIPPATH, struct.pack('<I', RGN_COPY)),
         create_brush(4, GREEN), select(4), select(NULL_PEN),
         rect_record(EMR_RECTANGLE, 100, 100, 160, 200),  # only the part inside the circle shows
         record(EMR_RESTOREDC, struct.pack('<i', -1))]
    return emf_file(r, BOUNDS, FRAME)


def emf_transform():
    """Window 0..1000 square mapped to the canvas (MM_ANISOTROPIC); a red square at the
    origin, then the same square rotated 30 degrees about (500, 500) (blue), then scaled
    and translated (green). Without the transforms, all three squares overlap."""
    import math
    c, s = math.cos(math.radians(30)), math.sin(math.radians(30))
    r = [state(EMR_SETMAPMODE, MM_ANISOTROPIC),
         extent_record(9, 1000, 1000), extent_record(10, 0, 0),  # window ext / org
         extent_record(11, 200, 200), extent_record(12, 0, 0),  # viewport ext / org
         create_pen(1, BLACK, 8), create_brush(2, RED), select(1), select(2),
         rect_record(EMR_RECTANGLE, 50, 50, 350, 350),
         record(EMR_SAVEDC),
         set_world_transform(c, s, -s, c, 500 - 500 * c + 500 * s, 500 - 500 * s - 500 * c),
         create_brush(3, BLUE), select(3), rect_record(EMR_RECTANGLE, 400, 100, 700, 400),
         record(EMR_RESTOREDC, struct.pack('<i', -1)),
         record(EMR_SAVEDC),
         set_world_transform(1, 0, 0, 1, 0, 0),
         modify_world_transform(MWT_LEFTMULTIPLY, 0.5, 0, 0, 0.5, 600, 600),
         create_brush(4, GREEN), select(4), rect_record(EMR_RECTANGLE, 0, 0, 600, 600),
         modify_world_transform(MWT_IDENTITY),
         record(EMR_RESTOREDC, struct.pack('<i', -1))]
    return emf_file(r, BOUNDS, FRAME)


def emf_clip():
    """A gray backdrop; a red rectangle clipped to a window (IntersectClipRect) and a blue
    rectangle with a hole cut out of it (ExcludeClipRect). MS-EMF expects the hole; in a
    survey neither LibreOffice nor the surveyed decoders drew it, so treat that part as
    an unverified expectation."""
    r = [create_pen(1, BLACK, 2), create_brush(2, GRAY), select(1), select(2),
         rect_record(EMR_RECTANGLE, 5, 5, 195, 195),
         record(EMR_SAVEDC), rect_record(EMR_INTERSECTCLIPRECT, 30, 30, 100, 100),
         create_brush(3, RED), select(3), rect_record(EMR_RECTANGLE, 10, 10, 160, 160),
         record(EMR_RESTOREDC, struct.pack('<i', -1)),
         record(EMR_SAVEDC), rect_record(EMR_EXCLUDECLIPRECT, 130, 130, 170, 170),
         create_brush(4, BLUE), select(4), select(NULL_PEN),
         rect_record(EMR_RECTANGLE, 110, 110, 195, 195),
         record(EMR_RESTOREDC, struct.pack('<i', -1))]
    return emf_file(r, BOUNDS, FRAME)


def emf_bitmap():
    """A 32x32 test pattern stretched to 80x80 (StretchDIBits) and copied 1:1 (BitBlt)."""
    info, bits = dib24(32, 32, test_pattern)
    r = [stretch_dibits((10, 10), (80, 80), info, bits),
         bit_blt((120, 10), (32, 32), info, bits),
         stretch_dibits((100, 100), (90, 90), info, bits)]
    return emf_file(r, BOUNDS, FRAME)


def emf_text():
    """Blue 'EMF text' in Arial and a red bold line below (ExtTextOutW, UTF-16)."""
    r = [create_font(1, 'Arial', -28), select(1), state(EMR_SETBKMODE, 1),
         state(EMR_SETTEXTCOLOR, BLUE), state(EMR_SETTEXTALIGN, 0),
         ext_text_out(10, 40, 'EMF text', 15),
         create_font(2, 'Arial', -24, 700), select(2), state(EMR_SETTEXTCOLOR, RED),
         ext_text_out(10, 100, 'Bold line', 14)]
    return emf_file(r, BOUNDS, FRAME)


# -- WMF (placeable) -----------------------------------------------------------

def wmf_record(function, params=b''):
    assert len(params) % 2 == 0
    return struct.pack('<IH', 3 + len(params) // 2, function) + params


def wmf_file(records, bbox=(0, 0, 200, 200), inch=96):
    objects = sum(1 for r in records if struct.unpack_from('<H', r, 4)[0] in (0x02FA, 0x02FB, 0x02FC))
    records = records + [wmf_record(0x0000)]
    body = b''.join(records)
    max_record = max(len(r) for r in records) // 2
    metaheader = struct.pack('<HHHIHIH', 1, 9, 0x0300, 9 + len(body) // 2, objects, max_record, 0)
    key = struct.pack('<IH4hHI', 0x9AC6CDD7, 0, *bbox, inch, 0)
    checksum = 0
    for (word,) in struct.iter_unpack('<H', key):
        checksum ^= word
    return key + struct.pack('<H', checksum) + metaheader + body


def w_pen(color, width=2, style=0):
    return wmf_record(0x02FA, struct.pack('<HhhI', style, width, 0, color))


def w_brush(color, style=0, hatch=0):
    return wmf_record(0x02FC, struct.pack('<HIH', style, color, hatch))


def w_select(index):
    return wmf_record(0x012D, struct.pack('<H', index))


def w_points(function, pts):
    return wmf_record(function, struct.pack('<H', len(pts)) + b''.join(struct.pack('<hh', *p) for p in pts))


def wmf_shapes():
    """Placeable WMF (window 0..200): red rectangle, green ellipse, blue triangle, black zig-zag."""
    r = [wmf_record(0x020B, struct.pack('<hh', 0, 0)), wmf_record(0x020C, struct.pack('<hh', 200, 200)),
         w_pen(BLACK, 3), w_select(0), w_brush(RED), w_select(1),
         wmf_record(0x041B, struct.pack('<4h', 60, 90, 10, 10)),  # bottom, right, top, left
         w_brush(GREEN), w_select(2), wmf_record(0x0418, struct.pack('<4h', 60, 190, 10, 110)),
         w_brush(BLUE), w_select(3), w_points(0x0324, [(50, 80), (90, 150), (10, 150)]),
         w_select(0), w_points(0x0325, [(110, 150), (130, 90), (150, 150), (170, 90), (190, 150)])]
    return wmf_file(r)


def wmf_clip_bitmap():
    """Placeable WMF: a 32x32 pattern drawn with StretchDIB into a clip window, then TextOut."""
    info, bits = dib24(32, 32, test_pattern)
    dib = wmf_record(0x0F43, struct.pack('<IH8h', SRCCOPY, 0, 32, 32, 0, 0, 90, 90, 10, 10) + info + bits)
    face = 'Arial'.encode('ascii').ljust(32, b'\0')
    font = wmf_record(0x02FB, struct.pack('<5h8B', -28, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0) + face)
    text = 'WMF text'.encode('ascii')
    r = [wmf_record(0x020B, struct.pack('<hh', 0, 0)), wmf_record(0x020C, struct.pack('<hh', 200, 200)),
         wmf_record(0x0416, struct.pack('<4h', 70, 70, 20, 20)),  # clip to (20,20)-(70,70)
         dib,
         wmf_record(0x0416, struct.pack('<4h', 200, 200, 0, 0)),
         wmf_record(0x0209, struct.pack('<I', BLUE)), wmf_record(0x0102, struct.pack('<H', 1)),
         font, w_select(0),
         wmf_record(0x0521, struct.pack('<H', len(text)) + text + struct.pack('<hh', 150, 10))]
    return wmf_file(r)


FIXTURES = [
    ('emf-shapes.emf', emf_shapes, 'EMF shapes: pens, brushes, polygons, polylines'),
    ('emf-shapes-header108.emf', lambda: emf_shapes(108), 'EMF shapes, 108-byte header'),
    ('emf-path.emf', emf_path, 'EMF paths: fill, stroke, curves, clip path'),
    ('emf-transform.emf', emf_transform, 'EMF map mode and world transforms'),
    ('emf-clip.emf', emf_clip, 'EMF IntersectClipRect / ExcludeClipRect'),
    ('emf-bitmap.emf', emf_bitmap, 'EMF StretchDIBits / BitBlt'),
    ('emf-text.emf', emf_text, 'EMF ExtTextOutW'),
    ('wmf-shapes.wmf', wmf_shapes, 'WMF shapes'),
    ('wmf-clip-bitmap.wmf', wmf_clip_bitmap, 'WMF clip, StretchDIB, TextOut'),
]


# -- deck ----------------------------------------------------------------------

def write_deck(out_dir):
    """One slide per fixture. python-pptx is imported here only."""
    from io import BytesIO
    from pptx import Presentation
    from pptx.opc.packuri import PackURI
    from pptx.util import Inches, Pt

    placeholder = (b'\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89'
                   b'\0\0\0\rIDATx\x9cc\xf8\xff\xff?\0\x05\xfe\x02\xfe\xa7\x9a\xa0\xa0\0\0\0\0IEND\xaeB`\x82')
    deck = Presentation()
    deck.slide_width, deck.slide_height = Inches(10), Inches(5.625)
    for name, build, caption in FIXTURES:
        slide = deck.slides.add_slide(deck.slide_layouts[6])
        box = slide.shapes.add_textbox(Inches(.5), Inches(.2), Inches(9), Inches(.6))
        box.text_frame.text = caption
        box.text_frame.paragraphs[0].runs[0].font.size = Pt(20)
        pic = slide.shapes.add_picture(BytesIO(placeholder), Inches(3.5), Inches(1.2),
                                       Inches(3), Inches(3))
        part = slide.part.related_part(pic._element.blip_rId)
        ext = name.rsplit('.', 1)[1]
        part._blob = build()
        part.partname = PackURI(f'/ppt/media/{name}')
        part._content_type = f'image/x-{ext}'
    path = out_dir / 'metafile-pictures.pptx'
    deck.save(path)
    return path


def main(argv):
    args = [a for a in argv[1:] if not a.startswith('--')]
    if len(args) != 1:
        sys.exit(__doc__)
    out_dir = Path(args[0])
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, build, _ in FIXTURES:
        (out_dir / name).write_bytes(build())
        print(out_dir / name)
    if '--pptx' in argv:
        print(write_deck(out_dir))


if __name__ == '__main__':
    main(sys.argv)
