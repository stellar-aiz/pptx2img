"""Compare line breaks and horizontal positions against LibreOffice using single-text-box fixtures."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageStat
from compare import run
from compare_boundary_widths import line_ink, same_breaks

POSITION_TOLERANCE = 2.0  # About 0.48pt at 1200px width; separate from pixel comparison thresholds.


def horizontal_distribution(image, bounds):
    gray = image.convert('L').crop((0, bounds[1], image.width, bounds[3]))
    ink = [255 - ImageStat.Stat(gray.crop((x, 0, x + 1, gray.height))).mean[0]
           for x in range(gray.width)]
    total = sum(ink)
    return [value / total for value in ink] if total else []


def transport_distance(first, second):
    """Difference in cumulative normalized horizontal ink distribution, independent of overall darkness."""
    if not first or len(first) != len(second):
        return None
    cumulative = distance = 0.0
    for a, b in zip(first, second):
        cumulative += a - b
        distance += abs(cumulative)
    return distance


def compare_positions(actual, reference, font_pt):
    if actual.size != reference.size:
        return {'passed': False, 'reason': 'image_size_mismatch'}
    gap = max(1, int(font_pt * actual.width / 288 * .1))
    a, b = line_ink(actual, gap), line_ink(reference, gap)
    lines = []
    if same_breaks(a, b):
        for left, right in zip(a['bounds'], b['bounds']):
            distance = transport_distance(horizontal_distribution(actual, left),
                                          horizontal_distribution(reference, right))
            edge = max(abs(left[0] - right[0]), abs(left[2] - right[2]))
            lines.append({'actual_bounds': left, 'reference_bounds': right,
                          'transport_px': distance, 'edge_error_px': edge,
                          'passed': distance is not None and distance <= POSITION_TOLERANCE
                                    and edge <= POSITION_TOLERANCE})
    return {'passed': bool(lines) and all(line['passed'] for line in lines),
            'line_breaks_match': same_breaks(a, b), 'actual_line_bounds': a['bounds'],
            'reference_line_bounds': b['bounds'], 'lines': lines}


def check_positions(directory, cases):
    results = []
    for case in cases:
        name = f"slide-{case['slide']:03}.png"
        with Image.open(directory / 'actual' / name) as a, Image.open(directory / 'reference' / name) as b:
            results.append({**case, **compare_positions(a, b, case['font_pt'])})
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    cases = json.loads(args.fixture.with_suffix('.json').read_text())
    args.out.mkdir(parents=True, exist_ok=False)
    report = {'status': 'error', 'tolerance_px': POSITION_TOLERANCE, 'modes': {}}
    try:
        for enabled in (False, True):
            mode = 'on' if enabled else 'off'
            directory = args.out / mode
            pixels = run(argparse.Namespace(binary=args.binary, fixture=args.fixture,
                out=directory, soffice=None, width=1200, pixel_tolerance=16,
                max_mae=.02, max_changed_ratio=.05, punctuation_compression=enabled))
            report['modes'][mode] = {'pixels_passed': pixels,
                                    'slides': check_positions(directory, cases)}
        ok = bool(cases) and all(m['pixels_passed'] and all(s['passed'] for s in m['slides'])
                                for m in report['modes'].values())
        report['status'] = 'passed' if ok else 'difference_exceeded'
        return 0 if ok else 1
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        (args.out / 'position-report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    raise SystemExit(main())
