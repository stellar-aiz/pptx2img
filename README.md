# pptx2img

PPTX を直接読み、各スライドを PNG にする独立 CLI。
LibreOffice、Microsoft Office、document-skills、ブラウザー、
Python / Node ランタイムを実行時に必要としません。

**現在は初期プロトタイプです。PowerPoint 互換レンダラーとして完成していません。**
生成済み PPTX を視覚確認できる「目」を育てるための出発点です。

## 使い方

ビルドには Rust / Cargo が必要です。初回ビルド時は依存取得に通信します。
実行時はローカル処理で、ネットワークも Office も起動しません。

```sh
cargo build --release --locked
./target/release/pptx2img examples/basic.pptx --out images --width 1600
```

出力先は新しいディレクトリを指定してください。既存出力は上書きしません。
`slide-001.png` などと `manifest.json` を出力します。
途中で I/O エラーが発生した場合、出力済み画像が残ることがあります。
完了した処理だけが manifest を持ちます。

```sh
# 近似描画・未対応を検出したら、画像を書かず失敗する
./target/release/pptx2img input.pptx --out verified --strict

# OS フォントに加えて指定したフォントを使う
./target/release/pptx2img input.pptx --out images --font-file /path/to/font.ttf
```

警告は標準エラーと manifest の両方に残します。
通常モードは警告付きプレビューを許し、終了コード 0 を返します。
これは表示互換性の保証を意味しません。
`--strict` で警告があれば終了コード 1、出力先は作成しません。
テキストを含むスライドは、現時点では必ず近似描画の警告になります。

## 対応範囲

- Transitional OOXML の PPTX と presentation に記録されたスライド順序
- 明示座標・寸法・回転・反転を持つ矩形、楕円、直線
- 明示 RGB・システム色の lastClr・テーマ色による単色背景、塗り、線
- スライド → レイアウト → マスター → テーマの関連と色マッピング
- major / minor の欧文・日本語テーマフォント（Jpan 補助フォントを含む）
- テキストの明示フォント、サイズ、色、太字、斜体、段落の左右中央揃え
- 実フォントの公称文字幅を使った折り返し、基本的な日本語禁則
- 明示改行・空行、段落前後の余白、ポイント／百分率の行送りと固定行間の初行配置
- hangingPunct の段落・リスト・マスター・presentation 既定からの継承と、
  行末の句読点（、。，．）1 文字のぶら下がり
- 明示字間 `spc`（負値による句読点の詰め／正値による字間拡張）を
  文字幅・改行・左右中央揃えに反映。段落・リスト既定からの継承と明示ゼロに対応
- 上・中央・下の縦配置、文字枠からのはみ出し警告
- run / 段落既定 / リストレベル既定の文字属性を属性ごとに継承
- 埋め込まれた PNG / JPEG の配置

## 制限

次は未実装です。品質判定に使う場合は特に注意してください。

- レイアウト・マスターの図形・座標・背景・文字スタイルの完全な継承
- プレースホルダー継承、テーマの描画スタイル行列と themeOverride
- 完全な禁則・完全な自動句読点圧縮・全記号のぶら下がり・複雑な文字 shaping・自動縮小
- あらゆるフォント・文字組みで Office と一致するベースライン
- フォント代替の正確な追跡、Office との文字組み一致
- 表、グラフ、SmartArt、グループ、自由図形、多くのプリセット図形
- 透明度、グラデーション、影などの効果、画像トリミング
- SVG / EMF / WMF 画像、外部リンク、アニメーション
- Strict OOXML、旧式 .ppt、暗号化ファイル

未対応検出も開発途中です。strict モードは現在検出できる警告に対する
ゲートであり、未実装属性の完全検出や PowerPoint との一致は保証しません。
フォントが異なる端末では文字の見え方が変わります。
現時点の実行確認は macOS のみです。

入力は 128 MiB、展開後合計は 256 MiB、部品ごとは 32 MiB、
最大 200 スライドです。出力と埋め込み画像にも画素数の上限があります。
ZIP をファイルシステムへ展開せず、外部画像を取得しません。
不特定入力向けのサービス公開は、追加の耐性検証後に行ってください。

## 構成

```text
PPTX (ZIP/XML)
  → OOXML 解析・スライド順序・座標解決（src/pptx.rs）
  → 内部 SVG
  → resvg / usvg / tiny-skia
  → PNG + manifest（src/main.rs）
```

### 着想とコードの出自

