# 全体・局所・診断を分けた比較

`compare.py` は LibreOffice を比較対象に使います。PowerPoint の正解画像は
ありません。参照側にも重なりや描画差があり、閾値内は完全互換の証明には
なりません。実資料の調査では参照側にも重なりが観測されたため、差の原因は
両方の画像を確認してください。2026-10-18 の元資料比較では、1・20 ページに
参照側の重なりも観測されています。元資料と画像はローカルにのみ保持します。

## 指定矩形

`--regions regions.json` でページごとの局所比較を追加できます。
`slide` は 1 始まり、`name` は同じページ内で一意です。
`rect` は `[x, y, 幅, 高さ]` を画像全体に対する 0〜1 の比率で指定します。
画像外、面積ゼロ、存在しないページ、重複名は実行エラーです。

```json
{
  "regions": [
    {"slide": 1, "name": "小図形", "rect": [0.1, 0.1, 0.02, 0.02]}
  ]
}
```

矩形の左上を切り下げ、右下を切り上げた画素範囲を使います。
領域指標には実際の `pixel_rect`（左・上・右・下、右下は範囲外）も記録します。
局所にも全体と同じ閾値（既定 MAE 0.02、差分画素率 0.05、画素差許容 16）を
適用します。差を隠すための個別閾値は追加していません。

`report.json` の読み方は次のとおりです。

| 項目 | 意味 |
| --- | --- |
| `slides[].passed` / `global_passed` | 従来どおりの全体画素判定 |
| `slides[].regions` | 指定領域ごとの指標と判定 |
| `regions_passed` | 指定領域の全件判定。未指定なら `null` |
| `diagnostic_summary` | 診断種類ごとの件数と `has_omissions` |
| `slides[].renderer_diagnostics` | 部品・図形・位置を含む元の診断 |
| `status` | 全体・指定領域のいずれかが超過すれば `difference_exceeded` |

`diagnostic_summary` は全体と各ページに保存します。
古い manifest に診断欄がなければ `available: false` とし、省略の有無を
`null` にします。`renderer_warnings` は従来どおり保持します。

終了コードは全体・指定領域とも閾値内なら 0、いずれか超過なら 1、
実行エラーなら 2 です。診断そのものは画素判定と独立しています。
例えば `global_passed: true` と `has_omissions: true` が同時に成立します。
局所指標は差の検出であり、欠落の原因や見えている対象を自動同定するものでは
ありません。`omitted` がなくても未検出の非互換性はあり得ます。

## 自作の小領域省略の再現

小グラフが余白に埋もれる資料を独立に生成します。生成時だけ
`python-pptx` が必要で、実行時依存への追加はありません。
グラフ省略は描画側の既知の制限であり、この比較器の変更では修正しません。

```sh
cargo build --locked
.venv-visual/bin/python tests/visual/make_local_omission.py /tmp/local-omission.pptx
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/debug/pptx2img \
  --fixture /tmp/local-omission.pptx --regions /tmp/local-omission.json \
  --width 1600 --out target/visual-tests/local-omission
```

全体は閾値内、指定領域は超過、省略診断あり、終了コード 1 を期待します。
`--regions` を省くと従来の全体判定だけになり、終了コード 0 になりますが、
省略診断はレポートで確認できます。

## 再現条件と回帰テスト

出力先には入力 PPTX、実行コマンド、標準出力・標準エラー、参照 PDF、
両者の PNG、全体差分、指定領域 JSON、比較器ソースのコピーを保存します。
レポートには入力・描画バイナリ・LibreOffice 実行ファイル・比較器・領域 JSON
の SHA256、使用幅、閾値、圧縮条件、Python・画像ライブラリ・OS の版、
フォントファイル一覧と SHA256、描画側が解決したフォントを記録します。
同じバイナリ・フォント・OS・比較依存と保存した設定で再実行してください。
出力先は新規ディレクトリを指定します。

```sh
.venv-visual/bin/python -m unittest discover -s tests/visual -p 'test_*.py'
cargo test --locked
```

小図形を消した画像では全体が閾値内でも局所が超過すること、矩形の検証、
省略診断の集計、旧 manifest の未記録状態、従来の閾値境界を検証します。
元資料・比較画像はローカルで保持し、公開の証跡には自作資料だけを使います。
