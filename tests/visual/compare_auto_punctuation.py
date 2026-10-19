"""Compare the same PPTX with compression OFF/ON and verify that the setting takes effect."""
import argparse
import json
from pathlib import Path
from PIL import Image
from compare import run, compare_images, passes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    result = {'status': 'error'}
    try:
        for enabled in (False, True):
            options = argparse.Namespace(binary=args.binary, fixture=args.fixture,
                out=args.out / ('on' if enabled else 'off'), soffice=None,
                width=1200, pixel_tolerance=16, max_mae=.02, max_changed_ratio=.05,
                punctuation_compression=enabled)
            if not run(options):
                raise AssertionError(f'Comparison thresholds exceeded with compression={enabled}')
        def metrics(first, second, page):
            name = f'slide-{page:03}.png'
            with Image.open(args.out / first / name) as a, Image.open(args.out / second / name) as b:
                return compare_images(a, b, 16)[0]
        result['renderer_effect'] = metrics('off/actual', 'on/actual', 7)
        result['reference_effect'] = metrics('off/reference', 'on/reference', 7)
        result['disabled_renderer_vs_enabled_reference'] = metrics('off/actual', 'on/reference', 7)
        result['renderer_control'] = metrics('off/actual', 'on/actual', 4)
        result['reference_control'] = metrics('off/reference', 'on/reference', 4)
        # Avoid passing on a low whole-page average alone; this fixture fails if the setting is ignored.
        assert not passes(result['disabled_renderer_vs_enabled_reference'], .02, .05), 'Failed to detect an implementation that ignores compression'
        assert result['renderer_effect']['mae'] > .005, 'Renderer compression has no effect'
        assert result['reference_effect']['mae'] > .005, 'LibreOffice compression has no effect'
        assert result['renderer_control']['mae'] == 0, 'Characters outside compression scope changed'
        assert result['reference_control']['mae'] < .001, 'Reference control page changed'
        result['status'] = 'passed'
    except Exception as error:
        result['error'] = str(error)
        raise
    finally:
        (args.out / 'matrix-report.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
