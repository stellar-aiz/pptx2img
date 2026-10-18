# 色変換とテーマ塗り

lumMod/lumOff は HSL の明度に記述順で適用する。途中で8ビットへ丸めず、
最終色をRGBへ戻す。色相・彩度を保ち、明度は0〜1に制限する。

図形の fillRef はテーマ fmtScheme の fillStyleLst（1始まり）、
bgFillStyleLst（1001始まり）を参照する。phClr には fillRef の色を渡す。
明示 noFill/solidFill を優先する。未知の変換・単色以外のテーマ塗り・
未解決参照は警告に残す。線・効果・文字のテーマ参照は対象外。

根拠は ECMA-376 Part 1 の DrawingML と
[Open XML SDKのFillReference説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.fillreference)
を参照。ibis-office のコード・テスト・素材は転用していない。

自作資料: `python tests/visual/make_colors.py /tmp/colors.pptx`。
LibreOffice 26.2.5.2、1600px、既存閾値で MAE 0.000589、差分率0.422%。

原資料6・14ページをローカル目視・画素確認した。淡青の領域は
RGB (25,163,252) から (210,237,254) となり、参照 (209,237,254) に近づいた。
灰色パネルの塗りは再現され、残る周囲の影は未対応effectLstに由来する。
文字差・箇条書きは別課題。原資料画像は公開していない。
