"""白背景・黒文字・単一文字枠の専用資料で、改行差を画像から検出する。"""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageStat
from compare import run


def line_ink(image, merge_gap):
    gray = image.convert('L')
    mask = gray.point(lambda value: 255 if value < 200 else 0)
    bands = []
    for y in range(image.height):
        row = mask.crop((0, y, image.width, y + 1)).getbbox()
        if row is None:
            continue
        if bands and y - bands[-1][3] <= merge_gap:
            last = bands[-1]
            last[0], last[2], last[3] = min(last[0], row[0]), max(last[2], row[2]), y + 1
        else:
            bands.append([row[0], y, row[2], y + 1])
    masses = []
    for _, top, _, bottom in bands:
        strip = gray.crop((0, top, image.width, bottom))
        masses.append((255 - ImageStat.Stat(strip).mean[0]) * strip.width * strip.height / 255)
    total = sum(masses)
    return {'bounds': bands, 'shares': [value / total for value in masses] if total else []}


def same_breaks(actual, reference):
    # 書体のラスタライズの濃さは全体量で正規化する。1文字の移動は行の割合に現れる。
    # OCR ではなく、この専用資料の境界回帰を検出するための検査。
    a, b = actual['shares'], reference['shares']
    return bool(a) and len(a) == len(b) and all(abs(x - y) <= .02 for x, y in zip(a, b))


def check_images(directory, cases):
    records = []
    for case in cases:
        name = f"slide-{case['slide']:03}.png"
        with Image.open(directory / 'actual' / name) as a, Image.open(directory / 'reference' / name) as b:
            # スライド幅 4 inch。文字内の小さな縦の隙間だけを結合する。
            gap = max(1, int(case['font_pt'] * a.width / 288 * .1))
            actual, reference = line_ink(a, gap), line_ink(b, gap)
        records.append({**case, 'actual': actual, 'reference': reference,
                        'passed': same_breaks(actual, reference)})
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    cases = json.loads(args.fixture.with_suffix('.json').read_text())
    args.out.mkdir(parents=True, exist_ok=False)
    report = {'status': 'error', 'cases': cases, 'modes': {}}
    try:
        for enabled in (False, True):
            mode = 'on' if enabled else 'off'
            directory = args.out / mode
            options = argparse.Namespace(binary=args.binary, fixture=args.fixture,
                out=directory, soffice=None, width=1200, pixel_tolerance=16,
                max_mae=.02, max_changed_ratio=.05, punctuation_compression=enabled)
            pixels_passed = run(options)
            records = check_images(directory, cases)
            report['modes'][mode] = {'pixels_passed': pixels_passed, 'slides': records}
        ok = all(m['pixels_passed'] and all(r['passed'] for r in m['slides'])
                 for m in report['modes'].values())
        report['status'] = 'passed' if ok else 'difference_exceeded'
        return 0 if ok else 1
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        (args.out / 'line-break-report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    raise SystemExit(main())
