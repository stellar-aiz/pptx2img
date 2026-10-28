# 三者比較による検証方針

同一の非機密な入力から、Excel 本体、rhodopsin-xlsx、LibreOffice の PDF を得る。
Excel 本体の「PDF として保存」を正解とし、LibreOffice は差の原因を
切り分けるための独立した観測点とする。多数決では判定しない。

## 比較と判定

次の三組を、同じ比較器と条件で比較する。

- rhodopsin-xlsx と Excel：合否判定。
- LibreOffice と Excel：互換性の差の把握。
- rhodopsin-xlsx と LibreOffice：共通する差と固有の差の切り分け。

| 観測 | 判定・次の調査 |
| --- | --- |
| rhodopsin-xlsx が Excel と一致 | 対象の検査項目は合格。LibreOffice の差は別記録 |
| LibreOffice だけが Excel と一致 | rhodopsin-xlsx の解釈・配置・描画を調査 |
| rhodopsin-xlsx と LibreOffice が一致し、Excel と異なる | 不合格。Excel 固有の挙動や両者に共通する前提を調査 |
| 三者すべて異なる | 不合格。入力・環境条件・比較方法を確認して差を分解 |

一致とは対象の検査項目についての一致を指す。三者比較だけで原因が
確定するわけではない。LibreOffice の失敗や出力欠落は未観測として記録し、
一致扱いしない。

## 保存する証拠

fixture ごとに入力 `.xlsx`、Excel の参照 PDF、LibreOffice の比較 PDF を保存する。
rhodopsin-xlsx の出力 PDF と比較結果は、実装のリビジョンを付けた検証成果物とする。
参照 PDF は通常のテスト実行で上書きしない。

生成記録には次を含める。

- 入力と各 PDF のハッシュ、生成元アプリのバージョン、OS。
- 使用フォントの書体・バージョン・ファイルのハッシュ。
  再配布できないフォント本体は保存しない。
- ロケール、日付基準、印刷設定、PDF 出力手順。
- 比較器・ラスタライズ用ツールのバージョン、解像度、比較条件。
- 許容誤差とその根拠、既知の差、調査状況。

同じ入力を各アプリで開き、入力を再保存せずに PDF を生成する。
保存済みの数式結果と表示に使われた結果が一致するかも確認する。
再計算やフォント代用が発生した場合は環境差として記録し、
入力が揃った比較として扱わない。

## 検査項目

ページ数、ページ寸法、各ページの先頭・末尾のセル、全文字列、
金額、文字位置、フォント埋め込み、ラスタライズ画像の差を確認する。
繰り返し行列は本文の先頭・末尾と分けて記録する。

PDF のバイト列の一致は紙面の合否条件にしない。
rhodopsin-xlsx 自身の決定性は、同一入力・同一フォントで再生成したバイト列を
別途比較して検証する。

位置の許容誤差や画像差のしきい値は根拠を記録して固定する。
差を隠すための位置合わせ、ページの切り落とし、しきい値緩和はしない。
差分画像と数値を残し、既知の差は再現 fixture と結び付ける。

Excel と LibreOffice は開発・検証環境だけで使う。
配布する CLI の実行時依存には加えない。

## ページ幾何の比較ツール

開発用の Rust example で、三つの既存 PDF を読み取り専用で比較する。
配布 CLI の実行時依存には追加しない。

```sh
cargo run --example compare-pages -- excel.pdf rhodopsin-xlsx.pdf libreoffice.pdf > comparison.json
cargo test --example compare-pages
```

各 PDF の SHA-256、ページ順の MediaBox・CropBox（ポイント）、回転、
三組の差分を JSON に出す。継承された矩形・回転と UserUnit を解決する。
ページごとの抽出文字列も SHA-256・文字数・UTF-8 バイト数で比較する。
文字列本体は出力せず、空白・改行・Unicode の正規化を行わない。
抽出器は lopdf 0.38.0。抽出失敗・読み込み失敗は、その PDF を理由付きの
`unobserved` として記録する。残る観測があれば比較を続ける。
幾何値は厳密比較し、未観測を含む組には一致・不一致の値を付けない。
ファイルパス・PDF 内の文字列は診断へ含めない。

