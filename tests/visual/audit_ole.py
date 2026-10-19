"""Audit only OLE structure, relationships, and image formats without exporting text or images."""
from io import BytesIO
import json
from pathlib import Path
import posixpath
import sys
from xml.etree import ElementTree as ET
from zipfile import ZipFile
from PIL import Image

P = 'http://schemas.openxmlformats.org/presentationml/2006/main'
A = 'http://schemas.openxmlformats.org/drawingml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
MC = 'http://schemas.openxmlformats.org/markup-compatibility/2006'
NS = {'p': P, 'a': A, 'mc': MC}


def audit(path):
    rows = []
    with ZipFile(path) as archive:
        for name in sorted(archive.namelist()):
            if not name.startswith('ppt/slides/slide') or not name.endswith('.xml'):
                continue
            root = ET.fromstring(archive.read(name))
            frames = root.findall('.//p:graphicFrame', NS)
            if not any(f.findall('.//p:oleObj', NS) for f in frames):
                continue
            relpath = posixpath.dirname(name) + '/_rels/' + posixpath.basename(name) + '.rels'
            rels = {r.get('Id'): r for r in ET.fromstring(archive.read(relpath))}
            for frame in frames:
                data = frame.find('a:graphic/a:graphicData', NS)
                if data is None:
                    continue
                choices = data.findall('mc:AlternateContent/mc:Choice/p:oleObj', NS)
                ole = data.find('p:oleObj', NS)
                if ole is None:
                    ole = data.find('mc:AlternateContent/mc:Fallback/p:oleObj', NS)
                if ole is None:
                    continue
                pic = ole.find('p:pic', NS)
                blip = ole.find('.//a:blip', NS)
                rid = blip.get('{' + R + '}embed') if blip is not None else None
                rel = rels.get(rid)
                fmt = None
                if rel is not None and rel.get('TargetMode') != 'External':
                    imagepath = posixpath.normpath(posixpath.dirname(name) + '/' + rel.get('Target'))
                    with Image.open(BytesIO(archive.read(imagepath))) as image:
                        fmt = image.format
                hidden = pic.find('p:nvPicPr/p:cNvPr', NS).get('hidden') if pic is not None else None
                transform = frame.find('p:xfrm', NS)
                rows.append({
                    'page': int(Path(name).stem.removeprefix('slide')),
                    'kind': ole.get('progId'),
                    'choice_preview_count': sum(len(c.findall('.//a:blip', NS)) for c in choices),
                    'selected': 'Fallback' if choices else 'direct',
                    'preview_relation': rid, 'preview_format_from_bytes': fmt,
                    'hidden': hidden,
                    'offset': transform.find('a:off', NS).attrib,
                    'extent': transform.find('a:ext', NS).attrib,
                    'same_preview_outside': sum(b.get('{' + R + '}embed') == rid for b in root.findall('.//a:blip', NS)) - 1 if rid else 0,
                    'same_ole_in_choice': all(c.get('{' + R + '}id') == ole.get('{' + R + '}id') for c in choices),
                })
    return sorted(rows, key=lambda row: row['page'])


if __name__ == '__main__':
    print(json.dumps(audit(sys.argv[1]), ensure_ascii=False, indent=2))
