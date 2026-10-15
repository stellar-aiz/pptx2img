"""同じ PPTX を圧縮 OFF/ON で比較し、設定が実際に効いたことも検証する。"""
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
                raise AssertionError(f'圧縮 {enabled} の比較基準超過')
        def metrics(first, second, page):
            name = f'slide-{page:03}.png'
            with Image.open(args.out / first / name) as a, Image.open(args.out / second / name) as b:
                return compare_images(a, b, 16)[0]
        result['renderer_effect'] = metrics('off/actual', 'on/actual', 7)
        result['reference_effect'] = metrics('off/reference', 'on/reference', 7)
        result['disabled_renderer_vs_enabled_reference'] = metrics('off/actual', 'on/reference', 7)
        result['renderer_control'] = metrics('off/actual', 'on/actual', 4)
        result['reference_control'] = metrics('off/reference', 'on/reference', 4)
        # ページ全体の平均が小さいだけの空振りを防ぐ。設定を外すと失敗する資料。
        assert not passes(result['disabled_renderer_vs_enabled_reference'], .02, .05), '圧縮を無視する実装を検出できません'
        assert result['renderer_effect']['mae'] > .005, '自作側の圧縮が効いていません'
        assert result['reference_effect']['mae'] > .005, 'LibreOffice 側の圧縮が効いていません'
        assert result['renderer_control']['mae'] == 0, '圧縮対象のない文字が変化しました'
        assert result['reference_control']['mae'] < .001, '参照側の対照ページが変化しました'
        result['status'] = 'passed'
    except Exception as error:
        result['error'] = str(error)
        raise
    finally:
        (args.out / 'matrix-report.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
