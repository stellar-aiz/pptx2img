# 開発用配布物の生成と検査

通常 CLI の変換と Excel 参照照合は未完了。ここで生成する ZIP は開発用で、
マニフェストの `status` も `development` に固定する。
梱包成功を製品完成や変換成功として扱わない。

## 生成

クリーンな Git 作業ツリーと対象 CPU・OS のネイティブ環境で実行する。
事前に Rust ツールチェーンと `cargo fetch --locked` による依存取得を済ませる。
Python 3 の標準ライブラリだけを使い、配布バイナリには Python を含めない。

```sh
python3 tools/distribution-bundle.py \
  --target aarch64-apple-darwin \
  --out target/distribution/rhodopsin-xlsx-aarch64-apple-darwin.zip
python3 tools/distribution-bundle.py \
  --verify target/distribution/rhodopsin-xlsx-aarch64-apple-darwin.zip
```

対象は macOS arm64 / x86_64、Windows x86_64、Linux x86_64 の四つ。
`--target` には CI の対象表と同じ Rust ターゲット名を指定する。
ツールは `cargo build --locked --offline --release` を自ら実行し、
Cargo のビルド出力から実行ファイルを一意に特定する。
既存の任意バイナリを指定して別コミットのものと取り違える入口は設けない。

生成前後でソースコミットと作業ツリーを検査し、未記録の変更は拒否する。
ビルドした実行ファイルの `--version` と `--help` を実行する。
これは起動検査であり、PDF 変換の受け入れ試験ではない。

## ZIP の内容

| 内容 | 用途 |
| --- | --- |
| rhodopsin-xlsx または rhodopsin-xlsx.exe | 対象環境の単一実行ファイル |
| LICENSE | プロジェクト自身の MIT ライセンス原文 |
| NOTICE.txt、licenses/ | 依存の宣言、選択ライセンス、原文資料 |
| sbom.cdx.json | 全対象 OS・開発依存を含む CycloneDX 1.6 一覧 |
| manifest.json | 対象、版、ソースコミット、Rust 版、Cargo.lock のハッシュ、各ファイルのハッシュ・サイズ・権限 |
| SHA256SUMS | マニフェストを含む全ファイルの SHA-256。自身は除外 |

ZIP 内は `rhodopsin-xlsx-版-対象/` に収める。実行ファイルには Unix の実行権限を
保存する。ZIP の順序・日時・権限・圧縮方式を固定し、同一バイナリ・資料・
来歴情報から同じ ZIP を作る。異なる OS やコンパイラでのバイナリ一致は保証しない。

生成済み ZIP を内部検査してから、出力先と同じディレクトリの一時ファイルに書き、
排他的に確定する。既存出力を上書きせず、通常の失敗では一時ファイルを除去する。
強制終了・電源断時の残存を無条件に防ぐ保証ではない。

検査はファイル集合、内容ハッシュ、サイズ、実行権限、SBOM の版、対象名を照合する。
署名や発行者の真正性を保証するものではない。Office フォントは含めない。

## CI と残作業

各対象の CI が生成・検査済み ZIP を Actions 成果物に保存する。
GitHub Release への公開はこのワークフローに含まない。
実際のブックの変換、Excel 参照照合、sandbox、ピークメモリ、呼び出し側との
統合の受け入れ検証は別途必要である。
