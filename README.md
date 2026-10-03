# VTLXX

VTLXX は、極小の実行系から上位の言語層を bootstrap する構造を検証する実験的な言語処理系です。

Rust 実装は最終ターゲットそのものではなく、意味論と bootstrap 構造を検証する reference implementation / executable specification として扱います。

## 読み始める場所

- repository で設計・実装・テスト・レビューを行うときの共通ルールは [`AGENTS.md`](AGENTS.md) を確認してください。
- Extended Classic VTL (ECVTL) の current source surface、実行方法、記述例、formatting rule は [`crates/poc/extended-classic-vtl/README.md`](crates/poc/extended-classic-vtl/README.md) を実用入口として確認してください。
- 現在の開発段階、対象範囲、進捗は、対応する GitHub Tracker / マイルストーン issue を確認してください。
- 重要な設計判断は採用済み ADR、現在の実装事実はコードとテストを正本とします。

README は入口と実用上の案内です。Tracker、ADR、コード、テストと食い違う場合は README を確定仕様として扱わず、対象に適用される `AGENTS.md` の確認順序に従って現在状態を確認してください。
