# 下線の観測入力

`examples/make-underline-probe.rs` は、未実装の二重・会計下線の寸法を
Excel で観測するための入力を生成する。生成物は正解 fixture ではない。
製品の下線処理や通常 CLI の対応範囲は変更しない。

## 固定した条件

一枚のシートの B〜F 列を、なし・一重・二重・会計一重・会計二重とする。
A 列は条件名。行ごとに次の8条件を並べ、計40件を同じ印刷設定で比較する。

| 行 | 入力 | 条件 |
| --- | --- | --- |
| 2 | Agjp | 11 pt、下端揃え |
| 3 | Agjp | 20 pt、下端揃え |
| 4 | 1234.5 | 組み込み数値書式4 |
| 5 | 1234.5 | 通貨記号・埋め空白・予約空白を含む会計書式 |
| 6 | -1234.5 | 同じ会計書式の負数区分 |
| 7 | 0 | 同じ会計書式のゼロ区分 |
| 8 | Ag、改行、jp | 明示改行、折り返し、行高60 pt |
| 9 | Agjp | Ag は黒11 pt、jp は青20 pt。各部分で下線を指定 |

通常行高44 pt、見出し32 pt、全列幅16、A4横、100%、余白0.5インチ、
印刷範囲 A1:F9。数式・罫線・画像は含めない。
書体名を引数で固定するが、対象アプリが実際に使った書体は別に確認する。

## 再現

```sh
mkdir -p target/underline-observation
cargo run --locked --offline --example make-underline-probe -- \
  target/underline-observation/input.xlsx 'Hiragino Sans' \
  > target/underline-observation/input.json
```

出力先は未作成である必要がある。JSON に入力ハッシュ・要求書体・条件を保存し、
実使用書体は null、Excel と LibreOffice の参照は unobserved のまま出す。
同じ引数から生成する ZIP はバイト一致する。機密帳票・書体本体は含めない。

2026-10-21 のローカル生成は3015バイト、SHA-256 は
`3127904dc5e3d3f3fd9ee86f8af1c1ed7120dd3dddc70aeaaa46be3b3b9fa1e2`。
読み戻し2試験、Clippy、二回生成の一致、既存出力の拒否と内容不変を確認した。
Artifact Tool の読み込みで54セルの値と配置を確認し、プレビューも目視した。
そのプレビューは会計下線と複数書式の表現を Excel と同等には扱えないため、
下線寸法の正解として利用しない。Excel/LibreOffice の PDF は未取得。

## 次に必要な観測

同じ入力を、代用の有無を確認できる Excel 環境で PDF 出力する。
アプリ版・OS・ロケール・実フォント名と版・印刷条件・入力と PDF のハッシュを
[検証方針](validation.md) に従って記録する。入力を保存し直した場合は、
元入力と同じ比較条件と扱わず別セットに分ける。

全セルについて次を測定し、紙面位置と文字位置を結び付けて保存する。

- 通常下線の基線からの距離・厚さ、二重下線の各線の座標と間隔。
- 会計下線の左右端が文字送り・セル幅・予約空白のどれに従うか。
- 負数・ゼロ・明示改行・サイズの混在による下線位置と分割の変化。
- PDF の表示文字列、書体、用紙寸法、ページ数。

この40件だけでは、結合セル・回転・自動行高・倍率・他書体の規則は証明できない。
測定前に定数を製品へ採用したり、プレビュー結果で Excel 観測を代替しない。

## 仕様との境界

[Microsoft の下線列挙](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-xls/6b992267-b600-40b2-8ac5-4a81f01d3ef4)
と [Open XML の下線値](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.underlinevalues)
は種類の根拠として参照した。これらのページは、今回必要な線間隔や
ページ上の座標の根拠にはならない。
