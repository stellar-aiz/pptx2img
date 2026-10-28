# 依存ライセンスと SBOM

開発環境で次を実行する。配布する Rust CLI からは呼び出さない。
Python はこの開発用ツールだけで使用する。

```sh
python3 tools/dependency-artifacts.py --out-dir target/dependency-artifacts
python3 -m unittest discover -s tools -p 'test_*.py'
```

生成処理は `cargo metadata --locked --offline` を使う。
必要なクレートを開発環境へ取得しておく。生成時にネットワークは使わない。
既存の出力ディレクトリは拒否する。

生成物は CycloneDX 1.6 の `sbom.cdx.json`、`NOTICE.txt`、
`licenses/` 配下の取得済みクレートのライセンス原文・著作者資料。
原文は翻訳せず保持し、NOTICE に資料の SHA-256 を記録する。
日時・マシンのパスを含めず、同じ metadata と資料から同じ内容を生成する。

一覧は Cargo metadata が解決した全対象 OS・開発依存を含む。
個々の配布バイナリに含まれる依存だけを表すものではない。
配布対象ごとの絞り込みと、実際のバイナリとの対応付けは今後追加する。

ライセンス式の AND / OR / WITH と括弧を解析し、許可一覧にある寛容な
ライセンスを選べない依存は停止する。OR の選択肢に copyleft が含まれても、
MIT 等を選べる場合は選択結果を記録して通す。AND は全項目の許可を必要とする。
不明な識別子・構文、ライセンス資料の欠落は黙って通さない。
許可一覧の変更は依存変更と同じレビュー対象とする。

ライセンス資料の自動収集だけで再配布条件の充足を保証しない。
配布前には依存が要求する個別の表示と、収集対象外の第三者資産も確認する。
フォントはこの依存一覧に含まれず、別途埋め込み権限を検査する。

2026-10-20 の生成物は、[公式 JSON スキーマ](https://cyclonedx.org/schema/bom-1.6.schema.json)
で検証済み。ライセンス式の許可・拒否は開発用回帰テストで検査する。


## バイナリとの同梱

[配布手順](distribution.md) の生成処理は上記と同じライセンス検査を呼び、
SBOM・NOTICE・原文資料を、ビルドした単一実行ファイルと同じ ZIP に収める。
プロジェクト自身の `LICENSE` は Cargo.toml の既存 MIT 宣言に対応する。
ライセンス本文は [MIT の原文](https://opensource.org/license/mit) を保持する。
梱包マニフェストにソースコミット、Cargo.lock と全ファイルのハッシュを記録する。
この対応付けでも、SBOM の列挙範囲は全対象 OS・開発依存を含むままである。
