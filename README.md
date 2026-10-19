# pptx2img

PPTX を直接読み、各スライドを PNG にする独立 CLI・Rust ライブラリ。
LibreOffice、Microsoft Office、document-skills、ブラウザー、
Python / Node ランタイムを実行時に必要としません。

**現在は初期プロトタイプです。PowerPoint 互換レンダラーとして完成していません。**
生成済み PPTX を視覚確認できる「目」を育てるための出発点です。

## 使い方

ビルドには Rust / Cargo が必要です。初回ビルド時は依存取得に通信します。
実行時はローカル処理で、ネットワークも Office も起動しません。

```sh
cargo build --release --locked
./target/release/pptx2img examples/basic.pptx --out images --width 1600 --font-fallback
```

`--out` を省略すると、入力ファイルと同じ場所に、最後の拡張子を
`.pptx2img` に置き換えたディレクトリを作成します。
たとえば `pptx2img ~/Downloads/資料.pptx` の出力先は
`~/Downloads/資料.pptx2img/` です。
`--out` を指定した場合はそのパスを優先します。
いずれの場合も既存の出力先は上書きせず、エラーで終了します。
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
既定ではフォント不足・欠字・太さや斜体の不一致を検出すると、
書体名と対象スライドを表示し、出力先を作らず終了コード 1 を返します。
`--font-fallback` を指定すると代替描画を許可し、警告付きで出力します。
それ以外の警告は通常モードでは出力を妨げません。
これは表示互換性の保証を意味しません。
`--strict` で警告があれば終了コード 1、出力先は作成しません。
テキストを含むスライドは、現時点では必ず近似描画の警告になります。

### CLI の結果形式

変換成功時は標準出力に、失敗時は標準エラーに、1つの YAML 文書を返します。
成功時の警告は従来どおり標準エラーと manifest に残します。
`--help` と `--version` は通常の表示です。

```yaml
status: completed
output:
  created: true
  folder: "./images"
  slides: 20
```

```yaml
status: failed
output:
  created: false
errors:
  - code: FONT_NOT_FOUND
    family: "Calibri"
    slides: [6, 14]
    suggestion: "必要なフォントを --font-file <path> で追加してください。入手できない場合は依頼者に提供を求めてください。"
```

`errors` は問題の種類と書体ごとにまとめ、対象スライドを重複なく列挙します。
`FONT_GLYPH_MISSING` は `characters` に `character` と `codepoint` を含みます。
ほかに `FONT_STYLE_MISSING`、`FONT_MAPPING_TARGET_NOT_FOUND`、
`FONT_SUBSTITUTION_REQUIRED`、`STRICT_MODE_REJECTED`、
`CONVERSION_FAILED`、`INVALID_ARGUMENTS` を返します。
不足エラーには代替描画を推奨する案内を含めません。

`output.created` は今回の実行で出力先を作成したかを示します。
途中の書き込み失敗では `status: failed` でも `created: true` になり、
その場合は `folder` を返します。既存出力先への拒否では `created: false` です。
引数エラーの終了コードは 2、変換エラーは 1、成功は 0 です。
文字列は引用・エスケープし、書体名やパスに含まれる記号・改行を保護します。

## Rust ライブラリとして使う

CLI と同じパッケージが library crate も提供します。
ローカルのプロジェクトから使う場合は、依存を追加してください。
以下はローカルパスでの利用例であり、crates.io への公開は前提にしません。

```toml
[dependencies]
pptx2img = { path = "../pptx2img" }
```

```rust,no_run
use pptx2img::{convert, ConvertOptions, StrictModeError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = ConvertOptions {
        width: 1600,
        ..Default::default()
    };
    match convert("slides.pptx", "images", &options) {
        Ok(report) => {
            for slide in &report.slides {
                for warning in &slide.warnings {
                    eprintln!("スライド {}: {warning}", slide.slide);
                }
            }
        }
        Err(error) => {
            if let Some(strict) = error.downcast_ref::<StrictModeError>() {
                eprintln!("停止時の診断: {:?}", strict.slides);
            }
            return Err(error.into());
        }
    }
    Ok(())
}
```