終了コード 0 は rhodopsin-xlsx と Excel のページ幾何・抽出文字列の一致だけを表す。
いずれかの差は 1、Excel または rhodopsin-xlsx が未観測の場合と実行自体の失敗は 2。
LibreOffice の差・欠落は終了コードを左右せず、観測結果に記録する。
JSON は schema_version 3。比較済みの組は幾何・抽出文字の結果を別々に保持する。
セルごとの期待文字列、金額、セル境界、文字位置、フォント埋め込み、画像差は
`unchecked` に列挙し、全体判定は `not_evaluated` のまま出す。
この段階の結果を製品の受け入れ判定へ流用しない。
抽出器が扱わない描画や抽出順の差もあるため、抽出文字列の一致・不一致を
そのまま全セルの印刷内容の判定にしない。
抽出前にフォーム XObject、引用符による文字表示命令、BDC、未解決書体、
書体復元後に再選択されない文字表示を検出し、理由付きの未観測にする。
これらを黙って空文字へ落とさないための検査で、PDF 全命令への対応ではない。
通常の画像 XObject はこの検査を通る。フォーム・タグ付き PDF の比較は、
抽出対応を追加するまで未観測となる。

比較器の合成テストは通過しているが、Excel 本体由来の参照 PDF を使った
三点観測は未実施。参照成果物の生成元を SHA-256 だけで証明することもできない。
このツールは非機密の管理済み fixture 用で、配布 CLI と同等の PDF 展開メモリ
制限を備えていない。

## 一ページ分の画像比較ツール

同じラスタライザ・版・解像度・背景色・アンチエイリアス設定で各 PDF の
同じページを不透明な 8 bit PNG に変換し、次の開発用ツールへ渡す。
PDF の画像化自体はこのツールでは行わない。

```sh
cargo run --example compare-rasters -- excel.png rhodopsin-xlsx.png libreoffice.png > raster-comparison.json
cargo test --example compare-rasters
```

各入力の SHA-256 と画素寸法、および三組の比較結果を JSON に出す。
比較結果は、異なる画素数、最大チャンネル差、RGB 各チャンネルの絶対差の総和・平均、
差のある領域の外接矩形。矩形は左上原点で右端・下端を含まない座標とする。
同一画素の複数チャンネルが違っても、異なる画素数は一つと数える。

位置合わせ・色補正・縮小・しきい値による差の除外は行わない。
寸法不一致の場合は不一致を記録し、画素差の各指標は未評価とする。
透過画像は背景合成を推測せず拒否し、ICC プロファイル等の未対応メタデータも拒否する。
入力ファイルは各 32 MiB、復号後は各 64 MiB、各辺 16384 画素に制限する。
ファイル名やパスは結果・診断に含めない。入力ファイルへは書き込まない。

終了コード 0 は Excel と rhodopsin-xlsx の画素が完全一致した場合、1 は差がある場合、
2 はどちらかが未観測の場合または実行失敗。LibreOffice の結果は独立して記録する。
出力の `overall_verdict` は `not_evaluated` のままとする。
これは一ページ分の画素比較であり、PDF 全体の合格・文字抽出・物理寸法・
生成元アプリ・ラスタライズ条件を保証しない。条件は別途 fixture の生成記録で管理する。
全ページの集約には後述の `tools/compare-document.py` を使用する。
実 Excel の参照 PDF から画像化した三者比較はまだ実施していない。

既存の出力ディレクトリを `--diff-dir` で指定すると、三組の差分 PNG も保存する。

```sh
mkdir raster-differences
cargo run --example compare-rasters -- excel.png rhodopsin-xlsx.png libreoffice.png --diff-dir raster-differences > raster-comparison.json
```

固定名 `rhodopsin_xlsx_vs_excel.png`、`libreoffice_vs_excel.png`、
`rhodopsin_xlsx_vs_libreoffice.png` を使用する。各画素の RGB 絶対差を増幅せず保存し、
差がゼロの領域は黒になる。小さい差は暗く見えるため、数値の差分件数も併用する。
元画像と同じ画素寸法・座標で、PNG の圧縮は可逆。
寸法不一致または未観測を含む組は画像を作らず、その理由を記録する。

既存ファイルを上書きしない。一枚ごとに同じディレクトリの一時ファイルから
排他的に確定し、途中の PNG を公開しない。三枚全体を一括確定する方式ではなく、
一枚の保存失敗時も他の組の保存を試みる。既に完成した画像は残し、
レポートの `difference_images` に各組の保存状態・固定相対名・SHA-256 を記録する。
保存失敗があれば終了コードは 2 とするが、比較数値は引き続き stdout に返す。
指定したディレクトリのパスは診断へ出さない。