[ibis-office](https://github.com/ibisjp/ibis-office) は、Office に頼らない
レンダラーを独自に作るという着想の参考です。
**ibis-office のコードをコピー・移植・翻訳した実装ではありません。**

調査では公開 README やクレートの入口などを閲覧しましたが、コードを
転用していません。OOXML の解釈・レイアウト処理は本プロジェクトで
独立に実装し、自作した PPTX と LibreOffice の描画結果で検証します。
テスト用 PPTX も本プロジェクト用に生成したもので、ibis-office から
持ち込んだものではありません。

resvg / usvg / tiny-skia / skrifa などは、描画・フォント処理の
依存ライブラリとして利用します。これらの利用と、ibis-office のコードを
転用していないことは別の事柄です。第三者コードを一切使わないという
意味ではありません。
本プロジェクトの公開ライセンスは未設定です。公開前に決定してください。
依存ライブラリのライセンスは各ライブラリの規定に従います。

## 検証と次の開発

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

テストは実際の ZIP/XML 入力と PNG 画素を照合し、スライド順序、
埋め込み画像、既存出力の保護、不正 ZIP、strict の停止を確認します。
`examples/basic.pptx` はこのプロジェクト用の合成テスト資料です。

テーマ色・テーマフォントと基本的な折り返しは実装済みです。
固定行間の初行位置と句読点のぶら下がりは比較テストで改善を確認しました。
明示字間による句読点の詰めと、任意で有効にする自動圧縮に対応しました。
境界幅での文字組み一致とマスター図形継承は今後の課題です。
PowerPoint が出した参照画像と同一フォントで比較する回帰テストを整備し、
対応表現を増やします。生成側のプレビューとの自己一致だけでは、
PPTX の表示互換性を検証できません。

## LibreOffice との描画比較

LibreOffice は**開発・テスト用の比較対象**としてのみ使います。
通常の CLI と `cargo test` は LibreOffice・Python を要求しません。
比較テストは `#[ignore]` で分離し、明示実行時に依存がなければ失敗します。

```sh
python3 -m venv .venv-visual
.venv-visual/bin/python -m pip install -r tests/visual/requirements.txt
# LibreOffice は別途導入する。PATH または SOFFICE で実行ファイルを指定する。
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice -- --ignored --nocapture
```

Windows では `.venv-visual/Scripts/python.exe` を使い、環境変数は
使用するシェルの方法で設定してください。Windows / Linux は未実測です。

図形・画像だけの `examples/geometry.pptx` と、テーマ色・テーマフォントの
`examples/theme.pptx`、固定行間・句読点を含む `examples/text-theme.pptx`
に加え、明示字間の `examples/character-spacing.pptx`（5 枚）を
両エンジンで描画します。
LibreOffice は実行ごとに専用の一時プロファイルを使い PDF に変換し、
PDFium が PNG にします。通常の LibreOffice プロファイルは使いません。
比較処理はネットワークや Office 自動操作サービスを必要としません。

`target/visual-tests/{geometry,theme,text-theme,character-spacing}-*/` に、入力 PPTX、`actual/`、
`reference/`（PDF と PNG）、`diff/`（黒地に赤い差分）、
`report.json`、各コマンドと標準出力・標準エラーを保存します。
失敗時も証跡を残し、実行ごとに新しいディレクトリを作ります。

判定は、RGB の平均絶対差を 0〜1 に正規化した MAE と、
いずれかのチャンネルで差が 16 を超える画素の割合を使います。
既定値は **MAE ≤ 0.02 かつ差分画素率 ≤ 0.05** です。
これは初期の回帰検知基準で、表示互換性の保証ではありません。
大きな背景に対して小さな文字が欠ける場合などは全体指標だけでは
見落とすため、差分画像の確認と個別の描画テストも併用します。

解像度は横幅 1200px に揃え、PDF ラスタライズの丸めによる 1px 以内の
寸法差のみ補正します。縦横比・ページ数の不一致はエラーにします。
LibreOffice の版、OS、フォントファイルの SHA-256、入力と実行ファイルの
SHA-256、PDFium / Pillow の版を report に残します。
これは環境の記録であり固定ではありません。回帰判定を継続する環境では
LibreOffice の版とインストールするフォントも固定してください。
macOS は CoreText を使う `SAL_USE_VCLPLUGIN=osx` で実行します。

日本語入りの資料や別の PPTX も直接比較できます。

```sh
cargo build --release --locked
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/release/pptx2img \
  --fixture examples/basic.pptx \
  --out target/visual-tests/basic-001
```

終了コードは一致範囲内なら 0、閾値超過なら 1、実行エラーなら 2 です。
`--max-mae`、`--max-changed-ratio`、`--pixel-tolerance` で基準を指定でき、
指定値も report に残ります。差を隠す目的で基準を緩めないでください。
LibreOffice 側にも描画差があり、検出した差分が必ず自作側の不具合とは
限りません。PowerPoint との比較は別の検証課題として残します。

比較器の一致・欠落・寸法違い・閾値のテストは LibreOffice 不要です。

```sh
.venv-visual/bin/python -m unittest discover -s tests/visual -p 'test_*.py'
```


## 文字組み・テーマ対応の検証記録

2026-10-15、macOS / LibreOffice 26.2.5.2 / Hiragino Sans で測定。
比較の閾値は従来から変更していません。

- 既存 basic.pptx の差分画素率は 4.21% → 3.91%、MAE は
  0.01694 → 0.01472。改善前後とも同じ PPTX・参照画像で比較。
- text-theme.pptx の 1 枚目（テーマ色・書体）は、固定行間修正により
  差分画素率 1.49% → 1.31%、MAE 0.00633 → 0.00511。
- 同資料の 2 枚目（日本語折り返し・空行・固定行間）は、句読点の
  ぶら下がり設定の継承と固定行間の初行位置を修正し、差分画素率
  4.31% → 3.30%、MAE 0.02256 → 0.01508。既存基準を満たしました。

改善前後とも入力 PPTX と比較基準は変更していません。
`theme.pptx` に加え、2 枚を含む `text-theme.pptx` 全体も
LibreOffice 比較の回帰テストに登録しています。個別の再現コマンド：

```sh
.venv-visual/bin/python tests/visual/compare.py \
  --binary target/release/pptx2img \
  --fixture examples/text-theme.pptx \
  --out target/visual-tests/text-theme-001
```

文字幅はフォントの公称 advance、縦配置は OS/2 の Windows セル
メトリクスを優先し、OS/2 がない場合は typographic/hhea に戻します。
描画側の kerning・合字によって測定幅と見た目の幅に差が出ることがあり、
完全な文字 shaping とフォント代替の一致は今後の課題です。


## 明示字間による句読点の詰め

DrawingML の [`spc` 属性](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.textcharacterpropertiestype.spacing)
を 1/100 pt 単位で読み、負値では字間を詰め、正値では広げます。
同じ文字書式が連続する区間の文字間に適用し、行末と書式境界には加えません。
この境界の扱いは LibreOffice 26.2.5.2 の PDF 内の文字座標でも確認しました。
文字自体を横に潰す処理ではありません。
文字幅を超える負の字間は警告を出し、送り幅を 0 に制限します。

**この節は明示字間の検証記録です。後続の自動圧縮の対応は次節を参照してください。**
調査用資料では、字間指定のない全角句読点は、狭い文字枠でも LibreOffice が
自動圧縮せず禁則に沿って改行しました。`kern` を変更しても同じでした。
これは検証した資料・環境での観測であり、すべての PPTX の一般則ではありません。
その後、LibreOffice 側の圧縮設定を有効にすることで再現できました（次節）。
既定動作では引き続き字間未指定の文字幅を変えません。

比較資料は次の内容を含みます。

1. 連続する句読点・閉じ括弧への負の字間と左右中央揃え
2. 文字列全体への負の字間
3. 字間ゼロの対照（修正前後の画像が同じ）
4. 圧縮によって改行位置が変わる狭い枠と正の字間
5. 段落既定の継承、明示ゼロによる取消、正値による上書き、18pt / 24pt

2026-10-15、macOS / LibreOffice 26.2.5.2 / Hiragino Sans、横幅 1200px。
同一入力・同一参照画像による MAE は以下の通りです。

| ページ | 修正前 | 修正後 |
|---|---:|---:|
| 句読点の詰め | 0.03574 | 0.01840 |
| 文字列全体の詰め | 0.03869 | 0.01716 |
| 字間ゼロ | 0.01842 | 0.01842 |
| 折り返し | 0.03328 | 0.01880 |
| 継承と上書き | 0.02380 | 0.01243 |

修正後は全ページが既存の MAE / 差分画素率の両基準を満たします。
通常の Rust テストでも、文字幅・改行・書式境界・継承・明示ゼロ・
SVG の文字位置を検証し、ページ全体の平均だけに依存しません。

資料の再生成には開発環境の `python-pptx` を使います。
通常の CLI 実行と比較テストではこの生成依存は不要です。

```sh
python tests/visual/make_character_spacing.py examples/character-spacing.pptx
```


## 自動句読点圧縮（実験的）

```sh
./target/release/pptx2img input.pptx --out images --punctuation-compression
```

このオプションは**描画時の方針**です。PPTX 内の設定を検出したという意味では
ありません。既定は無効で、選択を `manifest.json` の
`punctuation_compression` に記録します。有効時の近似描画は警告にも残します。

LibreOffice の [Asian Layout](https://help.libreoffice.org/latest/en-US/text/shared/optionen/01150100.html)
には、圧縮なし・句読点のみ・句読点と仮名の選択があります。
公開 API の [CharacterCompressionType](https://api.libreoffice.org/docs/idl/ref/CharacterCompressionType_8idl_source.html)
ではそれぞれ 0 / 1 / 2 です。今回、同一の PPTX を異なる専用プロファイルで
読み込み、`org.openoffice.Office.Common/AsianLayout/CompressCharacterDistance`
を 0 から 1 に変えると、幅不足の行で句読点が詰まり改行が変わると確認しました。
余裕のある行は変わりません。カーニングとは別の設定です。
検証した PPTX の結果が環境設定で変わることを確認したものであり、すべての
PowerPoint での既定値や、設定の PPTX への保存可否を断定するものではありません。

### 対応内容と限界

- 字間未指定またはゼロの横書き・折り返し行を対象に、全角句読点・括弧の
  半角分の余白を上限として、行に必要な分を圧縮する。
- 開き括弧の前、閉じ括弧・句読点の後の余白を調整し、字形は縮小しない。
- 禁則による追い出し後に最大圧縮が残るケースも比較結果に合わせる。
- 仮名、中点、感嘆符・疑問符は圧縮しない。`wrap="none"` では適用しない。
- 非ゼロの明示字間との併用は未対応。その段落は明示字間を優先し警告する。
- 境界幅の改行判定を改善したが、フォント計測の丸めや圧縮量には差が残る。
  下記の境界幅の比較資料で、確認済みの範囲と未解決ケースを分けて記録する。

### 比較テスト

```sh
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice automatic_punctuation -- --ignored --nocapture
```

8 枚の同一資料を圧縮 OFF / ON で描画し、各モードで LibreOffice と比較します。
比較器は専用の一時プロファイルへ設定を書き、ユーザーの通常設定は変更しません。
`libreoffice-profile.xcu`、実行コマンド、入力ハッシュ、圧縮条件を証跡に保存します。
通常の比較も圧縮 OFF を明示し、端末の既定値へ依存しないようにしました。

MAE ≤ 0.02、差分画素率 ≤ 0.05 は変更していません。さらに次を検証します。

- 自作側・LibreOffice 側の双方で、ON / OFF によって描画が変わること
- 圧縮対象がないページは双方とも変わらないこと
- 自作側だけ圧縮 OFF に戻すと、圧縮 ON の参照との比較が不合格になること

2026-10-15、macOS / LibreOffice 26.2.5.2 / Hiragino Sans で、密な句読点の
7 枚目は圧縮 ON の参照に対して MAE **0.03124 → 0.00637**、
差分画素率 **4.14% → 1.60%**。閾値内でも局所的な改行差が残るため、
単体テストの文字幅・字形位置・禁則・明示改行の検証も併用します。

再生成（開発時のみ `python-pptx` が必要）：

```sh
python tests/visual/make_auto_punctuation.py examples/auto-punctuation.pptx
```


### 境界幅での改行（2026-10-15）

字間ゼロの行では、行と枠の幅を 1/100 mm 単位に丸め、句読点ごとの圧縮可能量を同じ単位で
切り捨ててから足す判定へ変更しました。LibreOffice 26.2.5.2 / macOS /
Hiragino Sans の比較結果から得た近似であり、OOXML 全般の規則とは断定しません。
例えば 24pt の「あ、いう、え」は圧縮 ON で幅 120pt では折り返し、
120.01pt では一行になります。18pt では 90.01pt / 90.02pt が境界です。

`examples/boundary-widths.pptx` は、18pt・20pt・21.5pt・24pt・32pt の通常文字と句読点について、
境界の前後 ±0.1pt 内を 0.01pt の差も含めて調べる自作の 99 ページです。
圧縮 OFF / ON の計 198 条件を LibreOffice と比較します。
既存の MAE と差分画素率に加え、文字の行数と各行に占める描画量の割合を
比較します。後者は全体の濃さで正規化し、割合差 0.02 以下を要求します。
OCR ではなく、白背景・黒文字・単一文字枠の専用資料での回帰検出です。
同じ二行のまま一文字が移る差や空画像も検出する単体テストがあります。
`line-break-report.json` に条件、行の範囲、描画量の割合、判定を残します。

```sh
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice boundary_widths -- --ignored --nocapture
python tests/visual/make_boundary_widths.py examples/boundary-widths.pptx
```

調査時の 12pt・18pt・24pt の 63 ページ、圧縮 OFF / ON の計 126 条件では、
改行の不一致が変更前 27 件から変更後 2 件に減りました。
既存の画像比較だけでは変更前も全件合格していたため、行単位の検査を追加しています。

調査資料 `examples/boundary-font-metrics.pptx` は全 72 ページを保持しています。
12pt・圧縮 OFF・「あ、いう、え」・幅 59.98pt / 59.99pt の改行差は残ります。
20pt・21.5pt・32pt の境界差は下記の幅計測の変更で改善しました。
次の手動比較は現時点では不合格になります。合格する回帰テストには混ぜず、
不一致を無視する例外や閾値の緩和も入れていません。

```sh
python tests/visual/compare_boundary_widths.py \
  --binary target/release/pptx2img \
  --fixture examples/boundary-font-metrics.pptx --out /tmp/font-metrics-comparison
python tests/visual/make_boundary_widths.py examples/boundary-font-metrics.pptx \
  --sizes 12 20 21.5 32
```

また、改行が一致しても境界付近での句読点の圧縮量や字形位置は完全一致しません。
単一書体・余白なしの専用資料で確認した結果であり、混在書体や枠の余白を含めた
すべての丸め処理を再現したものではありません。非ゼロの明示字間を含む行は
下記の書式区間単位の丸めを使い、既存の字間比較テストで回帰を確認します。
実行時依存の追加はありません。


### フォント幅の計測（2026-10-15）

サイズ指定の `skrifa::GlyphMetrics` は FreeType 互換の固定小数点換算を行うため、
1/64px 単位のサイズ切り捨てが文字幅に入りました。例えば 20pt の全角幅は
約 26.656px でした。`Size::unscaled()` でフォント単位の advance を取得し、
倍精度で換算するよう変更しました。全角だけを補正する定数は使わず、半角・空白・
幅ゼロの文字も元の幅比率を維持します。

ただし公称サイズへ比例換算するだけでは LibreOffice との改行差は解消しません。
Impress の計測は 600 DPI の参照デバイスを使い、フォント高さを整数デバイス座標へ
丸めます。そのため、計測用の高さを 1/100 mm 単位へ丸めた後、600 DPI の整数へ
丸める段階を分離しました。計測用の全角幅は 20pt で 26.72px、21.5pt で
28.64px、32pt で 42.72px です。SVG に指定する字形のサイズは元のままです。

参照した公開実装と API：

- [skrifa 0.44.0 の Size API](https://docs.rs/skrifa/0.44.0/skrifa/instance/struct.Size.html)
- [Impress の参照デバイス設定](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/sd/source/ui/app/sdmod.cxx#L79)
- [フォント高さのデバイス座標変換](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/vcl/source/outdev/font.cxx#L686)
- [計測時のスケール](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/vcl/source/font/LogicalFontInstance.cxx#L115)

公開実装は丸め方の確認に参照しました。コードの移植はせず、計測モデルと自作の
比較資料に基づく独立実装です。LibreOffice はテスト時だけ利用します。
このモデルは確認した Impress の挙動への近似で、すべての Office や書体での
完全一致を意味しません。複雑な shaping やカーニングは別の未対応事項です。

正確な幅に変更すると、従来の微小誤差で隠れていた明示字間の境界差も現れました。
非ゼロの明示字間を含む行では、同じ書式の区間ごとに幅を丸めて足し合わせるよう
変更し、既存の字間比較を維持しました。閾値の緩和はありません。

既存の 72 ページを圧縮 OFF / ON で調べた 144 条件では、改行の不一致が
**65 件から 2 件**へ減りました。改善した 20pt・21.5pt・32pt の 54 ページは
`boundary-widths.pptx` の自動回帰テストにも追加しています。
残る 2 件は上記 12pt のケースで、元資料から削除していません。

通常の Rust テストは自作の `tests/fixtures/metrics.ttf` を使い、6 サイズで
全角・半角・空白・幅ゼロの比率、描画サイズの保持、欠落文字の警告を検証します。
このフォントは単純な矩形字形と自前の幅値だけで作り、外部の字形は使っていません。
実行時依存は追加していません。再生成だけには `fontTools` が必要です。

```sh
python tests/fixtures/make_metrics_font.py
```