`convert` は同期 API です。PNG と `manifest.json` を書き出し、
manifest と同じ内容を持つ `ConversionReport` を返します。
既存の出力先は上書きしません。幅は CLI と同じ 64〜4096 ピクセルです。
ライブラリ自身は標準出力・標準エラーへメッセージを書き込みません。
警告・構造化診断は `report.slides` から取得できます。
診断コードは公開 enum `DiagnosticCode` で判定できます。

`strict: true` で警告を検出すると、出力先を作らずエラーを返します。
`StrictModeError.slides` に停止時の警告・診断が入り、
その `image` は未出力の予定ファイル名です。
その他のエラーも `anyhow::Error` で返します。
途中の I/O エラーでは画像が残ることがあります。
初期プロトタイプのため、公開 API は今後変更される可能性があります。

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
- 埋め込まれた PNG / JPEG の配置と srcRect による切り抜き
- 基本的なプレースホルダー継承、マスター・レイアウトの共通図形と単色背景
- グループ座標系、明示セル書式の表（矩形の結合を含む）

## 制限

次は未実装です。品質判定に使う場合は特に注意してください。

- レイアウト・マスターの全属性・文字スタイルの完全な継承
- テーマの描画スタイル行列と themeOverride
- 完全な禁則・完全な自動句読点圧縮・全記号のぶら下がり・複雑な文字 shaping・自動縮小
- あらゆるフォント・文字組みで Office と一致するベースライン
- Office と同じフォント代替・文字組みの完全な再現
- 表のテーマスタイル、グラフ、SmartArt、グループの塗り継承、自由図形、多くのプリセット図形
- 透明度、グラデーション、影などの効果、画像のタイル・塗り先の矩形指定
- SVG / EMF / WMF 画像、外部リンク、アニメーション
- Strict OOXML、旧式 .ppt、暗号化ファイル

未対応検出も開発途中です。strict モードは現在検出できる警告に対する
ゲートであり、未実装属性の完全検出や PowerPoint との一致は保証しません。
フォントが異なる端末では文字の見え方が変わります。
通常テストは Windows / Linux / macOS の CI で検証します。
描画比較の実測環境は macOS です。

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
  → PNG + manifest（src/lib.rs）
  ↑ CLI の引数解析・メッセージ表示（src/main.rs）
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
境界幅・複雑な文字組みの一致と、継承対象属性の拡充は今後の課題です。
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
使用するシェルの方法で設定してください。Windows / Linux の LibreOffice 比較は未実測です。

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
各文字の x 座標を指定するよう改善しましたが、合字などの複雑な処理では幅に差が残り、
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

字間ゼロの行では、枠の幅を 1/100 mm 単位に四捨五入し、収まり判定に使う
累積文字幅は切り捨てます。句読点ごとの圧縮可能量も同じ単位で切り捨てて
足し合わせます。LibreOffice 26.2.5.2 / macOS /
Hiragino Sans の比較結果から得た近似であり、OOXML 全般の規則とは断定しません。
例えば 24pt の「あ、いう、え」は圧縮 ON で幅 120pt では折り返し、
120.01pt では一行になります。18pt では 90.01pt / 90.02pt が境界です。

`examples/boundary-widths.pptx` は、12pt・18pt・20pt・21.5pt・24pt・32pt の通常文字と句読点について、
境界の前後 ±0.1pt 内を 0.01pt の差も含めて調べる自作の 162 ページです。
圧縮 OFF / ON の計 324 条件を LibreOffice と比較します。
既存の MAE と差分画素率に加え、文字の行数と各行に占める描画量の割合を
比較します。後者は全体の濃さで正規化し、割合差 0.02 以下を要求します。
OCR ではなく、白背景・黒文字・単一文字枠の専用資料での回帰検出です。
同じ二行のまま一文字が移る差や空画像も検出する単体テストがあります。
`line-break-report.json` に条件、行の範囲、描画量の割合、判定を残します。

```sh
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice boundary_widths -- --ignored --nocapture
python tests/visual/make_boundary_widths.py examples/boundary-widths.pptx --include-prefix-cases
```