## 参照ファイルと生成記録の整合性

fixture ディレクトリに `reference.json` を置き、次の開発用チェックを実行する。

```sh
cargo run --example check-reference -- tests/fixtures/example/reference.json
```

記録の形は次のとおり。山括弧の値は説明用で、そのままでは検査を通らない。
ハッシュは実ファイルの SHA-256 を小文字の 64 桁で記録する。

```json
{
  "schema_version": 1,
  "input": {"file": "input.xlsx", "sha256": "<入力のSHA-256>"},
  "excel": {
    "artifact": {"file": "excel.pdf", "sha256": "<Excel PDFのSHA-256>"},
    "input_sha256": "<入力のSHA-256>",
    "application": "Microsoft Excel",
    "version": "<実際のバージョン>",
    "os": "<実際のOSと版>",
    "procedure": "<実際に行ったPDF保存手順>",
    "recalculated": false,
    "fonts_substituted": false
  },
  "libreoffice": null,
  "locale": "ja-JP",
  "date_system": "1900",
  "fonts": [
    {"family": "<実際の書体>", "version": "<実際の版>", "sha256": "<書体のSHA-256>"}
  ]
}
```

LibreOffice の参照があれば `libreoffice` に `excel` と同じ形の記録を追加し、
`application` を `LibreOffice` にする。欠落は `null` とし、Excel 参照の代用にしない。
各出力の `input_sha256` は同じ入力を示す必要がある。再計算や書体代用の記録が
ある場合は同条件での比較として受理しない。記録を偽って false に変更してはならない。

パスは記録ファイルのあるディレクトリからの相対パスに限定する。
各ファイルのハッシュを読み取り専用で照合し、ファイルサイズは各 128 MiB、
記録ファイルは 64 KiB に制限する。診断へファイル名や記録内容を転載しない。
書体は再配布制限を避けるため記録だけを要求し、書体ファイル自体はこのチェックでは
読み込まない。書体のハッシュ・版・使用実態は別途確認する。

終了コード 0 は記録とハッシュの整合性のみ、2 は不整合または読み取り失敗。
結果は `provenance: declared_not_authenticated` と
`overall_verdict: not_evaluated` を明記する。PDF・xlsx の構文検査、アプリの真正性、
保存手順の実施、フォントの使用実態、紙面一致を保証するツールではない。
現時点でこの記録を満たす実 Excel 参照 fixture はリポジトリにない。

## 全ページの観測集約

`tools/compare-document.py` は PDF 比較器と PNG 比較器を順に呼び出し、
全ページの数値・差分画像を新規ディレクトリへ保存する。

```sh
cargo build --locked --example compare-pages --example compare-rasters
python3 tools/compare-document.py comparison-input.json \
  --pdf-comparator target/debug/examples/compare-pages \
  --raster-comparator target/debug/examples/compare-rasters \
  --out-dir target/comparison
```

入力記録のパスは記録ファイルの親を基準とする。各 `pages` は先頭からの順序。
PDF と PNG の対応・画像化条件は生成者が確認する。このツールは画像化しない。

```json
{
  "schema_version": 1,
  "inputs": {
    "excel": {"pdf": "excel.pdf", "pages": ["excel-1.png", "excel-2.png"]},
    "rhodopsin_xlsx": {"pdf": "ours.pdf", "pages": ["ours-1.png", "ours-2.png"]},
    "libreoffice": {"pdf": "lo.pdf", "pages": ["lo-1.png", "lo-2.png"]}
  }
}
```

PDF の実測ページ数と PNG 列挙数を照合する。欠落・余剰・PDF 未観測を
完全な比較とは扱わない。二ページ目以降の差も終了コードへ伝播する。
LibreOffice の欠落や余剰は独立に記録し、Excel との判定を変更しない。
終了コードは観測範囲の一致が 0、差が 1、未観測または保存失敗が 2。
`overall_verdict` は常に `not_evaluated`。セル範囲・金額・文字位置・
埋め込みフォント・参照の真正性・PNG と PDF の対応は別途検証が必要。

`comparison.json` とページ別の三組の差分 PNG を保存する。
既存の出力ディレクトリは拒否する。途中失敗では保存済み証跡は残る。
入力記録は 1 MiB、各入力の列挙数は 10,000 ページまで。
各比較プロセスには 120 秒の上限を設け、一度に一ページだけ画像を比較する。
入力内容・入力パス・子プロセスの標準エラーは集約診断へ転載しない。
