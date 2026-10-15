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
- 上・中央・下の縦配置、文字枠からのはみ出し警告
- run / 段落既定 / リストレベル既定の文字属性を属性ごとに継承
- 埋め込まれた PNG / JPEG の配置

## 制限

次は未実装です。品質判定に使う場合は特に注意してください。

- レイアウト・マスターの図形・座標・背景・文字スタイルの完全な継承
- プレースホルダー継承、テーマの描画スタイル行列と themeOverride
- 完全な禁則・句読点圧縮・全記号のぶら下がり・複雑な文字 shaping・自動縮小
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
次は句読点圧縮などの組版とマスター図形継承を優先します。
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
を両エンジンで描画します。
LibreOffice は実行ごとに専用の一時プロファイルを使い PDF に変換し、
PDFium が PNG にします。通常の LibreOffice プロファイルは使いません。
比較処理はネットワークや Office 自動操作サービスを必要としません。

`target/visual-tests/{geometry,theme,text-theme}-*/` に、入力 PPTX、`actual/`、
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