調査時の 12pt・18pt・24pt の 63 ページ、圧縮 OFF / ON の計 126 条件では、
改行の不一致が変更前 27 件から変更後 2 件に減りました。
既存の画像比較だけでは変更前も全件合格していたため、行単位の検査を追加しています。

調査資料 `examples/boundary-font-metrics.pptx` は全 72 ページを保持しています。
20pt・21.5pt・32pt の境界差は下記の幅計測の変更で、残っていた
12pt・圧縮 OFF・幅 59.98pt / 59.99pt の改行差は累積幅の判定変更で改善しました。
元の再現資料も保持し、次の手動比較で全 144 条件を確認できます。
不一致を無視する例外や閾値の緩和は入れていません。

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
この段階で残った 2 件は上記 12pt のケースで、下記の累積幅判定で改善しました。

通常の Rust テストは自作の `tests/fixtures/metrics.ttf` を使い、6 サイズで
全角・半角・空白・幅ゼロの比率、描画サイズの保持、欠落文字の警告を検証します。
このフォントは単純な矩形字形と自前の幅値だけで作り、外部の字形は使っていません。
実行時依存は追加していません。再生成だけには `fontTools` が必要です。

```sh
python tests/fixtures/make_metrics_font.py
```


### 12pt の残存改行差（2026-10-15）

12pt の「あ、いう、え」では、先頭 5 文字の公称幅は 60pt、
1/100 mm 換算で約 2116.67 です。幅 59.98pt / 59.99pt の枠は、
同じ単位への四捨五入で 2116 になります。累積文字幅まで四捨五入して 2117 と
判定していたため、5文字目の読点が収まらず、禁則により手前の文字も追い出していました。

収まり判定に使う累積文字幅の整数部を取るよう変更し、文字ごとの幅・描画座標・
禁則の規則は変更していません。各文字を切り捨ててから合計すると誤差が蓄積するため、
必ず候補行の累積幅を使います。明示字間の書式区間単位の判定は維持しています。

比較では、句読点なしの「あいうえおか」、5文字で終わる「あいうえお」「あ、いう、」
でも同じ境界を確認しました。字形や句読点だけに依存する特例は入れていません。
12pt の 36pt・48pt・60pt の境界前後と文字列末尾も自動比較に加えました。
既存の 72 ページ資料に残っていた不一致は **2 件から 0 件**になります。

