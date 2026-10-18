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
