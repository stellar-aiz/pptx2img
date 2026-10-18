# OLE の静的プレビューと非表示診断

OLE 本体は実行せず、内部の PNG/JPEG 画像だけを既存画像経路へ渡す。
外部参照は取得しない。表示座標は graphicFrame に従い、内側の pic の
座標を重ねて適用しない。mc:AlternateContent は Fallback の OLE を
一つだけ選択する。Choice の拡張表現には依存しない。
frame または preview が hidden の場合は非表示を保持する。

内部画像を表示した場合も静的代替である旨の近似診断を残す。
プレビューなし、部品・関連なし、未対応形式、外部参照は種類と理由を
省略診断へ残す。WMF/EMF の描画や OLE バイナリの解釈は対象外。

## 元資料の局所調査

元資料の SHA256 は Issue #12 に記録済み。素材・本文・画像は公開しない。
`tests/visual/audit_ole.py 入力.pptx` で以下の構造情報のみ抽出できる。
形式は関連先の画像バイトを Pillow で判定した。

| 頁 | 種別 | Choice preview | Fallback preview | hidden | 別図形で同じ画像を参照 |
|---|---|---|---|---|---|
| 1 | TCLayout.ActiveDocument.1 | なし | WMF / rId4 | 1 | なし |
| 3 | TCLayout.ActiveDocument.1 | なし | WMF / rId7 | 1 | なし |
| 9 | TCLayout.ActiveDocument.1 | なし | WMF / rId7 | 1 | なし |
| 10 | TCLayout.ActiveDocument.1 | なし | WMF / rId4 | 1 | なし |
| 11 | TCLayout.ActiveDocument.1 | なし | WMF / rId7 | 1 | なし |
| 12 | TCLayout.ActiveDocument.1 | なし | WMF / rId4 | 1 | なし |
| 18 | TCLayout.ActiveDocument.1 | なし | WMF / rId7 | 1 | なし |

全件で Choice/Fallback は同じ OLE 関連を持つ代替表現であり、
二つの可視図形ではない。選択した Fallback だけを診断する。
全件の位置は (1588, 1588)、寸法は (1227, 1588) EMU。
幅 1600 px の出力で位置約 (0.208, 0.208)、寸法約 (0.161, 0.208) px。
非表示指定に加え、表示枠自体も 1 px 未満である。

3・9・11・18 頁の左上 40×40 px を変更前・変更後・LibreOffice 参照で
局所目視した。全て白で一致し、差分画素は 0。対象 OLE の可視内容の
復元を示す結果ではなく、非表示指定を守り診断を具体化した結果である。
各頁の診断は Fallback の hidden を理由とし、対象 frame ID 6 に対応する。

調査途中の「プレビューなし」は Choice のみを見た暫定結果で、Fallback
を含めて訂正した。「PNG」という暫定報告もバイト検査により WMF に
訂正した。上表が確定した調査結果である。

## 自作資料での検証

`tests/visual/make_ole.py` は自作 PNG/JPEG を OLE プレビューと通常画像に
並べた 2 頁を生成する。本体は自作の無効なデータで、実行不要を確認する。
LibreOffice 比較は MAE 0.001201 / 0.001731、差分率 0.494% / 0.461% で
既存閾値内。画像座標・輪郭・左右の一致を目視した。

Rust テストでは半透明画像の画素で一回の描画、PNG/JPEG、Fallback 選択、
hidden、欠落・未対応形式・外部参照の理由と対象 ID を検証する。
`tests/fixtures/ole-preview.jpg` は Pillow で生成した自作 2×2 の赤い画像。