これは LibreOffice 26.2.5.2 / macOS / Hiragino Sans の観測結果に基づく近似です。
[累積文字位置の計測](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/vcl/source/outdev/text.cxx#L850)と
[改行候補から禁則を適用する処理](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/editeng/source/editeng/impedit3.cxx#L1976)
も調査時に参照しましたが、コードの移植は行っていません。
句読点の圧縮量や字形位置の完全一致は引き続き未達です。


### 境界幅で残る最大圧縮と行内位置（2026-10-15）

折り返した行を自然幅へ戻せる整数境界では、最大圧縮の文字位置が残る挙動を
反映しました。24pt の「あ、いう、え」は枠幅 119.9pt ではほぼ圧縮が戻り、
119.95〜120pt では先頭行に最大圧縮が残ります。後続行のない文字列末尾には
この保持条件を適用せず、枠に収めるために必要な量だけ圧縮します。

保持判定は自然幅と枠幅を 1/100 mm の整数へ丸めて比較し、境界の 1 単位を
扱います。この値は保持判定だけに使い、改行判定や画像比較の閾値は緩めません。
[圧縮済み文字位置の復元処理](https://github.com/LibreOffice/core/blob/libreoffice-26.2.5.2/editeng/source/editeng/impedit3.cxx#L4917)
も挙動の確認に参照しました。コードの移植はせず、自作資料の観測結果に基づく
独立実装です。LibreOffice の実行時依存は追加していません。

新しい自作資料 `examples/punctuation-positions.pptx` は、読点・句点・括弧を
12pt・18pt・24pt の境界前後で確認する 162 ページです。圧縮 OFF / ON の
324 条件で、行内位置の不一致は旧実装の **38 件から 0 件**になりました。
既存の LibreOffice 比較 6 系統も合格しています。

新しい比較は従来の画素比較と改行比較に加え、各行の横方向の描画量を正規化し、
累積分布の差から位置のずれを測ります。分布間の移動距離と左右端のずれが
それぞれ 2px 以下で合格です（画像幅 1200px、専用資料では約 0.48pt）。
外枠が同じでも内部の描画位置が動く差を検出しますが、OCR や文字ごとの
座標照合ではありません。今回の最大移動距離は約 1.91px、端のずれは 2px でした。

```sh
python tests/visual/compare_punctuation_positions.py \
  --binary target/release/pptx2img \
  --fixture examples/punctuation-positions.pptx --out /tmp/punctuation-positions
python tests/visual/make_punctuation_positions.py examples/punctuation-positions.pptx
```

結果は `position-report.json` と OFF / ON ごとの画像・差分・環境記録に残ります。
Rust の ignored テスト `punctuation_positions_match_both_modes` からも実行できます。

連続する括弧「「あ」」い」には改行や位置の差が残っています。既知の不一致を
`examples/punctuation-position-nested.pptx` に保持し、次の比較は失敗を報告します。
合格する自動回帰資料からは分けています。混在書体・余白・複雑な shaping を含む
完全互換性を意味せず、近似描画の警告も維持しています。

```sh
python tests/visual/compare_punctuation_positions.py \
  --binary target/release/pptx2img \
  --fixture examples/punctuation-position-nested.pptx --out /tmp/punctuation-nested
python tests/visual/make_punctuation_positions.py \
  examples/punctuation-position-nested.pptx --sizes 24 --nested
```

## 5 点の改善（2026-10-15）

今回追加した範囲と、まだ保証しない範囲を分けます。

| 改善 | 追加した動作 | 残る制限 |
|---|---|---|
| 共通ひな形の継承 | マスター→レイアウト→スライドの描画順、単色背景、プレースホルダーの座標・図形属性・基本文字書式 | テーマのスタイル行列、themeOverride、完全な文字書式継承 |
| 複合的な部品 | 入れ子グループの拡大・反転・回転、表の明示セル書式・結合、PNG/JPEG の srcRect 切り抜き | グループの塗り継承、表のテーマスタイル・帯、画像の効果・タイル・fillRect 指定 |
| フォントの整合 | 計測したフォント ID を描画にも使用、代替文字の再計測、各文字の x 座標を指定 | 複雑な文字組み・合字・自動縮小・PowerPoint との完全一致 |
| 継続検証 | 3 OS の通常 CI、固定版 LibreOffice による週次・手動比較、失敗時の証跡保存 | 最新結果は PR のチェック欄を参照。ホスト OS イメージ自体は完全固定ではない |
| 構造化警告 | 分類コード・部品・図形 ID/名前・図形ツリーやセル内の位置を manifest に記録 | 未対応属性すべての検出、図形への警告の重ね描き |

プレースホルダーはスライド→レイアウトで `idx`、レイアウト→マスターで
種類を照合します。ひな形に書かれたサンプル文字は描画せず、スライド側の本文に
設定を補います。スライド側の明示指定を優先し、`showMasterSp="0"` で
上位層の共通図形を非表示にできます。座標・単色塗り・基本文字書式の範囲での対応であり、
全属性の継承を保証するものではありません。
グループの入れ子は 64 段、表は 100000 セルまでです。
変換処理は 8 MiB のスタックを確保したスレッドで実行します。
OS ごとのメインスレッドの容量差によって XML 解析が異常終了しないようにし、
64 段の描画と、65 段以上の入力に対する終了コード 1・出力なしをテストしています。

### フォントを指定したものだけに限定する

```sh
./target/release/pptx2img input.pptx --out images \
  --no-system-fonts --font-file /path/to/font.ttf
```

`--font-file` は複数指定できます。`--no-system-fonts` を付けなければ、
従来どおり OS のフォントも利用します。manifest の `fonts` に使用した
ファミリー名・PostScript 名・コレクション内の face index を記録します。
同名で異なる版のフォントの同一性を保証するものではないため、
再現する場合は同じフォントファイルを指定してください。
描画時にだけ別のフォントへ置き換える動作を止め、代替は幅計測前に解決します。
対応する字形がなければ `missing_glyph` として記録し、近似の警告を維持します。

### 機械で読める警告

manifest に `schema_version: 2` を追加しました。
既存の `slides[].warnings`（重複を除いた文字列の一覧）は維持し、
`slides[].diagnostics` に対象ごとの記録を追加しています。
同じ警告でも対象図形が違えば別の記録です。ページ番号は親の `slide` です。

```json
{
  "code": "omitted",
  "part": "ppt/slides/slide1.xml",
  "object_id": "42",
  "object_name": "未対応グラフ",
  "location": "spTree/0/2",
  "message": "未対応の要素を省略しました: graphicFrame（表以外）"
}
```

分類は `unsupported`（未対応の属性）、`omitted`（要素の省略）、
`approximation`（近似）、`font_substitution`（書体の代替）、
`missing_glyph`（字形なし）、`text_overflow`（枠からのはみ出し）です。
図形 ID・名前がない要素ではその値は `null` です。
表は `location` に行・セル番号も記録します。
`--strict` は引き続き、いずれかの警告があれば出力前に停止します。

### 追加テストと自動実行

`tests/rendering.rs` は OS フォントを無効にして自作フォントを使い、
継承と明示書式の描画一致、図形・表・画像の画素、警告の対象の分離を検証します。
自作の `examples/composite-shapes.pptx` は、グループ・結合表・画像切り抜き・
継承タイトルの 4 ページを LibreOffice と比較します。

```sh
PPTX2IMG_TEST_PYTHON="$PWD/.venv-visual/bin/python" \
  cargo test --test libreoffice composite_shapes -- --ignored --nocapture
# 資料の再生成時だけ python-pptx 1.0.2 が必要
.venv-visual/bin/python tests/visual/make_composite_shapes.py
```

`.github/workflows/ci.yml` は push / PR ごとに Rust のテスト・書式・静的解析と
Python 比較器の単体テストを実行します。Windows / Linux の結果は CI の初回実行で
確認する必要があります。
`.github/workflows/visual.yml` は週次・手動で描画比較を実行します。
LibreOffice 26.2.5.2 の配布物を SHA-256 で確認し、主要な日本語フォント
（Hiragino Sans W3 / W6）のハッシュも `check_environment.py` で確認します。
フォントが基準と違えば比較前に失敗させます。OS とその他のフォントの情報も
従来どおり report に保存します。環境差を判定閾値の緩和で吸収しないでください。
ローカルでの全比較は LibreOffice 不要の通常テストから引き続き分離しています。

実装時の参照（仕様・依存ライブラリの公開 API）：

- [Microsoft: スライドマスター](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-slide-masters)
- [Microsoft: グループ座標変換](https://learn.microsoft.com/ja-jp/dotnet/api/documentformat.openxml.drawing.transformgroup)
- [usvg 0.48.1: FontResolver](https://docs.rs/usvg/0.48.1/usvg/struct.FontResolver.html)
- [GitHub: ホストランナー](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)

新しい実行時依存は追加していません。LibreOffice は比較にのみ利用します。

今回のローカル検証（macOS / LibreOffice 26.2.5.2）では、Rust 45 件、
Python 比較器 14 件、LibreOffice 比較 8 系統が合格しました。
`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、
`actionlint` も合格しています。既存の画像・改行・行内位置の閾値は変更していません。
PowerPoint の参照画像との照合は未確認です。GitHub 上の CI 結果は PR のチェック欄を参照してください。

### 不足フォントと明示的な代替

`--font-file /path/font.ttf` で利用権のあるフォントを追加できます。
代替を許可する場合だけ `--font-fallback` を指定します。Rust API では
`ConvertOptions.font_fallback = true` に相当し、既定は `false` です。
停止時は `FontFallbackError.slides` から構造化診断を取得できます。
`--font-map 'Meiryo UI=Noto Sans CJK JP'` は要求書体を別 family に対応させます。
この明示対応にも `--font-fallback` が必要です。
複数指定でき、同じ要求名の再指定は最後を採用します。対応は一段のみで、
循環する指定でも連鎖探索しません。Rust API では `ConvertOptions.font_mapping`
に同じ対応を設定します。代替先が存在しない場合は診断して自動選択へ戻ります。
フォントの自動取得・再配布は行いません。

自動選択は対象文字の対応範囲、斜体の一致、太さの差、次の family 順で評価します。
Noto Sans CJK JP、Noto Sans JP、Yu Gothic、Hiragino Sans、Meiryo、
Arial Unicode MS、Arial、その他。同順位は PostScript 名・family 名・face index
の順で安定させます。指定 family 内の選択も文字の対応範囲を考慮します。
欠字は同じ規則で別 face を探し、全候補で描けない字は `missing_glyph` を残します。

manifest の `slides[].diagnostics[].font` は要求 family、実 family、PostScript 名、
face index、理由を保持し、診断の `part`・`object_id` とスライド番号から影響箇所を
取得できます。理由は `family_missing`（書体不在）、`explicit_mapping`（明示指定）、
`mapped_family_missing`（代替指定先不在）、`glyph_fallback`（字形不足）、
`style_approximation`（太さ・斜体不一致）です。CLI は書体ごとにまとめて表示します。
計測と描画は同一 face を使いますが、代替後も元書体との文字幅・行高差は残ります。

### 本文の既定書式

通常のテキストボックス（`txBox=1`）では `presentation/defaultTextStyle` を使います。
プレースホルダーは従来どおりレイアウト・マスターの書式を継承し、図形内文字は
`otherStyle` を参照します。run・段落・リストの明示値が既定値に優先します。
空段落は `endParaRPr` のサイズとフォントメトリクスで行高を決めます。

仕様の入口は [MicrosoftのdefaultTextStyle解説](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-presentations)。
`examples/text-inheritance.pptx` は自作の同一Arial・明示18pt／継承18ptの比較資料です。
LibreOfficeでも両者は18ptです。縦位置・行送りの画像差は残り、既存比較閾値を
超えるため完全互換とは扱いません。枠に収まらない文字は切り捨てず診断を残します。

### 保存済み自動調整

`normAutofit` の `fontScale`（未指定100%）を全runの計測・折り返し・描画へ
適用します。`lnSpcReduction`（未指定0%）は百分率行間から減算し、例えば
120%-20%=100%になります。固定ptの行間は減らしません。
子の `noAutofit` は親の自動調整を解除します。保存率の動的な再探索は行いません。

`spAutoFit` は横書き・固定幅で必要な高さまで枠を拡張し、上端・中央・下端の
アンカーと余白を保持します。表セル・縦書き、折り返しなしの横方向拡張は未対応で
警告します。縦書きや表の行高まで対応済みとは扱いません。
自作資料は `examples/text-autofit.pptx`、再生成コードは
`tests/visual/make_text_autofit.py` です。

[NormalAutoFitの仕様](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit)と
[OfficeのspAutoFit解釈](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/19d31037-28bf-4693-85be-3a37bfe82d51)を参照。
LibreOffice 26.2.5.2はこの自作資料の保存縮小率を再計算し、保存枠も自動拡張しないため、
全画素比較には差が残ります。既存の閾値を緩めず差分を記録します。

### 箇条書きと段落の字下げ

`buChar` と `buAutoNum` の記号・番号を本文とは別に配置します。
記号位置は `marL+indent`、本文と折り返し行の開始は `marL`、右余白は `marR` です。
段落・リスト・マスターから書式を継承し、`buNone` で解除できます。
記号の書体・サイズ・色も継承し、不足書体の対応は本文と共通です。

自動番号は算用数字・英字・ローマ数字と句点／閉じ括弧／両括弧／装飾なしに対応します。
番号は階層ごとに数え、段落に直接指定した `startAt` で再開始します。
リストから継承する開始番号は列の初回だけに使います。画像記号は未対応、
その他の番号形式と左揃え以外の記号位置は警告付き近似です。
自作比較資料は `examples/text-bullets.pptx`、再生成コードは
`tests/visual/make_text_bullets.py` です。LibreOfficeと番号・階層・折り返しを
照合していますが、縦位置と行送りの差は既存画像比較閾値を超えています。
