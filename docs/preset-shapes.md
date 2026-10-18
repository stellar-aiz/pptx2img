# プリセット図形の実装根拠と比較

対象は homePlate、upArrow、rightArrow、rightBrace、
wedgeRoundRectCallout。輪郭と文字矩形を図形のローカル座標系で作り、
既存の回転・反転・グループ変換を適用する。

[ECMA-376 Part 1 第5版](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
配布物内の DrawingMLGeometries にある数式・経路を仕様として参照し、
Rust の実装を独立に作成した。upArrow は rightArrow の座標交換で表現する。
ibis-office のコード・テスト・素材は取り込んでいない。

`avLst` の数値 `val` を扱い、未知の調整式は警告して既定値へ戻す。
未対応のプリセットは引き続き omitted 診断となる。

再現資料は次のコマンドで生成できる。生成物は実資料を含まない。

```sh
python tests/visual/make_preset_shapes.py /tmp/presets.pptx
python tests/visual/compare.py --fixture /tmp/presets.pptx \
  --binary target/debug/pptx2img --out /tmp/presets-comparison --width 1600
```

2026-10-18、LibreOffice 26.2.5.2、Hiragino Sans、句読点圧縮OFFで比較。
標準値ページは MAE 0.000521／差分率0.313%、
調整値・回転・反転ページは MAE 0.001012／差分率0.473%。
既存の MAE 0.02・差分率5%・画素差許容16を変更していない。
文字組みとテーマ背景の既知の診断は残る。Office 完全互換の証明ではない。

## 線とコネクタ

straightConnector1 と bentConnector2、11種類の prstDash、5種類の
headEnd/tailEnd、cap と join を扱う。線幅を単位に線種と端部寸法を決める。
端部と線本体は重ね描きなので、小さい矢印やstealthの接合部に既知差があり、
approximation診断を残す。custDashなどは未対応警告を維持する。

自作資料: `python tests/visual/make_lines.py /tmp/lines.pptx`。
LibreOffice比較3ページは既存閾値内（MAE 0.004429、0.005158、0.001855、
差分率0.764%、2.025%、0.827%）。これは接合部の完全一致を意味しない。

## テーマ線参照

`lnRef` の1始まりの `idx` でテーマの `lnStyleLst` を選び、`phClr` を
参照側の色へ置き換える。テーマ線、継承した線、最も近い直接指定の順で重ねる。
幅・cap・複合線属性、塗り、破線、接合、始端・終端を個別に解決する。
直接指定した `noFill`、端部の `type="none"`、`prstDash="solid"` は親の指定を解除する。
端部の部分指定では未指定の属性を保持する。`idx=0` は線なしとし、直接指定は優先する。
未解決の参照番号、custDash、複合線などの未対応警告を維持する。

根拠は [LineReference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.linereference?view=openxml-3.0.1)、
[LineStyleList](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.linestylelist?view=openxml-3.0.1)、
[Outline](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outline?view=openxml-3.0.1)。
仕様と自作回帰資料から独立に実装した。

```sh
python tests/visual/make_theme_lines.py /tmp/theme-lines.pptx
python tests/visual/compare.py --fixture /tmp/theme-lines.pptx \
  --binary target/release/pptx2img --out /tmp/theme-lines-comparison --width 1600
```

自作資料はテーマ指定、同等の直接指定、幅0.25ptの部分上書きの3ページ。
回帰ではテーマ指定と直接指定の全画素一致、上書き・解除・未対応診断を検証する。
線端の接合は従来どおり近似であり、細い線の端部サイズ等には参照との差が残り得る。
フォント代替の見た目の残差は別の [Issue #25](https://github.com/stellar-aiz/pptx2img/issues/25) で追跡する。

細線の端部は、自作0.25/0.5/0.75/1/1.5/2/3/4/6ptとsm/med/lgの27組合せを
LibreOffice 26.2.5.2で実測した。2pt以下では端部寸法が一定となり、2pt超は
線幅に比例したため、端部の基準幅をmax(線幅, 2pt)で近似する。線本体は太くしない。
この近似は診断に明記し、PowerPoint完全互換とは扱わない。
再現: `python tests/visual/make_arrow_sizes.py /tmp/arrow-sizes.pptx`。
