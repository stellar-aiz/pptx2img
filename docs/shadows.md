# 外側の影

outerShdw の色・alpha・方向・距離・ぼかしをSVGフィルターへ変換する。
SourceAlphaから影を作り、元図形の後ろへ合成する。図形の回転・反転、
rotWithShape、後続図形との重なり順を画素テストで確認する。
ぼかしは半径の半分を標準偏差とするGaussian近似で、診断を残す。
拡縮・傾斜、innerShdw/glow等、テーマeffectRefは未対応として警告する。

[OOXML OuterShadowの属性](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outershadow)
と本プロジェクトの自作比較に基づく独立実装。
ibis-office のコード・テスト・素材は転用していない。

自作資料: `python tests/visual/make_shadows.py /tmp/shadows.pptx`。
2ページ目の影パラメータは原資料10ページで確認した
blurRad=50800、dist=38100、dir=2700000、black・alpha=40000、
algn=tl、rotWithShape=0を含む。原資料の形状・文字・素材は転用していない。

LibreOffice 26.2.5.2、1600pxで最終比較:
方向のページはMAE0.001331・差分率0.682%、
枠のページはMAE0.002667・差分率1.070%。既存閾値は変更していない。
枠の局所画像を目視し、影の位置と濃淡が参照へ近づいたことを確認した。
ぼかし核や回転した枠の影には残差がある。
